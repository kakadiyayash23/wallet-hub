//! Provider-neutral types. This crate has no I/O and no database.
//!
//! The three traits in [`traits`] are the "existing structure" that a new
//! provider implements. A provider only translates its wire format into a
//! [`WalletCommand`]; the ledger applies that command under one rule set.

pub mod error;
pub mod ids;
pub mod money;
pub mod sig;
pub mod traits;
pub mod wallet;

pub use error::WalletError;
pub use ids::{AccountEnv, PlayerId, Source};
pub use money::Money;
pub use wallet::{MoveKind, RoundRef, WalletCommand, WalletOutcome};
