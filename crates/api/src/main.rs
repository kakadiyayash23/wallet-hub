//! HTTP layer. Thin: it maps requests to domain calls and nothing else.

mod auth;
mod callbacks;
mod chaos;
mod config;
mod error;
mod live;
mod routes;
mod session;
mod state;

use axum::http::{header, HeaderValue, Method};
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Sync the BigBang catalog when the games table is empty, so the lobby is
/// never blank on a fresh database. Failures only log; the operator can retry.
async fn sync_catalog_if_empty(state: state::Shared) {
    use domain::traits::GameProvider;
    match state.store.games(None, None, 1, 0).await {
        Ok(rows) if rows.is_empty() => match state.bigbang.sync_catalog().await {
            Ok(listings) => match state.store.upsert_games("bigbang", &listings).await {
                Ok(n) => tracing::info!(games = n, "catalog synced on first boot"),
                Err(e) => tracing::warn!(error = %e, "catalog store failed"),
            },
            Err(e) => {
                tracing::warn!(error = %e, "catalog sync failed; use Operator → Sync catalog")
            }
        },
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "could not check the catalog"),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().compact())
        .init();

    let cfg = config::Config::from_env()?;
    let store = ledger::Store::connect(&cfg.database_url).await?;
    store.migrate().await?;
    tracing::info!("migrations applied");

    let state = Arc::new(state::AppState::new(cfg.clone(), store));
    tracing::info!(
        user_data = %format!("{}/callbacks/bigbang/user_data", cfg.public_base_url),
        balance_change = %format!("{}/callbacks/bigbang/balance_change", cfg.public_base_url),
        stripe = %format!("{}/webhooks/stripe", cfg.public_base_url),
        "configure these callback URLs at the providers"
    );

    // First boot: fill the lobby without a manual sync from the operator page.
    tokio::spawn(sync_catalog_if_empty(state.clone()));

    let origins: Vec<HeaderValue> = cfg
        .web_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            auth::OPS_HEADER,
        ])
        .allow_credentials(true);

    let app = Router::new()
        .merge(routes::router())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = cfg.api_bind.parse()?;
    tracing::info!(%addr, "api listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
