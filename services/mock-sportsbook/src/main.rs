//! A stand-in sportsbook provider. It serves real odds when ODDS_API_KEY is set
//! (The Odds API, free tier) and built-in fixtures otherwise. It accepts bets,
//! settles them after a delay, and sends signed callbacks to the wallet, the
//! same way a real provider would.

mod odds;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use domain::traits::{BetEvent, BetRequest, BetTicket, CashOutQuote, Market, SportInfo};
use domain::Money;
use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone)]
struct Config {
    bind: String,
    wallet_callback_url: String,
    secret: String,
    odds_api_key: Option<String>,
    settle_after: Duration,
}

#[derive(Clone, Debug)]
struct BetRecord {
    req: BetRequest,
    status: String,
}

struct App {
    cfg: Config,
    http: reqwest::Client,
    bets: Mutex<HashMap<String, BetRecord>>,
    odds: odds::OddsCache,
}

type Shared = Arc<App>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();
    let cfg = Config {
        bind: std::env::var("SPORTS_BIND").unwrap_or_else(|_| "0.0.0.0:8090".into()),
        wallet_callback_url: std::env::var("WALLET_CALLBACK_URL")
            .unwrap_or_else(|_| "http://localhost:8080/callbacks/sports".into()),
        secret: std::env::var("SPORTS_CALLBACK_SECRET")
            .unwrap_or_else(|_| "change-me-sports-secret".into()),
        odds_api_key: std::env::var("ODDS_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        settle_after: Duration::from_secs(
            std::env::var("SETTLE_AFTER_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
        ),
    };
    let app = Arc::new(App {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?,
        bets: Mutex::new(HashMap::new()),
        odds: odds::OddsCache::new(cfg.odds_api_key.clone()),
        cfg: cfg.clone(),
    });
    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/sports", get(sports))
        .route("/markets", get(markets))
        .route("/bets", post(place_bet))
        .route("/bets/:ticket/cashout", post(cash_out))
        .with_state(app);
    tracing::info!(bind = %cfg.bind, "mock-sportsbook listening");
    let listener = tokio::net::TcpListener::bind(&cfg.bind).await?;
    axum::serve(listener, router).await?;
    Ok(())
}

#[derive(Deserialize)]
struct MarketsQuery {
    #[serde(default = "default_sport")]
    sport: String,
}

fn default_sport() -> String {
    "soccer".into()
}

/// The sport rail: key, label and how many upcoming events each has.
async fn sports(State(app): State<Shared>) -> Json<Vec<SportInfo>> {
    let mut out = Vec::new();
    for (key, label, _) in odds::SPORTS {
        let count = app
            .odds
            .markets(&app.http, key)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        out.push(SportInfo {
            key: (*key).into(),
            label: (*label).into(),
            count,
        });
    }
    Json(out)
}

async fn markets(
    State(app): State<Shared>,
    Query(q): Query<MarketsQuery>,
) -> Result<Json<Vec<Market>>, (StatusCode, String)> {
    app.odds
        .markets(&app.http, &q.sport)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))
}

async fn place_bet(
    State(app): State<Shared>,
    Json(req): Json<BetRequest>,
) -> Result<Json<BetTicket>, (StatusCode, String)> {
    if req.stake.minor() <= 0 {
        return Err((StatusCode::BAD_REQUEST, "stake must be positive".into()));
    }
    let market = app.odds.find(&app.http, &req.event_id).await;
    let event_name = market
        .as_ref()
        .map(|m| format!("{} v {}", m.home, m.away))
        .unwrap_or_else(|| req.event_id.clone());
    // A real book re-prices. We accept the client's odds if they match the current market within 2%.
    if let Some(m) = &market {
        if let Some(sel) = m.selections.iter().find(|s| s.key == req.selection) {
            let diff = (sel.odds_milli - req.odds_milli).abs();
            if diff * 50 > sel.odds_milli {
                return Err((
                    StatusCode::CONFLICT,
                    format!("odds changed: now {}", sel.odds_milli),
                ));
            }
        }
    }
    let ticket_ref = format!("tk_{}", uuid::Uuid::new_v4().simple());
    app.bets.lock().await.insert(
        ticket_ref.clone(),
        BetRecord {
            req: req.clone(),
            status: "open".into(),
        },
    );
    tracing::info!(%ticket_ref, stake = %req.stake, "bet accepted");

    // Settle later, like a real event finishing.
    let app2 = app.clone();
    let ticket2 = ticket_ref.clone();
    tokio::spawn(async move {
        tokio::time::sleep(app2.cfg.settle_after).await;
        settle(app2, ticket2).await;
    });

    Ok(Json(BetTicket {
        ticket_ref,
        odds_milli: req.odds_milli,
        event_name,
    }))
}

async fn settle(app: Shared, ticket_ref: String) {
    let rec = {
        let mut bets = app.bets.lock().await;
        let Some(rec) = bets.get_mut(&ticket_ref) else {
            return;
        };
        if rec.status != "open" {
            return;
        }
        rec.status = "settling".into();
        rec.clone()
    };
    // Fair coin weighted by the odds: p(win) = 1 / odds.
    let p_win = 1000.0 / rec.req.odds_milli as f64;
    let won = rand::thread_rng().gen_bool(p_win.clamp(0.02, 0.98));
    let (result, payout) = if won {
        (
            "won",
            rec.req
                .stake
                .mul_milli(rec.req.odds_milli)
                .unwrap_or(Money::ZERO),
        )
    } else {
        ("lost", Money::ZERO)
    };
    let event = BetEvent::Settled {
        ticket_ref: ticket_ref.clone(),
        result: result.into(),
        payout,
    };
    let ok = send_callback(&app, &event).await;
    let mut bets = app.bets.lock().await;
    if let Some(r) = bets.get_mut(&ticket_ref) {
        r.status = if ok { result.into() } else { "open".into() };
    }
}

async fn cash_out(
    State(app): State<Shared>,
    Path(ticket): Path<String>,
) -> Result<Json<CashOutQuote>, (StatusCode, String)> {
    let rec = {
        let mut bets = app.bets.lock().await;
        let Some(rec) = bets.get_mut(&ticket) else {
            return Err((StatusCode::NOT_FOUND, "unknown ticket".into()));
        };
        if rec.status != "open" {
            return Err((StatusCode::CONFLICT, format!("bet is {}", rec.status)));
        }
        rec.status = "cashed_out".into();
        rec.clone()
    };
    // Demo pricing: 85% of the stake back.
    let amount = rec.req.stake.mul_milli(850).unwrap_or(Money::ZERO);
    let event = BetEvent::CashedOut {
        ticket_ref: ticket.clone(),
        amount,
    };
    let app2 = app.clone();
    tokio::spawn(async move {
        send_callback(&app2, &event).await;
    });
    Ok(Json(CashOutQuote {
        ticket_ref: ticket,
        amount,
    }))
}

/// POST the event to the wallet with `X-Signature`. Retries three times like a real provider.
async fn send_callback(app: &App, event: &BetEvent) -> bool {
    let body = serde_json::to_vec(event).unwrap_or_default();
    let sig = domain::sig::hmac_sha256_hex(app.cfg.secret.as_bytes(), &body);
    for attempt in 1..=3u32 {
        let res = app
            .http
            .post(&app.cfg.wallet_callback_url)
            .header("content-type", "application/json")
            .header("x-signature", &sig)
            .body(body.clone())
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => {
                tracing::info!(?event, "callback delivered");
                return true;
            }
            Ok(r) => tracing::warn!(attempt, status = %r.status(), "callback not accepted"),
            Err(e) => tracing::warn!(attempt, error = %e, "callback failed"),
        }
        tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
    }
    false
}
