use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct PlayerId(pub Uuid);

impl PlayerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PlayerId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Which balance a movement touches. Sandbox callbacks never mix with live money.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountEnv {
    Live,
    Sandbox,
}

impl AccountEnv {
    pub fn as_str(self) -> &'static str {
        match self {
            AccountEnv::Live => "live",
            AccountEnv::Sandbox => "sandbox",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "live" => Some(AccountEnv::Live),
            "sandbox" => Some(AccountEnv::Sandbox),
            _ => None,
        }
    }
}

/// Who sent the money movement. Part of the idempotency key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Bigbang,
    Stripe,
    Sports,
    Chaos,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Bigbang => "bigbang",
            Source::Stripe => "stripe",
            Source::Sports => "sports",
            Source::Chaos => "chaos",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
