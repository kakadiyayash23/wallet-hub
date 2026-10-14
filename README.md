# Wallet Hub

A small iGaming operator platform. One Rust service owns the player wallet and a double-entry ledger. Outside systems plug in through traits:

| Integration | Status | What it does |
|---|---|---|
| **BigBang Casino API** | real sandbox | Game catalog, launch URLs, and the seamless wallet callbacks (`user_data`, `balance_change`) with HMAC signatures and idempotent `transaction_id`s |
| **Stripe** | real test mode | Deposits through Checkout, confirmed by signed webhooks. Withdrawals go to a review queue |
| **Sportsbook** | mock with real odds | Our own `mock-sportsbook` service: soccer, cricket, basketball and tennis markets from The Odds API (or fixtures), bet placement, cash-out, signed settlement callbacks |

The Vue app has a player side (lobby, game, wallet, sports) and an operator side (callback log with replay, reconciliation, chaos simulator).

Read [ARCHITECTURE.md](ARCHITECTURE.md) for the design and [docs/adr](docs/adr) for the decisions.

## Prerequisites

- Rust 1.80+ (`rustup`), Node 20+, Docker
- A free BigBang sandbox key (`ek_test_…`) from <https://api.bigbangcasino.bet/portal>
- A Stripe test account (`sk_test_…`) and the [Stripe CLI](https://stripe.com/docs/stripe-cli)
- A tunnel that gives your laptop a public URL. BigBang must reach your wallet endpoints. Prefer a Cloudflare quick tunnel:

  ```bash
  npx -y cloudflared tunnel --url http://localhost:8080
  ```

  The ngrok free tier answers GET requests from browser-like user agents with an HTML warning page instead of your JSON, and those requests never show in the local inspector. A provider that identifies itself like a browser then sees a broken `user_data` endpoint.

## Secrets you need

Copy `.env.example` to `.env` and fill these in:

| Variable | Where it comes from |
|---|---|
| `BIGBANG_API_KEY` | BigBang portal → API keys. Use the `ek_test_` sandbox key |
| `STRIPE_SECRET_KEY` | Stripe Dashboard → Developers → API keys (test mode) |
| `STRIPE_WEBHOOK_SECRET` | Printed by the `stripe listen` command in the run steps below |
| `OPS_API_KEY` | Any long random string. The operator page asks for it |
| `SPORTS_CALLBACK_SECRET` | Any long random string. Shared by `api` and `mock-sportsbook` |
| `ODDS_API_KEY` | Optional. Free key from <https://the-odds-api.com>. Without it the sportsbook uses built-in fixtures |
| `PUBLIC_BASE_URL` | Your tunnel URL, for example `https://abc123.ngrok-free.app` |

Generate random strings with:

```bash
openssl rand -hex 32
```

Never commit `.env`. It is in `.gitignore`.

## Run it (development)

1. Start Postgres:

   ```bash
   docker compose up -d postgres
   ```

   Without Docker, a Homebrew Postgres works too:

   ```bash
   brew install postgresql@16 && brew services start postgresql@16 && createuser -s wallet && createdb -O wallet wallet_hub
   ```

2. Start the API. It applies the migrations on boot and logs the three callback URLs you must configure:

   ```bash
   cargo run -p api
   ```

3. Start the mock sportsbook and the Vue app in two more terminals:

   ```bash
   cargo run -p mock-sportsbook
   ```

   ```bash
   cd web && npm install && npm run dev
   ```

4. Start the tunnel and the Stripe listener:

   ```bash
   ngrok http 8080
   ```

   ```bash
   stripe listen --events checkout.session.completed,checkout.session.async_payment_succeeded,checkout.session.async_payment_failed,checkout.session.expired --forward-to localhost:8080/webhooks/stripe
   ```

   Newer Stripe CLI versions need the event list. Test deposits with card `4242 4242 4242 4242`.

5. In the BigBang Dashboard, set your API key to **seamless wallet** and enter the two callback URLs:
   - `user_data`: `https://<tunnel>/callbacks/bigbang/user_data`
   - `balance_change`: `https://<tunnel>/callbacks/bigbang/balance_change`

   A new key starts in **managed wallet** mode. In that mode games run on BigBang's own virtual balance and your wallet never receives a callback. Switch the mode, or the ledger stays empty while you play.

6. Open <http://localhost:5173> and enter a display name. The API syncs the catalog on its first boot with an empty database; **Operator → Sync catalog** refreshes it later.

7. Open **Wallet** and click **Add 1,000 sandbox funds**. With a sandbox key, every callback carries `sandbox: true` and posts to the sandbox account, which starts at zero. Then open a game. Every spin shows up as a ledger row under the iframe.

## Run it (one command)

```bash
docker compose up --build
```

The app is at <http://localhost:8000>. You still need the tunnel for BigBang and Stripe to reach port 8080.

## Try the money paths without any provider

- **Chaos simulator**: Operator → Chaos → Run. It fires 200 signed BigBang-shaped callbacks at your own wallet with duplicates, reordering and delays. The report must show drift `0.00` and trial balance `0`.
- **Sports bet**: Sports → pick a selection → Place bet. The mock settles it after `SETTLE_AFTER_SECS` (default 30) and sends a signed callback. Watch the balance move.
- **Withdrawal**: Wallet → Request. The ledger reserves the money at once; the payment waits in `pending_review`.

## Tests

```bash
make test
```

- Unit tests: money parsing, the BigBang signature with JavaScript number forms, Stripe webhook signatures, sportsbook callbacks.
- Ledger integration tests (`crates/ledger/tests`): idempotency, insufficient funds, rollback and tombstones, 40 concurrent duplicates. They run when `DATABASE_URL` is set and are skipped otherwise.
- Web: `vitest` for locale-aware money formatting.

```bash
make lint
```

runs `rustfmt`, `clippy -D warnings`, and `vue-tsc`.

## Layout

```
crates/domain            money type, ids, the three traits, WalletCommand
crates/ledger            Postgres store: money rules, events, payments, bets, games
crates/provider-bigbang  GameProvider for BigBang
crates/provider-stripe   PaymentProvider for Stripe
crates/provider-sports   SportsbookProvider client for the mock
crates/api               axum HTTP + WebSocket, callbacks, operator tools, chaos
services/mock-sportsbook stand-in sportsbook with real odds
web                      Vue 3 + TypeScript + Pinia + vue-i18n
migrations               sqlx migrations, applied on boot
docs/adr                 architecture decision records
```

## Known limits

- Auth is a demo: `X-Player-Id` and `X-Ops-Key` headers. A real platform puts sessions and roles here.
- Stripe withdrawals are a review queue, because paying out to a customer needs Stripe Connect.
- BigBang's `user_data` callback carries no documented signature; it is a balance read and is served as such.
- The `X-Body-Signature` header BigBang mentions is checked and logged on mismatch, not enforced, because its algorithm is not published.
- One platform currency (`PLATFORM_CURRENCY`, default EUR).
