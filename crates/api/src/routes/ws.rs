use crate::session;
use crate::state::Shared;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use domain::PlayerId;

/// Live balance updates for the logged-in player. The session cookie is sent
/// with the upgrade request, so no id travels in the URL.
pub async fn upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Shared>,
    headers: HeaderMap,
) -> Response {
    let pid =
        session::from_headers(state.cfg.session_secret.as_bytes(), &headers).and_then(|s| s.pid);
    let Some(pid) = pid else {
        return (StatusCode::UNAUTHORIZED, "not logged in").into_response();
    };
    ws.on_upgrade(move |socket| handle(socket, state, PlayerId(pid)))
}

async fn handle(mut socket: WebSocket, state: Shared, player: PlayerId) {
    let mut rx = state.live.subscribe();
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(ev) if ev.player_id == player => {
                    let text = serde_json::to_string(&ev).unwrap_or_default();
                    if socket.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            msg = socket.recv() => match msg {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            }
        }
    }
}
