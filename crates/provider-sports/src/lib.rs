//! Client for the mock sportsbook service in `services/mock-sportsbook`.
//! The wire format is ours, but the shape mirrors a real sportsbook feed:
//! markets, bet placement, cash-out, and signed settlement callbacks.

use async_trait::async_trait;
use domain::traits::{
    BetEvent, BetRequest, BetTicket, CashOutQuote, Market, RawCallback, SportInfo,
    SportsbookProvider,
};
use domain::WalletError;

#[derive(Clone, Debug)]
pub struct SportsConfig {
    pub base_url: String,
    /// Shared secret: callbacks carry `X-Signature: HMAC-SHA256(secret, raw body)`.
    pub callback_secret: String,
}

pub struct Sportsbook {
    cfg: SportsConfig,
    http: reqwest::Client,
}

impl Sportsbook {
    pub fn new(cfg: SportsConfig, http: reqwest::Client) -> Self {
        Self { cfg, http }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.cfg.base_url.trim_end_matches('/'), path)
    }
}

#[async_trait]
impl SportsbookProvider for Sportsbook {
    fn id(&self) -> &'static str {
        "sports"
    }

    async fn sports(&self) -> Result<Vec<SportInfo>, WalletError> {
        self.http
            .get(self.url("/sports"))
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("sports: {e}")))?
            .error_for_status()
            .map_err(|e| WalletError::provider(format!("sports: {e}")))?
            .json()
            .await
            .map_err(|e| WalletError::provider(format!("sports json: {e}")))
    }

    async fn markets(&self, sport: &str) -> Result<Vec<Market>, WalletError> {
        self.http
            .get(self.url("/markets"))
            .query(&[("sport", sport)])
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("markets: {e}")))?
            .error_for_status()
            .map_err(|e| WalletError::provider(format!("markets: {e}")))?
            .json()
            .await
            .map_err(|e| WalletError::provider(format!("markets json: {e}")))
    }

    async fn place_bet(&self, bet: BetRequest) -> Result<BetTicket, WalletError> {
        let res = self
            .http
            .post(self.url("/bets"))
            .json(&bet)
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("place_bet: {e}")))?;
        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(WalletError::provider(format!("place_bet rejected: {text}")));
        }
        res.json()
            .await
            .map_err(|e| WalletError::provider(format!("place_bet json: {e}")))
    }

    async fn cash_out(&self, ticket_ref: &str) -> Result<CashOutQuote, WalletError> {
        let res = self
            .http
            .post(self.url(&format!("/bets/{ticket_ref}/cashout")))
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("cash_out: {e}")))?;
        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(WalletError::provider(format!("cash_out rejected: {text}")));
        }
        res.json()
            .await
            .map_err(|e| WalletError::provider(format!("cash_out json: {e}")))
    }

    fn parse_callback(&self, raw: &RawCallback) -> Result<BetEvent, WalletError> {
        let sig = raw
            .header("x-signature")
            .ok_or(WalletError::InvalidSignature)?;
        if !domain::sig::verify_hex(self.cfg.callback_secret.as_bytes(), &raw.body, sig) {
            return Err(WalletError::InvalidSignature);
        }
        serde_json::from_slice(&raw.body)
            .map_err(|e| WalletError::bad_request(format!("bet event: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn verifies_and_parses_settlement() {
        let sb = Sportsbook::new(
            SportsConfig {
                base_url: "http://x".into(),
                callback_secret: "s3cret".into(),
            },
            reqwest::Client::new(),
        );
        let body = br#"{"type":"settled","ticket_ref":"tk_1","result":"won","payout":2100}"#;
        let mut headers = HashMap::new();
        headers.insert(
            "x-signature".into(),
            domain::sig::hmac_sha256_hex(b"s3cret", body),
        );
        let raw = RawCallback {
            method: "POST".into(),
            path: "/callbacks/sports".into(),
            query: HashMap::new(),
            headers,
            body: body.to_vec(),
        };
        match sb.parse_callback(&raw).unwrap() {
            BetEvent::Settled {
                ticket_ref,
                result,
                payout,
            } => {
                assert_eq!(ticket_ref, "tk_1");
                assert_eq!(result, "won");
                assert_eq!(payout.minor(), 2100);
            }
            _ => panic!(),
        }
        let bad = RawCallback {
            headers: HashMap::new(),
            ..raw
        };
        assert!(matches!(
            sb.parse_callback(&bad),
            Err(WalletError::InvalidSignature)
        ));
    }
}
