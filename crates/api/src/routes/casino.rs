use crate::auth::Player;
use crate::error::ApiResult;
use crate::state::Shared;
use axum::extract::{Query, State};
use axum::Json;
use domain::traits::{GameProvider, LaunchMode, LaunchUrl};
use domain::WalletError;
use ledger::games::{CategoryCount, Game};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct GamesQuery {
    pub category: Option<String>,
    pub search: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    60
}

pub async fn list_games(
    State(state): State<Shared>,
    Query(q): Query<GamesQuery>,
) -> ApiResult<Json<Vec<Game>>> {
    let limit = q.limit.clamp(1, 500);
    let search = q.search.as_deref().map(str::trim).filter(|s| !s.is_empty());
    Ok(Json(
        state
            .store
            .games(q.category.as_deref(), search, limit, q.offset.max(0))
            .await?,
    ))
}

pub async fn categories(State(state): State<Shared>) -> ApiResult<Json<Vec<CategoryCount>>> {
    Ok(Json(state.store.categories().await?))
}

#[derive(Deserialize)]
pub struct LaunchBody {
    pub game_id: i64,
    #[serde(default)]
    pub demo: bool,
}

pub async fn launch(
    State(state): State<Shared>,
    Player(player): Player,
    Json(body): Json<LaunchBody>,
) -> ApiResult<Json<LaunchUrl>> {
    let game = state
        .store
        .game(body.game_id)
        .await?
        .ok_or_else(|| WalletError::bad_request("unknown game"))?;
    let mode = if body.demo {
        LaunchMode::Demo
    } else {
        LaunchMode::Real
    };
    let return_url = format!("{}/lobby", state.cfg.web_origin());
    let url = state
        .bigbang
        .launch(&player, &game.provider_game_id, mode, &return_url)
        .await?;
    Ok(Json(url))
}
