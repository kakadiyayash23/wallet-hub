use crate::auth::Player;
use crate::error::ApiResult;
use crate::state::Shared;
use axum::extract::State;
use axum::Json;
use domain::traits::PlayerRef;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginBody {
    pub display_name: String,
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String {
    "en".into()
}

/// Dev login: creates the player on first use. No password; this is a demo.
pub async fn login(
    State(state): State<Shared>,
    Json(body): Json<LoginBody>,
) -> ApiResult<Json<PlayerRef>> {
    let name = body.display_name.trim();
    if name.len() < 2 || name.len() > 40 {
        return Err(
            domain::WalletError::bad_request("display_name must be 2-40 characters").into(),
        );
    }
    let locale = if body.locale == "de" { "de" } else { "en" };
    let player = state
        .store
        .ensure_player(name, locale, &state.cfg.platform_currency)
        .await?;
    Ok(Json(player))
}

pub async fn me(Player(p): Player) -> Json<PlayerRef> {
    Json(p)
}

#[derive(Deserialize)]
pub struct UpdateBody {
    pub locale: String,
}

pub async fn update_me(
    State(state): State<Shared>,
    Player(p): Player,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<PlayerRef>> {
    let locale = if body.locale == "de" { "de" } else { "en" };
    state.store.set_locale(p.id, locale).await?;
    Ok(Json(state.store.player_by_id(p.id).await?))
}
