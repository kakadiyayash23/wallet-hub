# ADR 002: Idempotency key in the same transaction as the money move

**Status:** accepted · 2026-10-14

## Context

Providers retry callbacks with the same transaction id after timeouts. Two retries can arrive at the same moment. A duplicate must return the original balance and move nothing.

## Decision

`(source, idempotency_key)` is a unique index on `journals`. The lookup, the balance check, the insert and the balance update run in one transaction with the account row locked (`FOR UPDATE`). The reply balance is stored on the journal and returned on duplicates.

Rollbacks write a journal too. A rollback whose original never arrived writes a **tombstone**, so a late original is refused instead of applied.

## Consequences

- No separate idempotency store to drift out of sync.
- Per-account serialisation. Throughput per player is one move at a time, which is what a wallet needs anyway.
