use crate::auth::{MaybeSession, Player};
use crate::error::ApiResult;
use crate::session;
use crate::state::Shared;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderMap;
use axum::Json;
use domain::traits::PlayerRef;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct LoginBody {
    pub display_name: String,
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String {
    "en".into()
}

fn set_session(state: &Shared, sess: &session::Session) -> HeaderMap {
    let token = session::issue(state.cfg.session_secret.as_bytes(), sess);
    let mut headers = HeaderMap::new();
    if let Ok(v) = session::set_cookie(&token, state.cfg.cookie_secure).parse() {
        headers.insert(SET_COOKIE, v);
    }
    headers
}

/// Dev login: creates the player on first use and sets the session cookie.
/// No password; this is a demo. The operator role, if any, is kept.
pub async fn login(
    State(state): State<Shared>,
    MaybeSession(existing): MaybeSession,
    Json(body): Json<LoginBody>,
) -> ApiResult<(HeaderMap, Json<PlayerRef>)> {
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
    let ops = existing.map(|s| s.ops).unwrap_or(false);
    let sess = session::new_session(Some(player.id.0), ops);
    Ok((set_session(&state, &sess), Json(player)))
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

pub async fn logout(State(state): State<Shared>) -> (HeaderMap, Json<serde_json::Value>) {
    let mut headers = HeaderMap::new();
    if let Ok(v) = session::clear_cookie(state.cfg.cookie_secure).parse() {
        headers.insert(SET_COOKIE, v);
    }
    (headers, Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct OpsLoginBody {
    pub key: String,
}

/// Grants the operator role on the current session when the key matches.
pub async fn ops_login(
    State(state): State<Shared>,
    MaybeSession(existing): MaybeSession,
    Json(body): Json<OpsLoginBody>,
) -> ApiResult<(HeaderMap, Json<serde_json::Value>)> {
    if !domain::sig::constant_time_eq(body.key.trim().as_bytes(), state.cfg.ops_api_key.as_bytes())
    {
        return Err(domain::WalletError::InvalidSignature.into());
    }
    let pid = existing.and_then(|s| s.pid);
    let sess = session::new_session(pid, true);
    Ok((set_session(&state, &sess), Json(json!({ "ops": true }))))
}

/// Who am I: player (if any) and operator flag. Never 401, so the app can boot.
pub async fn whoami(
    State(state): State<Shared>,
    MaybeSession(sess): MaybeSession,
) -> Json<serde_json::Value> {
    let Some(sess) = sess else {
        return Json(json!({ "player": null, "ops": false }));
    };
    let player = match sess.pid {
        Some(pid) => state.store.player_by_id(domain::PlayerId(pid)).await.ok(),
        None => None,
    };
    Json(json!({ "player": player, "ops": sess.ops }))
}
