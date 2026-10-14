//! BigBang Casino API: catalog, launch, and the seamless wallet callbacks.
//!
//! Wire contract (from https://api.bigbangcasino.bet/docs/):
//! * `GET  <user_data URL>?username=…` -> `{username, balance, currency}`
//! * `POST <balance_change URL>` JSON body -> `{status: "ok", balance}`
//! * `signature` = HMAC-SHA256(username + amount + game + game_category + transaction_id, API_KEY)
//!   where `amount` is the number in its shortest JavaScript string form.
//! * `transaction_id` is the idempotency key. `round_id` + `round_end` close rounds.
//! * Sandbox callbacks carry `"sandbox": true`.

mod signature;
mod wire;

use async_trait::async_trait;
use domain::traits::{
    CallbackOutcome, CallbackReply, GameListing, GameProvider, LaunchMode, LaunchUrl, PlayerRef,
    ProviderHistoryRow, RawCallback,
};
use domain::{AccountEnv, Money, MoveKind, RoundRef, Source, WalletCommand, WalletError};
use serde_json::json;

pub use signature::{js_number_string, sign_balance_change};
pub use wire::BalanceChange;

#[derive(Clone, Debug)]
pub struct BigBangConfig {
    pub api_key: String,
    pub base_url: String,
    /// Which key is configured. A `sandbox: true` callback always goes to the sandbox account.
    pub env: AccountEnv,
    /// Callbacks carry no currency, so the platform currency applies.
    pub currency: String,
}

pub struct BigBang {
    cfg: BigBangConfig,
    http: reqwest::Client,
}

impl BigBang {
    pub fn new(cfg: BigBangConfig, http: reqwest::Client) -> Self {
        Self { cfg, http }
    }

    pub fn config(&self) -> &BigBangConfig {
        &self.cfg
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.cfg.base_url.trim_end_matches('/'), path)
    }

    async fn ensure_user(&self, player: &PlayerRef) -> Result<(), WalletError> {
        let res = self
            .http
            .post(self.url("/users/create"))
            .header("X-API-Key", &self.cfg.api_key)
            .json(&json!({ "user_token": player.external_token, "username": player.display_name }))
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("users/create: {e}")))?;
        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(WalletError::provider(format!(
                "users/create failed: {text}"
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl GameProvider for BigBang {
    fn id(&self) -> &'static str {
        "bigbang"
    }

    async fn sync_catalog(&self) -> Result<Vec<GameListing>, WalletError> {
        let mut out = Vec::new();
        let limit = 1000usize;
        let mut offset = 0usize;
        loop {
            let res: wire::GamesResponse = self
                .http
                .get(self.url("/games"))
                .header("X-API-Key", &self.cfg.api_key)
                .query(&[("limit", limit.to_string()), ("offset", offset.to_string())])
                .send()
                .await
                .map_err(|e| WalletError::provider(format!("games: {e}")))?
                .error_for_status()
                .map_err(|e| WalletError::provider(format!("games: {e}")))?
                .json()
                .await
                .map_err(|e| WalletError::provider(format!("games json: {e}")))?;
            let n = res.data.len();
            out.extend(res.data.into_iter().map(GameListing::from));
            if n < limit {
                break;
            }
            offset += limit;
        }
        Ok(out)
    }

    async fn launch(
        &self,
        player: &PlayerRef,
        provider_game_id: &str,
        mode: LaunchMode,
        return_url: &str,
    ) -> Result<LaunchUrl, WalletError> {
        let game_id: i64 = provider_game_id
            .parse()
            .map_err(|_| WalletError::bad_request("bigbang game id must be numeric"))?;
        let body = match mode {
            LaunchMode::Demo => {
                json!({ "game_id": game_id, "demo": true, "language": player.locale })
            }
            LaunchMode::Real => {
                self.ensure_user(player).await?;
                json!({
                    "game_id": game_id,
                    "user_token": player.external_token,
                    "language": player.locale,
                    "return_url": return_url,
                })
            }
        };
        let res = self
            .http
            .post(self.url("/games/launch"))
            .header("X-API-Key", &self.cfg.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("launch: {e}")))?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        let parsed: wire::LaunchResponse = serde_json::from_str(&text)
            .map_err(|e| WalletError::provider(format!("launch ({status}): {e}: {text}")))?;
        match parsed.game_url {
            Some(url) if parsed.success => Ok(LaunchUrl {
                url,
                session_id: parsed.session_id,
            }),
            _ => Err(WalletError::provider(format!(
                "launch failed ({status}): {}",
                parsed
                    .error
                    .map(|e| format!("{} (code {})", e.message, e.code))
                    .unwrap_or(text)
            ))),
        }
    }

    fn parse_callback(&self, raw: &RawCallback) -> Result<WalletCommand, WalletError> {
        if raw.path.ends_with("/user_data") {
            let username = raw
                .query
                .get("username")
                .cloned()
                .ok_or_else(|| WalletError::bad_request("user_data: missing username"))?;
            let (player_token, env) = split_sandbox_prefix(&username, self.cfg.env);
            return Ok(WalletCommand::ReadBalance {
                player_token,
                currency: self.cfg.currency.clone(),
                env,
            });
        }

        let cb: BalanceChange = serde_json::from_slice(&raw.body)
            .map_err(|e| WalletError::bad_request(format!("balance_change: {e}")))?;

        let message = cb.signature_message();
        if !domain::sig::verify_hex(
            self.cfg.api_key.as_bytes(),
            message.as_bytes(),
            &cb.signature,
        ) {
            return Err(WalletError::InvalidSignature);
        }
        // The docs also mention an X-Body-Signature header for the informational
        // fields. Its exact algorithm is not published, so log a mismatch only.
        if let Some(h) = raw.header("x-body-signature") {
            if !domain::sig::verify_hex(self.cfg.api_key.as_bytes(), &raw.body, h) {
                tracing::warn!("bigbang: X-Body-Signature did not match HMAC(api_key, raw body)");
            }
        }

        let amount = cb.amount_money()?;
        let (player_token, prefixed_env) = split_sandbox_prefix(&cb.username, self.cfg.env);
        let env = if cb.sandbox {
            AccountEnv::Sandbox
        } else {
            prefixed_env
        };
        let kind = match cb.kind.as_deref() {
            Some("bet") => MoveKind::Bet,
            Some("win") => MoveKind::Win,
            Some("refund") => MoveKind::Refund,
            _ => MoveKind::NetRound,
        };
        let round = cb.round_id.clone().map(|round_id| RoundRef {
            round_id,
            round_end: cb.round_end.unwrap_or(true),
        });
        Ok(WalletCommand::Move {
            player_token,
            currency: self.cfg.currency.clone(),
            env,
            source: Source::Bigbang,
            idempotency_key: cb.transaction_id.clone(),
            kind,
            amount,
            round,
            meta: json!({
                "game": cb.game,
                "game_category": cb.game_category,
                "game_id": cb.game_id,
                "provider_id": cb.provider_id,
                "bet_amount": cb.bet_amount,
                "win_amount": cb.win_amount,
                "sandbox": cb.sandbox,
            }),
        })
    }

    fn render_reply(&self, raw: &RawCallback, outcome: &CallbackOutcome) -> CallbackReply {
        let is_read = raw.path.ends_with("/user_data");
        match outcome {
            CallbackOutcome::Ok(o) => {
                let body = if is_read {
                    // The docs accept `balance`/`Balance` as string or number; the
                    // portal shows the capitalised numeric form. Send both.
                    let username = raw.query.get("username").cloned().unwrap_or_default();
                    json!({
                        "username": username,
                        "Username": username,
                        "balance": o.balance.to_decimal_string(),
                        "Balance": o.balance.to_f64(),
                        "currency": o.currency,
                    })
                } else if o.duplicate {
                    json!({ "status": "ok", "balance": o.balance.to_decimal_string(), "duplicate": true })
                } else {
                    json!({ "status": "ok", "balance": o.balance.to_decimal_string() })
                };
                CallbackReply { status: 200, body }
            }
            // Contract (agent.md §3.2): a refused bet is a 4xx with the word
            // "insufficient" in `error`; BigBang then refuses the round and does
            // not retry. A 2xx without a balance would be retried for 12 hours.
            CallbackOutcome::InsufficientFunds { balance, .. } => CallbackReply {
                status: 400,
                body: json!({ "error": "insufficient balance", "balance": balance.to_decimal_string() }),
            },
            // Unknown player: 404 on both endpoints (§3.1).
            CallbackOutcome::PlayerNotFound => CallbackReply {
                status: 404,
                body: json!({ "error": "unknown user" }),
            },
            CallbackOutcome::RolledBack { balance, .. } => CallbackReply {
                status: 409,
                body: json!({ "error": "transaction rolled back", "balance": balance.to_decimal_string() }),
            },
        }
    }

    fn render_error(&self, err: &WalletError) -> CallbackReply {
        match err {
            WalletError::InvalidSignature => CallbackReply {
                status: 401,
                body: json!({ "status": "error", "error": "invalid_signature" }),
            },
            WalletError::BadRequest(m) => CallbackReply {
                status: 400,
                body: json!({ "status": "error", "error": "bad_request", "message": m }),
            },
            _ => CallbackReply {
                status: 500,
                body: json!({ "status": "error", "error": "internal" }),
            },
        }
    }

    async fn history(&self, player: &PlayerRef) -> Result<Vec<ProviderHistoryRow>, WalletError> {
        let res: wire::HistoryResponse = self
            .http
            .get(self.url(&format!("/users/{}/history", player.external_token)))
            .header("X-API-Key", &self.cfg.api_key)
            .query(&[("limit", "200")])
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("history: {e}")))?
            .error_for_status()
            .map_err(|e| WalletError::provider(format!("history: {e}")))?
            .json()
            .await
            .map_err(|e| WalletError::provider(format!("history json: {e}")))?;
        res.data
            .into_iter()
            .map(|r| {
                Ok(ProviderHistoryRow {
                    external_id: r.id.to_string(),
                    game: r.game,
                    bet: Money::from_f64(r.bet)
                        .map_err(|e| WalletError::provider(e.to_string()))?,
                    win: Money::from_f64(r.win)
                        .map_err(|e| WalletError::provider(e.to_string()))?,
                    at: r.date_time,
                })
            })
            .collect()
    }
}

/// Sandbox launch URLs carry the player as `sbx_<token>`. If a callback uses
/// that form, strip it and force the sandbox account.
fn split_sandbox_prefix(username: &str, default_env: AccountEnv) -> (String, AccountEnv) {
    match username.strip_prefix("sbx_") {
        Some(rest) if !rest.is_empty() => (rest.to_string(), AccountEnv::Sandbox),
        _ => (username.to_string(), default_env),
    }
}

#[cfg(test)]
mod prefix_tests {
    use super::*;

    #[test]
    fn strips_sandbox_prefix() {
        assert_eq!(
            split_sandbox_prefix("sbx_player_42", AccountEnv::Live),
            ("player_42".into(), AccountEnv::Sandbox)
        );
        assert_eq!(
            split_sandbox_prefix("player_42", AccountEnv::Live),
            ("player_42".into(), AccountEnv::Live)
        );
        assert_eq!(
            split_sandbox_prefix("sbx_", AccountEnv::Live),
            ("sbx_".into(), AccountEnv::Live)
        );
    }
}
