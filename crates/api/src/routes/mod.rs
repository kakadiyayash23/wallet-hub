use crate::state::Shared;
use axum::routing::{get, post};
use axum::Router;

mod casino;
mod inbound;
mod ops;
mod players;
mod sports;
mod wallet;
mod ws;

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/health", get(health))
        // players
        .route("/api/players", post(players::login))
        .route(
            "/api/players/me",
            get(players::me).patch(players::update_me),
        )
        // casino
        .route("/api/games", get(casino::list_games))
        .route("/api/games/categories", get(casino::categories))
        .route("/api/casino/launch", post(casino::launch))
        // wallet
        .route("/api/wallet", get(wallet::summary))
        .route("/api/wallet/ledger", get(wallet::ledger))
        .route("/api/wallet/payments", get(wallet::payments))
        .route("/api/wallet/deposits", post(wallet::deposit))
        .route("/api/wallet/withdrawals", post(wallet::withdraw))
        .route("/api/wallet/sandbox/topup", post(wallet::sandbox_topup))
        // sports
        .route("/api/sports", get(sports::sports))
        .route("/api/sports/markets", get(sports::markets))
        .route(
            "/api/sports/bets",
            get(sports::list_bets).post(sports::place_bet),
        )
        .route("/api/sports/bets/:id/cashout", post(sports::cash_out))
        // inbound from providers
        .route("/callbacks/bigbang/user_data", get(inbound::bigbang))
        .route("/callbacks/bigbang/balance_change", post(inbound::bigbang))
        .route("/webhooks/stripe", post(inbound::stripe))
        .route("/callbacks/sports", post(inbound::sports))
        // operator
        .route("/api/ops/events", get(ops::events))
        .route("/api/ops/events/stats", get(ops::event_stats))
        .route("/api/ops/events/:id/replay", post(ops::replay))
        .route("/api/ops/reconcile/:player_id", get(ops::reconcile))
        .route("/api/ops/catalog/sync", post(ops::catalog_sync))
        .route("/api/ops/chaos", post(ops::chaos))
        .route("/api/ops/trial-balance", get(ops::trial_balance))
        .route("/api/ops/players", get(ops::players))
        // live
        .route("/ws", get(ws::upgrade))
}

async fn health() -> &'static str {
    "ok"
}
