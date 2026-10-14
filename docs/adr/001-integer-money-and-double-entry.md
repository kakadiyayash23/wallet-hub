# ADR 001: Integer money and a double-entry ledger

**Status:** accepted · 2026-10-14

## Context

Providers send decimal amounts as JSON numbers. Balances must be exact and auditable. Rounds can arrive as one net movement or as several linked movements.

## Decision

- `Money` is an `i64` in minor units. Decimal strings and floats are converted once, at the provider edge, with the shortest round-trip form so `0.1` becomes `10` and never `9`.
- Every business event is a journal with two ledger entries that sum to zero. The sum of all entries is the trial balance and must be `0`.
- `accounts.balance` is a cached sum, updated inside the same transaction.

## Consequences

- No floating point in the ledger. Reconciliation is integer subtraction.
- Storage doubles (two rows per event). Acceptable for the volumes here.
- Reports can be built from `ledger_entries` alone.
