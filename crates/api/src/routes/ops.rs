use crate::auth::Ops;
use crate::callbacks;
use crate::chaos::{self, ChaosRequest};
use crate::error::ApiResult;
use crate::state::Shared;
use axum::extract::{Path, Query, State};
use axum::Json;
use domain::traits::{GameProvider, ProviderHistoryRow};
use domain::{AccountEnv, Money, PlayerId, Source};
use ledger::events::{EventStats, InboundEvent};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct EventsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub source: Option<String>,
}

fn default_limit() -> i64 {
    100
}

pub async fn events(
    State(state): State<Shared>,
    _ops: Ops,
    Query(q): Query<EventsQuery>,
) -> ApiResult<Json<Vec<InboundEvent>>> {
    Ok(Json(
        state
            .store
            .events(q.limit.clamp(1, 500), q.source.as_deref())
            .await?,
    ))
}

pub async fn event_stats(State(state): State<Shared>, _ops: Ops) -> ApiResult<Json<EventStats>> {
    Ok(Json(state.store.event_stats().await?))
}

/// Re-run a stored callback through the same handler. The replay is recorded
/// as a new event that points at the original.
pub async fn replay(
    State(state): State<Shared>,
    _ops: Ops,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let ev = state.store.event(id).await?;
    let raw = callbacks::raw_from_event(&ev);
    let reply = match ev.source.as_str() {
        "bigbang" => callbacks::handle_bigbang(&state, raw, Some(id)).await,
        "stripe" => callbacks::handle_stripe(&state, raw, Some(id)).await,
        "sports" => callbacks::handle_sports(&state, raw, Some(id)).await,
        other => {
            return Err(
                domain::WalletError::bad_request(format!("cannot replay source {other}")).into(),
            )
        }
    };
    Ok(Json(json!({ "status": reply.status, "body": reply.body })))
}

#[derive(Serialize)]
pub struct Reconciliation {
    pub player_id: PlayerId,
    pub env: AccountEnv,
    pub provider_net: Money,
    pub ledger_net: Money,
    pub drift: Money,
    pub provider_rows: usize,
    pub rows: Vec<ProviderHistoryRow>,
}

/// Compare what BigBang says happened with what the ledger recorded.
pub async fn reconcile(
    State(state): State<Shared>,
    _ops: Ops,
    Path(player_id): Path<Uuid>,
) -> ApiResult<Json<Reconciliation>> {
    let player = state.store.player_by_id(PlayerId(player_id)).await?;
    let rows = state.bigbang.history(&player).await?;
    let mut provider_net = Money::ZERO;
    for r in &rows {
        provider_net = provider_net
            .checked_add(r.win)
            .and_then(|m| m.checked_sub(r.bet))
            .map_err(anyhow::Error::from)?;
    }
    let env = state.cfg.bigbang_env;
    let ledger_net = state
        .store
        .net_by_source(player.id, Source::Bigbang, env)
        .await?;
    let drift = ledger_net
        .checked_sub(provider_net)
        .map_err(anyhow::Error::from)?;
    Ok(Json(Reconciliation {
        player_id: player.id,
        env,
        provider_net,
        ledger_net,
        drift,
        provider_rows: rows.len(),
        rows,
    }))
}

pub async fn catalog_sync(
    State(state): State<Shared>,
    _ops: Ops,
) -> ApiResult<Json<serde_json::Value>> {
    let listings = state.bigbang.sync_catalog().await?;
    let n = state.store.upsert_games("bigbang", &listings).await?;
    Ok(Json(json!({ "synced": n })))
}

pub async fn chaos(
    State(state): State<Shared>,
    _ops: Ops,
    Json(req): Json<ChaosRequest>,
) -> ApiResult<Json<chaos::ChaosReport>> {
    Ok(Json(chaos::run(&state, req).await?))
}

pub async fn trial_balance(
    State(state): State<Shared>,
    _ops: Ops,
) -> ApiResult<Json<serde_json::Value>> {
    let sum = state.store.trial_balance().await?;
    Ok(Json(
        json!({ "sum_of_all_entries": sum, "balanced": sum == 0 }),
    ))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PlayerRow {
    pub id: Uuid,
    pub external_token: String,
    pub display_name: String,
}

pub async fn players(State(state): State<Shared>, _ops: Ops) -> ApiResult<Json<Vec<PlayerRow>>> {
    let rows = sqlx::query_as::<_, PlayerRow>(
        "select id, external_token, display_name from players order by created_at desc limit 100",
    )
    .fetch_all(&state.store.pool)
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(rows))
}
