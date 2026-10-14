use crate::{AccountEnv, Money, Source};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What kind of movement a provider reported. Informational; the ledger treats
/// every kind the same way except `Rollback`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveKind {
    Bet,
    Win,
    Refund,
    /// One self-contained movement per round (amount = win - bet).
    NetRound,
    Deposit,
    Withdrawal,
    Cashout,
}

impl MoveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MoveKind::Bet => "bet",
            MoveKind::Win => "win",
            MoveKind::Refund => "refund",
            MoveKind::NetRound => "net_round",
            MoveKind::Deposit => "deposit",
            MoveKind::Withdrawal => "withdrawal",
            MoveKind::Cashout => "cashout",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundRef {
    pub round_id: String,
    pub round_end: bool,
}

/// The one language every provider speaks to the ledger.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WalletCommand {
    ReadBalance {
        player_token: String,
        currency: String,
        env: AccountEnv,
    },
    Move {
        player_token: String,
        currency: String,
        env: AccountEnv,
        source: Source,
        /// Provider transaction id. Unique per (source, key).
        idempotency_key: String,
        kind: MoveKind,
        /// Signed, player's point of view. Negative = debit.
        amount: Money,
        round: Option<RoundRef>,
        #[serde(default)]
        meta: serde_json::Value,
    },
    Rollback {
        player_token: String,
        currency: String,
        env: AccountEnv,
        source: Source,
        /// The idempotency key of the movement to reverse.
        original_key: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletOutcome {
    pub balance: Money,
    pub currency: String,
    pub env: AccountEnv,
    /// False when the command was a duplicate or a read.
    pub applied: bool,
    pub duplicate: bool,
    pub journal_id: Option<Uuid>,
    pub player_id: crate::PlayerId,
}
