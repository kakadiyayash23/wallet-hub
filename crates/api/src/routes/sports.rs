use crate::auth::Player;
use crate::error::ApiResult;
use crate::live::LiveEvent;
use crate::state::Shared;
use axum::extract::{Path, Query, State};
use axum::Json;
use domain::traits::{BetRequest, CashOutQuote, Market, SportInfo, SportsbookProvider};
use domain::{AccountEnv, Money, MoveKind, Source, WalletCommand, WalletError};
use ledger::bets::{Bet, NewBet};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct MarketsQuery {
    #[serde(default = "default_sport")]
    pub sport: String,
}

fn default_sport() -> String {
    "soccer".into()
}

pub async fn sports(State(state): State<Shared>) -> ApiResult<Json<Vec<SportInfo>>> {
    Ok(Json(state.sports.sports().await?))
}

pub async fn markets(
    State(state): State<Shared>,
    Query(q): Query<MarketsQuery>,
) -> ApiResult<Json<Vec<Market>>> {
    Ok(Json(state.sports.markets(&q.sport).await?))
}

pub async fn list_bets(
    State(state): State<Shared>,
    Player(p): Player,
) -> ApiResult<Json<Vec<Bet>>> {
    Ok(Json(state.store.bets(p.id, 100).await?))
}

#[derive(Deserialize)]
pub struct PlaceBetBody {
    pub event_id: String,
    pub event_name: String,
    /// home | draw | away
    pub selection: String,
    /// Label shown on the ticket, for example the team name.
    #[serde(default)]
    pub selection_label: String,
    pub odds_milli: i64,
    /// Decimal string, e.g. "10.00"
    pub stake: String,
}

pub async fn place_bet(
    State(state): State<Shared>,
    Player(p): Player,
    Json(body): Json<PlaceBetBody>,
) -> ApiResult<Json<Bet>> {
    let stake = Money::from_decimal_str(&body.stake)
        .map_err(|e| WalletError::bad_request(e.to_string()))?;
    if stake.minor() < 100 || stake.minor() > 1_000_000 {
        return Err(WalletError::bad_request("stake must be between 1.00 and 10,000.00").into());
    }
    if body.odds_milli < 1001 || body.odds_milli > 1_000_000 {
        return Err(WalletError::bad_request("odds out of range").into());
    }
    let bet_id = Uuid::new_v4();

    // 1. Reserve the stake. Insufficient funds stops here with 409.
    let debit = state
        .store
        .apply(&WalletCommand::Move {
            player_token: p.external_token.clone(),
            currency: p.currency.clone(),
            env: AccountEnv::Live,
            source: Source::Sports,
            idempotency_key: format!("bet:{bet_id}"),
            kind: MoveKind::Bet,
            amount: Money::from_minor(-stake.minor()),
            round: Some(domain::RoundRef {
                round_id: bet_id.to_string(),
                round_end: false,
            }),
            meta: json!({ "event": body.event_name, "selection": body.selection }),
        })
        .await?;
    state.publish(LiveEvent::from_outcome(
        &debit,
        "sports",
        "bet",
        Money::from_minor(-stake.minor()),
    ));

    let bet = state
        .store
        .create_bet(NewBet {
            id: bet_id,
            player: p.id,
            event_name: &body.event_name,
            market: "h2h",
            selection: if body.selection_label.is_empty() {
                &body.selection
            } else {
                &body.selection_label
            },
            stake,
            odds_milli: body.odds_milli,
            currency: &p.currency,
        })
        .await?;

    // 2. Hand it to the sportsbook. If it refuses, release the stake.
    let placed = state
        .sports
        .place_bet(BetRequest {
            bet_id,
            player_token: p.external_token.clone(),
            event_id: body.event_id.clone(),
            selection: body.selection.clone(),
            stake,
            odds_milli: body.odds_milli,
            currency: p.currency.clone(),
        })
        .await;
    match placed {
        Ok(ticket) => {
            state
                .store
                .set_bet_ticket(bet_id, &ticket.ticket_ref, ticket.odds_milli)
                .await?;
            Ok(Json(state.store.bet(bet_id).await?.unwrap_or(bet)))
        }
        Err(e) => {
            let rb = state
                .store
                .apply(&WalletCommand::Rollback {
                    player_token: p.external_token.clone(),
                    currency: p.currency.clone(),
                    env: AccountEnv::Live,
                    source: Source::Sports,
                    original_key: format!("bet:{bet_id}"),
                })
                .await?;
            state.publish(LiveEvent::from_outcome(&rb, "sports", "rollback", stake));
            state.store.set_bet_status(bet_id, "rejected").await?;
            Err(e.into())
        }
    }
}

pub async fn cash_out(
    State(state): State<Shared>,
    Player(p): Player,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<CashOutQuote>> {
    let bet = state
        .store
        .bet(id)
        .await?
        .ok_or_else(|| WalletError::bad_request("unknown bet"))?;
    if bet.player_id != p.id.0 {
        return Err(WalletError::bad_request("not your bet").into());
    }
    if bet.status != "open" {
        return Err(WalletError::bad_request("bet is not open").into());
    }
    let ticket = bet
        .ticket_ref
        .ok_or_else(|| WalletError::bad_request("bet has no ticket"))?;
    // The sportsbook confirms with a signed callback; the money moves there.
    Ok(Json(state.sports.cash_out(&ticket).await?))
}
