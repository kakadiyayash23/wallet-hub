//! The three integration traits. A provider crate implements one of these and
//! nothing else. It must compile without the HTTP layer.

use crate::{Money, PlayerId, WalletCommand, WalletError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A callback or webhook exactly as it arrived. Providers verify signatures
/// against `body`, never against a re-serialized copy.
#[derive(Clone, Debug)]
pub struct RawCallback {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl RawCallback {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerRef {
    pub id: PlayerId,
    pub external_token: String,
    pub display_name: String,
    pub locale: String,
    pub currency: String,
}

// ---- Payments ---------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct DepositRequest {
    pub payment_id: Uuid,
    pub player: PlayerRef,
    pub amount: Money,
    pub currency: String,
    pub success_url: String,
    pub cancel_url: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DepositRedirect {
    pub provider_ref: String,
    pub redirect_url: String,
}

#[derive(Clone, Debug)]
pub struct WithdrawalRequest {
    pub payment_id: Uuid,
    pub player: PlayerRef,
    pub amount: Money,
    pub currency: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WithdrawalHandle {
    pub provider_ref: Option<String>,
    /// pending_review | paid | failed
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentEvent {
    DepositCompleted {
        event_id: String,
        provider_ref: String,
        payment_id: Option<Uuid>,
        amount: Money,
        currency: String,
    },
    DepositFailed {
        event_id: String,
        provider_ref: String,
        payment_id: Option<Uuid>,
    },
    Ignored {
        event_id: String,
        event_type: String,
    },
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn start_deposit(&self, req: DepositRequest) -> Result<DepositRedirect, WalletError>;
    async fn start_withdrawal(
        &self,
        req: WithdrawalRequest,
    ) -> Result<WithdrawalHandle, WalletError>;
    /// Verify the signature and turn the raw webhook into a typed event. No side effects.
    fn parse_webhook(&self, raw: &RawCallback) -> Result<PaymentEvent, WalletError>;
}

// ---- Casino games ------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameListing {
    pub provider_game_id: String,
    pub name: String,
    pub title: String,
    pub category: String,
    pub thumbnail: Option<String>,
    pub game_type: String,
    pub is_premium: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    Real,
    Demo,
}

#[derive(Clone, Debug, Serialize)]
pub struct LaunchUrl {
    pub url: String,
    pub session_id: Option<String>,
}

/// What the ledger decided, in the shape the provider needs to answer.
#[derive(Clone, Debug)]
pub enum CallbackOutcome {
    Ok(crate::WalletOutcome),
    InsufficientFunds { balance: Money, currency: String },
    PlayerNotFound,
    RolledBack { balance: Money, currency: String },
}

#[derive(Clone, Debug)]
pub struct CallbackReply {
    pub status: u16,
    pub body: serde_json::Value,
}

/// One row of the provider's own transaction history, used for reconciliation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderHistoryRow {
    pub external_id: String,
    pub game: String,
    pub bet: Money,
    pub win: Money,
    pub at: String,
}

#[async_trait]
pub trait GameProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn sync_catalog(&self) -> Result<Vec<GameListing>, WalletError>;
    async fn launch(
        &self,
        player: &PlayerRef,
        provider_game_id: &str,
        mode: LaunchMode,
        return_url: &str,
    ) -> Result<LaunchUrl, WalletError>;
    /// Provider wire format in, provider-neutral command out. Verifies the signature.
    fn parse_callback(&self, raw: &RawCallback) -> Result<WalletCommand, WalletError>;
    fn render_reply(&self, raw: &RawCallback, outcome: &CallbackOutcome) -> CallbackReply;
    fn render_error(&self, err: &WalletError) -> CallbackReply;
    async fn history(&self, player: &PlayerRef) -> Result<Vec<ProviderHistoryRow>, WalletError>;
}

// ---- Sportsbook --------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Market {
    pub event_id: String,
    pub sport: String,
    pub league: String,
    pub home: String,
    pub away: String,
    pub starts_at: String,
    /// selection label -> odds in thousandths
    pub selections: Vec<Selection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SportInfo {
    pub key: String,
    pub label: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    pub key: String,
    pub label: String,
    pub odds_milli: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BetRequest {
    pub bet_id: Uuid,
    pub player_token: String,
    pub event_id: String,
    pub selection: String,
    pub stake: Money,
    pub odds_milli: i64,
    pub currency: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BetTicket {
    pub ticket_ref: String,
    pub odds_milli: i64,
    pub event_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CashOutQuote {
    pub ticket_ref: String,
    pub amount: Money,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BetEvent {
    Settled {
        ticket_ref: String,
        /// won | lost | void
        result: String,
        payout: Money,
    },
    CashedOut {
        ticket_ref: String,
        amount: Money,
    },
}

#[async_trait]
pub trait SportsbookProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn sports(&self) -> Result<Vec<SportInfo>, WalletError>;
    async fn markets(&self, sport: &str) -> Result<Vec<Market>, WalletError>;
    async fn place_bet(&self, bet: BetRequest) -> Result<BetTicket, WalletError>;
    async fn cash_out(&self, ticket_ref: &str) -> Result<CashOutQuote, WalletError>;
    fn parse_callback(&self, raw: &RawCallback) -> Result<BetEvent, WalletError>;
}
