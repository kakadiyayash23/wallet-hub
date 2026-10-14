use crate::config::Config;
use crate::live::LiveEvent;
use ledger::Store;
use provider_bigbang::{BigBang, BigBangConfig};
use provider_sports::{SportsConfig, Sportsbook};
use provider_stripe::{Stripe, StripeConfig};
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct AppState {
    pub cfg: Config,
    pub store: Store,
    pub http: reqwest::Client,
    pub bigbang: Arc<BigBang>,
    pub stripe: Arc<Stripe>,
    pub sports: Arc<Sportsbook>,
    pub live: broadcast::Sender<LiveEvent>,
}

impl AppState {
    pub fn new(cfg: Config, store: Store) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("wallet-hub/0.1")
            .build()
            .expect("reqwest client");
        let bigbang = Arc::new(BigBang::new(
            BigBangConfig {
                api_key: cfg.bigbang_api_key.clone(),
                base_url: cfg.bigbang_base_url.clone(),
                env: cfg.bigbang_env,
                currency: cfg.platform_currency.clone(),
            },
            http.clone(),
        ));
        let stripe = Arc::new(Stripe::new(
            StripeConfig {
                secret_key: cfg.stripe_secret_key.clone(),
                webhook_secret: cfg.stripe_webhook_secret.clone(),
                tolerance_secs: 300,
            },
            http.clone(),
        ));
        let sports = Arc::new(Sportsbook::new(
            SportsConfig {
                base_url: cfg.sports_base_url.clone(),
                callback_secret: cfg.sports_callback_secret.clone(),
            },
            http.clone(),
        ));
        let (live, _) = broadcast::channel(512);
        Self {
            cfg,
            store,
            http,
            bigbang,
            stripe,
            sports,
            live,
        }
    }

    pub fn publish(&self, ev: LiveEvent) {
        // No subscribers is fine.
        let _ = self.live.send(ev);
    }
}

pub type Shared = Arc<AppState>;
