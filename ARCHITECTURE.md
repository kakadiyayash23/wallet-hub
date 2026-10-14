# Architecture

## The one rule

**Providers translate. The ledger decides.**

A provider crate turns its wire format into a `WalletCommand`. It verifies the signature and parses the amount. It never touches the database. The ledger applies every command under one rule set, so idempotency, rollback and insufficient-funds logic exist once. A new provider cannot get them wrong.

```
HTTP request ──► api::callbacks ──► store.record_event (raw, first)
                      │
                      ▼
          provider.parse_callback ──► WalletCommand
                      │
                      ▼
               store.apply(cmd) ──► one DB transaction, account row locked
                      │
                      ▼
          provider.render_reply ──► provider-shaped JSON
```

## Crates

| Crate | Depends on | Role |
|---|---|---|
| `domain` | nothing | `Money` (i64 minor units), ids, the three traits, `WalletCommand`, `WalletError`, HMAC helpers |
| `ledger` | domain, sqlx | The `Store`: money rules in `ledger.rs`, plus repositories for events, payments, bets, games, players |
| `provider-bigbang` | domain, reqwest | `GameProvider`: catalog, launch, callback parsing, reply rendering, history |
| `provider-stripe` | domain, reqwest | `PaymentProvider`: Checkout sessions, webhook verification |
| `provider-sports` | domain, reqwest | `SportsbookProvider`: client for the mock, callback verification |
| `api` | all of the above, axum | Routes, auth extractors, callback orchestration, WebSocket, chaos simulator |
| `mock-sportsbook` | domain, axum | A stand-in provider that behaves like a real one |

The dependency arrows only point down. `domain` compiles with no I/O. A provider compiles without the HTTP layer, so it can be unit-tested with fixtures.

## The three traits

```rust
trait PaymentProvider   { start_deposit, start_withdrawal, parse_webhook }
trait GameProvider      { sync_catalog, launch, parse_callback, render_reply, render_error, history }
trait SportsbookProvider{ markets, place_bet, cash_out, parse_callback }
```

`parse_*` functions are synchronous and pure. They take a `RawCallback` (method, path, query, headers, raw body bytes) and verify signatures against the **raw bytes**, never against a re-serialized copy.

## The ledger

Tables: `players`, `accounts`, `journals`, `ledger_entries`, `rounds`.

- An **account** is one balance: `(player, currency, env)` where `env` is `live` or `sandbox`. Sandbox callbacks from BigBang post to the sandbox account and never mix with real money.
- A **journal** is one business event. `(source, idempotency_key)` is unique.
- Each journal owns two **ledger entries** that sum to zero: the player's side and a provider counter-account. `SELECT sum(amount) FROM ledger_entries` is always `0`; the operator page shows it as the trial balance.
- `accounts.balance` is a cache of the entries, updated in the same transaction.

`Store::apply` runs one transaction per command:

1. Find the player, create the account if needed, `SELECT … FOR UPDATE` on the account row.
2. Look up `(source, key)`. If it exists and is not a tombstone, return the stored `reply_balance` with `duplicate: true`.
3. For a `Move`, refuse a negative result with `InsufficientFunds { balance }`. This is an outcome, not a crash: each provider renders it in its own format. BigBang wants `400 { "error": "insufficient balance" }`, which makes it refuse the round without retries.
4. Insert journal + two entries, update the balance, upsert the round.
5. For a `Rollback`: reverse a known journal once (key `rb:<original>`), or write a **tombstone** when the original never arrived. If the original shows up later it is refused with `RolledBack`.

Concurrency: the row lock serialises moves per account. The unique index catches duplicate keys that race past the lookup. The integration test fires 40 concurrent requests on 10 keys and expects exactly 10 applies.

## Inbound events

Every callback and webhook is written to `inbound_events` **before** parsing: source, method, path with query, headers, raw body. After processing, the row gets `signature_ok`, `outcome` (`applied | duplicate | rejected | ignored | error | read`), a detail string and the journal id.

Replay (`POST /api/ops/events/:id/replay`) rebuilds the `RawCallback` from the row and runs the same handler. The replay is a new row with `replay_of` set. Because the ledger is idempotent, replaying an applied event is safe.

## BigBang specifics

- Signature: `HMAC-SHA256(username + amount + game + game_category + transaction_id, API_KEY)`. `amount` is rendered as JavaScript would render the number (`-2.5`, `2`, `0`). The body keeps `amount` as a raw JSON value so the exact wire form is used. See `provider-bigbang/src/signature.rs`.
- Idempotency key: `transaction_id`.
- Rounds: `round_id` + `round_end`. Standard games send one net movement per round; some providers send bet / win / win… with the last flagged `round_end`.
- Zero amounts are accepted. They close losing rounds.
- Retries: BigBang retries a failed **win** (non-2xx, timeout, or 2xx without a balance) with the same `transaction_id`. A refused **bet** (4xx) is not retried. Unknown player is `404`. The reply must come back fast; one indexed lookup and one update do that.
- `sandbox: true` routes to the sandbox account.

## Stripe specifics

- Deposit: `POST /api/wallet/deposits` creates a `payments` row (`pending`), then a Checkout Session with `metadata.payment_id`. The browser is redirected to Stripe.
- Webhook: verify `Stripe-Signature` (`t=…,v1=…`, HMAC over `"{t}.{body}"`, 300 s tolerance). On `checkout.session.completed` with `payment_status=paid`, find the payment by session id, check amount and currency, apply `Move { key: "deposit:<session_id>" }`, mark the payment completed.
- Withdrawal: `Move { key: "withdrawal:<payment_id>", amount: -x }` reserves at once; the payment waits in `pending_review`.

## Sportsbook flow

1. `POST /api/sports/bets`: ledger reserves the stake (`bet:<bet_id>`), a `bets` row is created, the mock is called.
2. If the mock rejects, the api issues a `Rollback` for the stake and marks the bet `rejected`.
3. The mock settles after a delay and POSTs a signed `BetEvent` to `/callbacks/sports`. Won → `Win +payout`, lost → `Win 0`, void → `Refund +stake`, all under key `settle:<ticket>`. Cash-out uses `cashout:<ticket>`.

## Live updates

`AppState.live` is a `tokio::sync::broadcast` channel. After any applied move, the handler publishes a `LiveEvent` (player, env, balance, amount, kind, source). `/ws?player_id=…` forwards matching events as JSON. The Pinia wallet store is the single source of truth for the balance pill.

## Chaos simulator

`POST /api/ops/chaos` seeds the sandbox account, builds N signed `balance_change` bodies with random amounts, adds duplicates and optional bad signatures, shuffles them, and fires them at the server's own callback URL from four workers with random delays. It then compares the final balance with the expected sum of unique valid amounts and reports drift and the trial balance.

## Operational notes

- Migrations are embedded (`sqlx::migrate!`) and applied on boot.
- `tracing` logs are compact in dev; switch to JSON for a log stack.
- Config is env only (`config.rs`). Missing required vars fail fast at start.
- Errors: `WalletError` is the one error type. `api::error` maps it to HTTP for the Vue app; providers map it to their own reply format for callbacks.

## What a second game provider looks like

One crate. Implement `GameProvider`. Translate the provider's callback fields into `WalletCommand::Move` or `Rollback`, with the provider's transaction id as `idempotency_key`. Render replies in the provider's format. Register it in `AppState` and add two routes. Nothing in `ledger` changes.
