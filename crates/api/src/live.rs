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
    pub round_id: Option<String>,
    pub round_end: Option<bool>,
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
            round_id: None,
            round_end: None,
        }
    }

    pub fn with_round(mut self, round: Option<&domain::RoundRef>) -> Self {
        if let Some(r) = round {
            self.round_id = Some(r.round_id.clone());
            self.round_end = Some(r.round_end);
        }
        self
    }
}
