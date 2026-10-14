#!/usr/bin/env bash
# End-to-end smoke test against a running api (8080) and mock-sportsbook (8090).
# Needs: curl, jq, openssl, psql, and the same .env the api uses (for OPS_API_KEY,
# BIGBANG_API_KEY, SPORTS_CALLBACK_SECRET, STRIPE_WEBHOOK_SECRET).
set -euo pipefail
cd "$(dirname "$0")/.."
set -a; source .env; set +a
API=${API:-http://localhost:8080}

say() { printf '\n== %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

say "health"
curl -fsS "$API/api/health" >/dev/null && echo ok

say "dev login"
JAR=$(mktemp); trap 'rm -f "$JAR"' EXIT
PLAYER=$(curl -fsS -c "$JAR" -X POST "$API/api/players" -H 'content-type: application/json' -d '{"display_name":"Smoke Tester","locale":"de"}')
PID=$(echo "$PLAYER" | jq -r .id); TOKEN=$(echo "$PLAYER" | jq -r .external_token)
echo "player $PID token $TOKEN (session cookie saved)"
H=(-b "$JAR" -H 'content-type: application/json')
O=(-H "X-Ops-Key: $OPS_API_KEY" -H 'content-type: application/json')

say "wallet summary creates both accounts"
curl -fsS "$API/api/wallet" "${H[@]}" | jq -c '.accounts | map({env, balance})'

say "bigbang user_data read"
curl -fsS "$API/callbacks/bigbang/user_data?username=$TOKEN" | jq -c .

say "bigbang balance_change: signed win of 12.5"
sign() { printf "%s" "$2" | openssl dgst -sha256 -hmac "$1" | awk '{print $NF}'; }
TXN="smoke-$(date +%s)"
SIG=$(sign "$BIGBANG_API_KEY" "${TOKEN}12.5SmokeSlotSmoke${TXN}")
BODY=$(jq -nc --arg u "$TOKEN" --arg t "$TXN" --arg s "$SIG" '{username:$u, amount:12.5, game:"SmokeSlot", game_category:"Smoke", transaction_id:$t, signature:$s, round_id:$t, type:"round", round_end:true, sandbox:true}')
R1=$(curl -fsS -X POST "$API/callbacks/bigbang/balance_change" -H 'content-type: application/json' -d "$BODY"); echo "$R1"
[ "$(echo "$R1" | jq -r .balance)" = "12.50" ] || fail "expected balance 12.50"

say "same transaction again -> duplicate, same balance, no move"
R2=$(curl -fsS -X POST "$API/callbacks/bigbang/balance_change" -H 'content-type: application/json' -d "$BODY"); echo "$R2"
[ "$(echo "$R2" | jq -r .balance)" = "12.50" ] || fail "duplicate changed the balance"

say "bad signature -> 401"
BAD=$(echo "$BODY" | jq -c '.signature = "00" + .signature[2:] | .transaction_id += "-bad"')
CODE=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/callbacks/bigbang/balance_change" -H 'content-type: application/json' -d "$BAD")
[ "$CODE" = "401" ] && echo "401 ok" || fail "expected 401, got $CODE"

say "bet bigger than balance -> 400 with 'insufficient balance' (round refused, no retry)"
TXN2="$TXN-big"; SIG2=$(sign "$BIGBANG_API_KEY" "${TOKEN}-999SmokeSlotSmoke${TXN2}")
BIG=$(jq -nc --arg u "$TOKEN" --arg t "$TXN2" --arg s "$SIG2" '{username:$u, amount:-999, game:"SmokeSlot", game_category:"Smoke", transaction_id:$t, signature:$s, sandbox:true}')
CODE=$(curl -s -o /tmp/wh-big.json -w '%{http_code}' -X POST "$API/callbacks/bigbang/balance_change" -H 'content-type: application/json' -d "$BIG"); cat /tmp/wh-big.json; echo
[ "$CODE" = "400" ] && echo "400 ok" || fail "expected 400, got $CODE"

say "unknown player -> 404"
CODE=$(curl -s -o /dev/null -w '%{http_code}' "$API/callbacks/bigbang/user_data?username=nobody_$TXN")
[ "$CODE" = "404" ] && echo "404 ok" || fail "expected 404, got $CODE"

say "stripe webhook: signed checkout.session.completed for a real pending payment"
# Create a pending payment row directly through the deposit route would call Stripe; instead insert via the withdrawal-free path:
PAYMENT_ID=$(uuidgen | tr 'A-Z' 'a-z')
psql "$DATABASE_URL" -qtAc "insert into payments (id, player_id, provider, direction, provider_ref, status, amount, currency) values ('$PAYMENT_ID', '$PID', 'stripe', 'deposit', 'cs_smoke_$TXN', 'pending', 5000, 'EUR')"
EVT=$(jq -nc --arg pid "$PAYMENT_ID" --arg cs "cs_smoke_$TXN" '{id:"evt_smoke", type:"checkout.session.completed", data:{object:{id:$cs, payment_status:"paid", amount_total:5000, currency:"eur", metadata:{payment_id:$pid}}}}')
T=$(date +%s); SSIG=$(sign "$STRIPE_WEBHOOK_SECRET" "$T.$EVT")
curl -fsS -X POST "$API/webhooks/stripe" -H 'content-type: application/json' -H "Stripe-Signature: t=$T,v1=$SSIG" -d "$EVT" | jq -c .
LIVE=$(curl -fsS "$API/api/wallet" "${H[@]}" | jq -r '.accounts[] | select(.env=="live") | .balance')
[ "$LIVE" = "5000" ] || fail "expected live balance 5000, got $LIVE"; echo "live balance 50.00 ok"

say "sports: place a bet, wait for the mock to settle, check the ledger"
MK=$(curl -fsS "$API/api/sports/markets?sport=soccer" | jq -c '.[0]')
BET=$(jq -nc --argjson m "$MK" '{event_id:$m.event_id, event_name:($m.home+" v "+$m.away), selection:$m.selections[0].key, odds_milli:$m.selections[0].odds_milli, stake:"10.00"}')
B=$(curl -fsS -X POST "$API/api/sports/bets" "${H[@]}" -d "$BET"); echo "$B" | jq -c '{id, ticket_ref, status, stake}'
sleep "$(( ${SETTLE_AFTER_SECS:-30} + 2 ))"
curl -fsS "$API/api/sports/bets" "${H[@]}" | jq -c '.[0] | {status, payout}'

say "withdrawal reserves at once"
curl -fsS -X POST "$API/api/wallet/withdrawals" "${H[@]}" -d '{"amount":"5.00"}' | jq -c '{status, amount}'

say "ops: health"
curl -fsS "$API/api/ops/health" "${O[@]}" | jq -c '.[] | {source, received_24h, last_error_detail}'

say "ops: chaos 100 rounds"
CH=$(curl -fsS -X POST "$API/api/ops/chaos" "${O[@]}" -d "{\"player_id\":\"$PID\",\"rounds\":100,\"duplicate_every\":4,\"bad_signature_every\":9}")
echo "$CH" | jq -c '{sent, duplicates_rejected, duplicates_sent, bad_signatures_rejected, bad_signatures_sent, drift, trial_balance, elapsed_ms}'
[ "$(echo "$CH" | jq -r .drift)" = "0" ] || fail "chaos drift"
[ "$(echo "$CH" | jq -r .trial_balance)" = "0" ] || fail "trial balance"

say "ops: replay the first bigbang event (must be a duplicate)"
EV=$(curl -fsS "$API/api/ops/events?limit=200&source=bigbang" "${O[@]}" | jq -r --arg t "$TXN" '.[] | select(.body | contains($t)) | select(.outcome=="applied") | .id' | head -1)
curl -fsS -X POST "$API/api/ops/events/$EV/replay" "${O[@]}" | jq -c .
curl -fsS "$API/api/ops/events?limit=3&source=bigbang" "${O[@]}" | jq -c '.[] | {outcome, replay_of}'

say "ledger view (German locale player)"
curl -fsS "$API/api/wallet/ledger?limit=6" "${H[@]}" | jq -c '.[] | {kind, amount, reply_balance, env, source}'

say "ALL SMOKE CHECKS PASSED"
