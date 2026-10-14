use crate::Money;

/// Errors a provider or the ledger can return. HTTP mapping lives in the api crate.
#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("player not found")]
    PlayerNotFound,
    #[error("insufficient funds (balance {balance})")]
    InsufficientFunds { balance: Money },
    #[error("invalid signature")]
    InvalidSignature,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("transaction {0} was rolled back; refusing to apply it")]
    RolledBack(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl WalletError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    pub fn provider(msg: impl Into<String>) -> Self {
        Self::Provider(msg.into())
    }
}
