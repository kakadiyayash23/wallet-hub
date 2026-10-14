# ADR 003: Providers translate, the ledger decides

**Status:** accepted · 2026-10-14

## Context

The platform will add more payment providers, game aggregators and sportsbooks. Each has its own wire format, signature scheme and error codes.

## Decision

Provider crates implement one trait each and only translate: `RawCallback` in, `WalletCommand` out, `WalletOutcome` in, provider-shaped reply out. They verify signatures against raw bytes. They have no database access.

The ledger applies every `WalletCommand` under one rule set.

## Consequences

- Adding a provider is one crate plus two routes.
- Money rules are tested once, with a real database, and cannot be bypassed.
- The `WalletCommand` enum is the contract to keep stable.
