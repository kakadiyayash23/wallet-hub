//! Demo auth. The player app sends `X-Player-Id` after a dev login; the
//! operator app sends `X-Ops-Key`. A real platform puts sessions here.

use crate::state::Shared;
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use domain::traits::PlayerRef;
use domain::PlayerId;
use serde_json::json;
use uuid::Uuid;

pub const PLAYER_HEADER: HeaderName = HeaderName::from_static("x-player-id");
pub const OPS_HEADER: HeaderName = HeaderName::from_static("x-ops-key");

pub struct Player(pub PlayerRef);

fn unauthorized(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": { "code": "unauthorized", "message": msg } })),
    )
        .into_response()
}

#[async_trait]
impl FromRequestParts<Shared> for Player {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get(&PLAYER_HEADER)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| unauthorized("missing X-Player-Id"))?;
        let id = Uuid::parse_str(raw).map_err(|_| unauthorized("X-Player-Id is not a uuid"))?;
        let player = state
            .store
            .player_by_id(PlayerId(id))
            .await
            .map_err(|_| unauthorized("unknown player"))?;
        Ok(Player(player))
    }
}

pub struct Ops;

#[async_trait]
impl FromRequestParts<Shared> for Ops {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        let key = parts
            .headers
            .get(&OPS_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if !key.is_empty()
            && domain::sig::constant_time_eq(key.as_bytes(), state.cfg.ops_api_key.as_bytes())
        {
            Ok(Ops)
        } else {
            Err(unauthorized("missing or wrong X-Ops-Key"))
        }
    }
}
