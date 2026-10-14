//! Extractors. A player needs a session cookie with a player id. The operator
//! role comes from the same cookie after `POST /api/ops/login`, or from the
//! `X-Ops-Key` header for scripts.

use crate::session;
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

pub const OPS_HEADER: HeaderName = HeaderName::from_static("x-ops-key");

pub struct Player(pub PlayerRef);

/// The raw session, if the cookie is valid. Never fails.
pub struct MaybeSession(pub Option<session::Session>);

fn unauthorized(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": { "code": "unauthorized", "message": msg } })),
    )
        .into_response()
}

#[async_trait]
impl FromRequestParts<Shared> for MaybeSession {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        Ok(MaybeSession(session::from_headers(
            state.cfg.session_secret.as_bytes(),
            &parts.headers,
        )))
    }
}

#[async_trait]
impl FromRequestParts<Shared> for Player {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        let sess = session::from_headers(state.cfg.session_secret.as_bytes(), &parts.headers)
            .ok_or_else(|| unauthorized("not logged in"))?;
        let pid = sess.pid.ok_or_else(|| unauthorized("not logged in"))?;
        let player = state
            .store
            .player_by_id(PlayerId(pid))
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
        if let Some(sess) =
            session::from_headers(state.cfg.session_secret.as_bytes(), &parts.headers)
        {
            if sess.ops {
                return Ok(Ops);
            }
        }
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
            Err(unauthorized("operator login required"))
        }
    }
}
