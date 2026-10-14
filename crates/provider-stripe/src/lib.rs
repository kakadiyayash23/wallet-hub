//! Stripe test mode: Checkout for deposits, signed webhooks for confirmation.
//!
//! Withdrawals: paying a customer out needs Stripe Connect, which is out of
//! scope. `start_withdrawal` returns `pending_review` and the operator settles
//! it by hand. The ledger still reserves the money at request time.

use async_trait::async_trait;
use domain::traits::{
    DepositRedirect, DepositRequest, PaymentEvent, PaymentProvider, RawCallback, WithdrawalHandle,
    WithdrawalRequest,
};
use domain::{Money, WalletError};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct StripeConfig {
    pub secret_key: String,
    pub webhook_secret: String,
    /// Seconds a webhook timestamp may differ from now. Stripe recommends 300.
    pub tolerance_secs: i64,
}

pub struct Stripe {
    cfg: StripeConfig,
    http: reqwest::Client,
}

impl Stripe {
    pub fn new(cfg: StripeConfig, http: reqwest::Client) -> Self {
        Self { cfg, http }
    }
}

#[derive(Debug, Deserialize)]
struct CheckoutSession {
    id: String,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StripeErrorBody {
    error: StripeError,
}

#[derive(Debug, Deserialize)]
struct StripeError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct Event {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    data: EventData,
}

#[derive(Debug, Deserialize)]
struct EventData {
    object: serde_json::Value,
}

#[async_trait]
impl PaymentProvider for Stripe {
    fn id(&self) -> &'static str {
        "stripe"
    }

    async fn start_deposit(&self, req: DepositRequest) -> Result<DepositRedirect, WalletError> {
        let currency = req.currency.to_lowercase();
        let amount = req.amount.minor().to_string();
        let payment_id = req.payment_id.to_string();
        let form: Vec<(&str, &str)> = vec![
            ("mode", "payment"),
            ("success_url", &req.success_url),
            ("cancel_url", &req.cancel_url),
            ("client_reference_id", &payment_id),
            ("metadata[payment_id]", &payment_id),
            ("metadata[player_token]", &req.player.external_token),
            ("line_items[0][price_data][currency]", &currency),
            ("line_items[0][price_data][unit_amount]", &amount),
            (
                "line_items[0][price_data][product_data][name]",
                "Wallet deposit",
            ),
            ("line_items[0][quantity]", "1"),
        ];
        let res = self
            .http
            .post("https://api.stripe.com/v1/checkout/sessions")
            .bearer_auth(&self.cfg.secret_key)
            .form(&form)
            .send()
            .await
            .map_err(|e| WalletError::provider(format!("stripe checkout: {e}")))?;
        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            let msg = serde_json::from_str::<StripeErrorBody>(&text)
                .map(|b| b.error.message)
                .unwrap_or(text);
            return Err(WalletError::provider(format!("stripe checkout: {msg}")));
        }
        let session: CheckoutSession = res
            .json()
            .await
            .map_err(|e| WalletError::provider(format!("stripe checkout json: {e}")))?;
        let url = session
            .url
            .ok_or_else(|| WalletError::provider("stripe returned no checkout url"))?;
        Ok(DepositRedirect {
            provider_ref: session.id,
            redirect_url: url,
        })
    }

    async fn start_withdrawal(
        &self,
        _req: WithdrawalRequest,
    ) -> Result<WithdrawalHandle, WalletError> {
        Ok(WithdrawalHandle {
            provider_ref: None,
            status: "pending_review".into(),
        })
    }

    fn parse_webhook(&self, raw: &RawCallback) -> Result<PaymentEvent, WalletError> {
        let header = raw
            .header("stripe-signature")
            .ok_or(WalletError::InvalidSignature)?;
        verify_signature(
            &self.cfg.webhook_secret,
            header,
            &raw.body,
            self.cfg.tolerance_secs,
        )?;

        let event: Event = serde_json::from_slice(&raw.body)
            .map_err(|e| WalletError::bad_request(format!("stripe event: {e}")))?;
        let obj = &event.data.object;
        let provider_ref = obj
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let payment_id = obj
            .get("metadata")
            .and_then(|m| m.get("payment_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        Ok(match event.kind.as_str() {
            "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
                let paid = obj.get("payment_status").and_then(|v| v.as_str()) == Some("paid");
                if !paid {
                    return Ok(PaymentEvent::Ignored {
                        event_id: event.id,
                        event_type: format!("{} (unpaid)", event.kind),
                    });
                }
                let amount_total = obj
                    .get("amount_total")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let currency = obj
                    .get("currency")
                    .and_then(|v| v.as_str())
                    .unwrap_or("eur")
                    .to_uppercase();
                PaymentEvent::DepositCompleted {
                    event_id: event.id,
                    provider_ref,
                    payment_id,
                    amount: Money::from_minor(amount_total),
                    currency,
                }
            }
            "checkout.session.expired" | "checkout.session.async_payment_failed" => {
                PaymentEvent::DepositFailed {
                    event_id: event.id,
                    provider_ref,
                    payment_id,
                }
            }
            other => PaymentEvent::Ignored {
                event_id: event.id,
                event_type: other.to_string(),
            },
        })
    }
}

/// `Stripe-Signature: t=<unix>,v1=<hex>[,v1=<hex>...]`, signed payload is `"{t}.{body}"`.
pub fn verify_signature(
    secret: &str,
    header: &str,
    body: &[u8],
    tolerance_secs: i64,
) -> Result<(), WalletError> {
    let mut timestamp: Option<i64> = None;
    let mut sigs: Vec<&str> = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", v)) => timestamp = v.parse().ok(),
            Some(("v1", v)) => sigs.push(v),
            _ => {}
        }
    }
    let t = timestamp.ok_or(WalletError::InvalidSignature)?;
    if sigs.is_empty() {
        return Err(WalletError::InvalidSignature);
    }
    let now = chrono::Utc::now().timestamp();
    if tolerance_secs > 0 && (now - t).abs() > tolerance_secs {
        return Err(WalletError::InvalidSignature);
    }
    let mut payload = Vec::with_capacity(body.len() + 16);
    payload.extend_from_slice(t.to_string().as_bytes());
    payload.push(b'.');
    payload.extend_from_slice(body);
    if sigs
        .iter()
        .any(|s| domain::sig::verify_hex(secret.as_bytes(), &payload, s))
    {
        Ok(())
    } else {
        Err(WalletError::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_for(secret: &str, t: i64, body: &[u8]) -> String {
        let payload = format!("{t}.{}", String::from_utf8_lossy(body));
        format!(
            "t={t},v1={}",
            domain::sig::hmac_sha256_hex(secret.as_bytes(), payload.as_bytes())
        )
    }

    #[test]
    fn accepts_a_fresh_valid_signature() {
        let body = br#"{"id":"evt_1","type":"x","data":{"object":{}}}"#;
        let now = chrono::Utc::now().timestamp();
        let h = header_for("whsec_test", now, body);
        assert!(verify_signature("whsec_test", &h, body, 300).is_ok());
    }

    #[test]
    fn rejects_old_or_tampered() {
        let body = br#"{"id":"evt_1"}"#;
        let now = chrono::Utc::now().timestamp();
        let h = header_for("whsec_test", now - 1000, body);
        assert!(verify_signature("whsec_test", &h, body, 300).is_err());
        let h = header_for("whsec_test", now, body);
        assert!(verify_signature("whsec_test", &h, br#"{"id":"evt_2"}"#, 300).is_err());
        assert!(verify_signature("other", &h, body, 300).is_err());
    }

    #[test]
    fn parses_a_completed_checkout() {
        let cfg = StripeConfig {
            secret_key: "sk".into(),
            webhook_secret: "whsec".into(),
            tolerance_secs: 0,
        };
        let stripe = Stripe::new(cfg, reqwest::Client::new());
        let body = serde_json::json!({
            "id": "evt_123", "type": "checkout.session.completed",
            "data": {"object": {"id": "cs_test_1", "payment_status": "paid", "amount_total": 5000,
                                "currency": "eur", "metadata": {"payment_id": "6f1b4c2e-8d1a-4f0a-9c3b-1a2b3c4d5e6f"}}}
        })
        .to_string();
        let h = header_for("whsec", 1, body.as_bytes());
        let mut headers = std::collections::HashMap::new();
        headers.insert("stripe-signature".to_string(), h);
        let raw = RawCallback {
            method: "POST".into(),
            path: "/webhooks/stripe".into(),
            query: Default::default(),
            headers,
            body: body.into_bytes(),
        };
        match stripe.parse_webhook(&raw).unwrap() {
            PaymentEvent::DepositCompleted {
                amount,
                currency,
                provider_ref,
                payment_id,
                ..
            } => {
                assert_eq!(amount.minor(), 5000);
                assert_eq!(currency, "EUR");
                assert_eq!(provider_ref, "cs_test_1");
                assert!(payment_id.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
