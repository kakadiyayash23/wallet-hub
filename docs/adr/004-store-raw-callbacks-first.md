# ADR 004: Store every inbound callback raw, before parsing

**Status:** accepted · 2026-10-14

## Context

Production debugging of provider integrations is mostly "what exactly did they send, and what did we do with it?". Signature failures and parse errors must not lose the payload.

## Decision

`inbound_events` receives method, path with query, headers and the raw body before any parsing. The outcome is written after. An operator can view the payload and replay it through the same handler; the ledger's idempotency makes replay safe.

## Consequences

- One extra insert per callback. Under 1 ms on an indexed table.
- Payloads may contain player tokens; the table is operator-only and should be retained for a bounded period in production.
