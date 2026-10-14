use anyhow::Context;
use domain::AccountEnv;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub api_bind: String,
    pub public_base_url: String,
    pub web_origins: Vec<String>,
    pub ops_api_key: String,
    pub platform_currency: String,
    pub bigbang_api_key: String,
    pub bigbang_base_url: String,
    pub bigbang_env: AccountEnv,
    pub stripe_secret_key: String,
    pub stripe_webhook_secret: String,
    pub sports_base_url: String,
    pub sports_callback_secret: String,
}

fn var(name: &str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| format!("missing env var {name}"))
}

fn var_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let env_str = var_or("BIGBANG_ENV", "sandbox");
        let bigbang_env = AccountEnv::parse(&env_str)
            .with_context(|| format!("BIGBANG_ENV must be sandbox or live, got {env_str}"))?;
        Ok(Self {
            database_url: var("DATABASE_URL")?,
            api_bind: var_or("API_BIND", "0.0.0.0:8080"),
            public_base_url: var_or("PUBLIC_BASE_URL", "http://localhost:8080"),
            web_origins: var_or("WEB_ORIGINS", "http://localhost:5173")
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            ops_api_key: var("OPS_API_KEY")?,
            platform_currency: var_or("PLATFORM_CURRENCY", "EUR"),
            bigbang_api_key: var("BIGBANG_API_KEY")?,
            bigbang_base_url: var_or("BIGBANG_BASE_URL", "https://api.bigbangcasino.bet"),
            bigbang_env,
            stripe_secret_key: var_or("STRIPE_SECRET_KEY", ""),
            stripe_webhook_secret: var_or("STRIPE_WEBHOOK_SECRET", ""),
            sports_base_url: var_or("SPORTS_BASE_URL", "http://localhost:8090"),
            sports_callback_secret: var("SPORTS_CALLBACK_SECRET")?,
        })
    }

    pub fn web_origin(&self) -> &str {
        self.web_origins
            .first()
            .map(String::as_str)
            .unwrap_or("http://localhost:5173")
    }

    pub fn local_port(&self) -> u16 {
        self.api_bind
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080)
    }
}
