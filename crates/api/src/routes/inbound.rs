//! Provider-facing routes. They capture the raw request and hand it to
//! `callbacks`, which stores it first and answers in the provider's format.

use crate::callbacks;
use crate::state::Shared;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use domain::traits::CallbackReply;

fn to_response(reply: CallbackReply) -> Response {
    let status = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(reply.body)).into_response()
}

pub async fn bigbang(
    State(state): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let raw = callbacks::raw_from_http(&method, &uri, &headers, body);
    to_response(callbacks::handle_bigbang(&state, raw, None).await)
}

pub async fn stripe(
    State(state): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let raw = callbacks::raw_from_http(&method, &uri, &headers, body);
    to_response(callbacks::handle_stripe(&state, raw, None).await)
}

pub async fn sports(
    State(state): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let raw = callbacks::raw_from_http(&method, &uri, &headers, body);
    to_response(callbacks::handle_sports(&state, raw, None).await)
}
