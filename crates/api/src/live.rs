//! Events pushed to the Vue app over WebSocket after every ledger apply.

use domain::{AccountEnv, Money, PlayerId, WalletOutcome};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
pub struct LiveEvent {
    pub player_id: PlayerId,
    pub env: AccountEnv,
    pub currency: String,
    pub balance: Money,
    pub journal_id: Option<Uuid>,
    pub source: String,
    pub kind: String,
    pub amount: Money,
}

impl LiveEvent {
    pub fn from_outcome(o: &WalletOutcome, source: &str, kind: &str, amount: Money) -> Self {
        Self {
            player_id: o.player_id,
            env: o.env,
            currency: o.currency.clone(),
            balance: o.balance,
            journal_id: o.journal_id,
            source: source.to_string(),
            kind: kind.to_string(),
            amount,
        }
    }
}
