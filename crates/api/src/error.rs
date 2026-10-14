use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use domain::WalletError;
use serde_json::json;

/// JSON error envelope for the Vue app. Provider callbacks render their own replies.
pub struct ApiError(pub WalletError);

impl From<WalletError> for ApiError {
    fn from(e: WalletError) -> Self {
        ApiError(e)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError(WalletError::Internal(e))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, extra) = match &self.0 {
            WalletError::PlayerNotFound => (StatusCode::NOT_FOUND, "player_not_found", json!({})),
            WalletError::InsufficientFunds { balance } => (
                StatusCode::CONFLICT,
                "insufficient_funds",
                json!({ "balance": balance }),
            ),
            WalletError::InvalidSignature => {
                (StatusCode::UNAUTHORIZED, "invalid_signature", json!({}))
            }
            WalletError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request", json!({})),
            WalletError::RolledBack(_) => (StatusCode::CONFLICT, "rolled_back", json!({})),
            WalletError::Provider(_) => (StatusCode::BAD_GATEWAY, "provider_error", json!({})),
            WalletError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", json!({}))
            }
        };
        let message = match &self.0 {
            WalletError::Internal(_) => "internal error".to_string(),
            e => e.to_string(),
        };
        let mut body = json!({ "error": { "code": code, "message": message } });
        if let (Some(obj), Some(extra)) = (body["error"].as_object_mut(), extra.as_object()) {
            for (k, v) in extra {
                obj.insert(k.clone(), v.clone());
            }
        }
        (status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
