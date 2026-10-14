use crate::auth::Player;
use crate::error::ApiResult;
use crate::live::LiveEvent;
use crate::state::Shared;
use axum::extract::{Query, State};
use axum::Json;
use domain::traits::{DepositRequest, PaymentProvider, WithdrawalRequest};
use domain::{AccountEnv, Money, MoveKind, Source, WalletCommand, WalletError};
use ledger::ledger::{AccountView, JournalView};
use ledger::payments::{NewPayment, Payment};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Summary {
    pub accounts: Vec<AccountView>,
    pub currency: String,
}

pub async fn summary(State(state): State<Shared>, Player(p): Player) -> ApiResult<Json<Summary>> {
    // Make sure both accounts exist so the UI can show them before any movement.
    for env in [AccountEnv::Live, AccountEnv::Sandbox] {
        state
            .store
            .apply(&WalletCommand::ReadBalance {
                player_token: p.external_token.clone(),
                currency: p.currency.clone(),
                env,
            })
            .await?;
    }
    Ok(Json(Summary {
        accounts: state.store.accounts(p.id).await?,
        currency: p.currency,
    }))
}

#[derive(Deserialize)]
pub struct LimitQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

pub async fn ledger(
    State(state): State<Shared>,
    Player(p): Player,
    Query(q): Query<LimitQuery>,
) -> ApiResult<Json<Vec<JournalView>>> {
    Ok(Json(
        state.store.journals(p.id, q.limit.clamp(1, 500)).await?,
    ))
}

pub async fn payments(
    State(state): State<Shared>,
    Player(p): Player,
    Query(q): Query<LimitQuery>,
) -> ApiResult<Json<Vec<Payment>>> {
    Ok(Json(
        state.store.payments(p.id, q.limit.clamp(1, 200)).await?,
    ))
}

#[derive(Deserialize)]
pub struct AmountBody {
    /// Decimal string, e.g. "50.00". Strings keep floats out of the API.
    pub amount: String,
}

fn parse_amount(s: &str) -> Result<Money, WalletError> {
    let m = Money::from_decimal_str(s).map_err(|e| WalletError::bad_request(e.to_string()))?;
    if m.minor() < 100 {
        return Err(WalletError::bad_request("minimum amount is 1.00"));
    }
    if m.minor() > 100_000_000 {
        return Err(WalletError::bad_request("maximum amount is 1,000,000.00"));
    }
    Ok(m)
}

pub async fn deposit(
    State(state): State<Shared>,
    Player(p): Player,
    Json(body): Json<AmountBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let amount = parse_amount(&body.amount)?;
    let payment_id = Uuid::new_v4();
    state
        .store
        .create_payment(NewPayment {
            id: payment_id,
            player: p.id,
            provider: "stripe",
            direction: "deposit",
            amount,
            currency: &p.currency,
            status: "pending",
        })
        .await?;
    let origin = state.cfg.web_origin();
    let redirect = state
        .stripe
        .start_deposit(DepositRequest {
            payment_id,
            player: p.clone(),
            amount,
            currency: p.currency.clone(),
            success_url: format!("{origin}/wallet?deposit=success"),
            cancel_url: format!("{origin}/wallet?deposit=cancel"),
        })
        .await;
    let redirect = match redirect {
        Ok(r) => r,
        Err(e) => {
            state.store.set_payment_status(payment_id, "failed").await?;
            return Err(e.into());
        }
    };
    state
        .store
        .set_payment_ref(payment_id, &redirect.provider_ref)
        .await?;
    Ok(Json(
        json!({ "payment_id": payment_id, "redirect_url": redirect.redirect_url }),
    ))
}

pub async fn withdraw(
    State(state): State<Shared>,
    Player(p): Player,
    Json(body): Json<AmountBody>,
) -> ApiResult<Json<Payment>> {
    let amount = parse_amount(&body.amount)?;
    let payment_id = Uuid::new_v4();
    // Reserve the money first. If the balance is too low this returns 409 and nothing is written.
    let outcome = state
        .store
        .apply(&WalletCommand::Move {
            player_token: p.external_token.clone(),
            currency: p.currency.clone(),
            env: AccountEnv::Live,
            source: Source::Stripe,
            idempotency_key: format!("withdrawal:{payment_id}"),
            kind: MoveKind::Withdrawal,
            amount: amount
                .checked_neg()
                .map_err(|e| WalletError::bad_request(e.to_string()))?,
            round: None,
            meta: json!({ "payment_id": payment_id }),
        })
        .await?;
    let handle = state
        .stripe
        .start_withdrawal(WithdrawalRequest {
            payment_id,
            player: p.clone(),
            amount,
            currency: p.currency.clone(),
        })
        .await?;
    let payment = state
        .store
        .create_payment(NewPayment {
            id: payment_id,
            player: p.id,
            provider: "stripe",
            direction: "withdrawal",
            amount,
            currency: &p.currency,
            status: &handle.status,
        })
        .await?;
    state.publish(LiveEvent::from_outcome(
        &outcome,
        "stripe",
        "withdrawal",
        Money::from_minor(-amount.minor()),
    ));
    Ok(Json(payment))
}

/// Demo only: credit the sandbox account so seamless-wallet spins with a
/// sandbox key have something to bet. Never touches the live account.
pub async fn sandbox_topup(
    State(state): State<Shared>,
    Player(p): Player,
    Json(body): Json<AmountBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let amount = parse_amount(&body.amount)?;
    let outcome = state
        .store
        .apply(&WalletCommand::Move {
            player_token: p.external_token.clone(),
            currency: p.currency.clone(),
            env: AccountEnv::Sandbox,
            source: Source::Chaos,
            idempotency_key: format!("topup:{}", Uuid::new_v4()),
            kind: MoveKind::Deposit,
            amount,
            round: None,
            meta: json!({ "reason": "sandbox top-up from wallet page" }),
        })
        .await?;
    state.publish(LiveEvent::from_outcome(
        &outcome, "sandbox", "deposit", amount,
    ));
    Ok(Json(
        json!({ "balance": outcome.balance, "env": "sandbox" }),
    ))
}
