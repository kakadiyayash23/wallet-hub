use crate::state::Shared;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use domain::PlayerId;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct WsQuery {
    pub player_id: Uuid,
}

pub async fn upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Shared>,
    Query(q): Query<WsQuery>,
) -> Response {
    ws.on_upgrade(move |socket| handle(socket, state, PlayerId(q.player_id)))
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
