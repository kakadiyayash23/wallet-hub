//! JSON shapes on the BigBang wire. Kept separate from the domain types.

use domain::traits::GameListing;
use domain::{Money, WalletError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct GamesResponse {
    #[serde(default)]
    pub data: Vec<GameItem>,
}

#[derive(Debug, Deserialize)]
pub struct GameItem {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub category_title: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub game_type: Option<String>,
    #[serde(default)]
    pub is_premium: bool,
}

impl From<GameItem> for GameListing {
    fn from(g: GameItem) -> Self {
        GameListing {
            provider_game_id: g.id.to_string(),
            title: g.title.clone().unwrap_or_else(|| g.name.clone()),
            name: g.name,
            category: g
                .category_title
                .or(g.provider)
                .or(g.category)
                .unwrap_or_else(|| "Other".into()),
            thumbnail: g.thumbnail,
            game_type: g.game_type.unwrap_or_else(|| "slot".into()),
            is_premium: g.is_premium,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LaunchResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub game_url: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoryResponse {
    #[serde(default)]
    pub data: Vec<HistoryItem>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryItem {
    pub id: i64,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub bet: f64,
    #[serde(default)]
    pub win: f64,
    #[serde(default)]
    pub date_time: String,
}

/// The `balance_change` callback body. `amount` stays a raw JSON value so the
/// signature can be computed from the exact number the provider sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceChange {
    pub username: String,
    pub amount: Value,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub game_category: String,
    pub transaction_id: String,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub round_id: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub round_end: Option<bool>,
    #[serde(default)]
    pub game_id: Option<i64>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub bet_amount: Option<f64>,
    #[serde(default)]
    pub win_amount: Option<f64>,
    #[serde(default)]
    pub sandbox: bool,
}

impl BalanceChange {
    /// `username + amount + game + game_category + transaction_id`
    pub fn signature_message(&self) -> String {
        format!(
            "{}{}{}{}{}",
            self.username,
            crate::signature::js_value_string(&self.amount),
            self.game,
            self.game_category,
            self.transaction_id
        )
    }

    pub fn amount_money(&self) -> Result<Money, WalletError> {
        let parsed = match &self.amount {
            Value::Number(n) => n
                .as_f64()
                .ok_or_else(|| WalletError::bad_request("amount is not a number"))
                .and_then(|f| {
                    Money::from_f64(f).map_err(|e| WalletError::bad_request(e.to_string()))
                }),
            Value::String(s) => {
                Money::from_decimal_str(s).map_err(|e| WalletError::bad_request(e.to_string()))
            }
            _ => Err(WalletError::bad_request("amount has an unexpected type")),
        }?;
        Ok(parsed)
    }
}
