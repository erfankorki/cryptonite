//! Application configuration loaded via the `config` crate.
//!
//! Non-sensitive values (exchange URLs) come from `config.toml`.
//! Credentials come from environment variables, which are populated from the
//! git-ignored `.env.local` / `.env` files (see `.env.example`).

mod secret;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

use crate::engine::domain::models::ExchangeName;

pub use secret::Secret;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub exchanges: ExchangeUrls,
    #[serde(default)]
    pub credentials: Credentials,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeUrls {
    pub nobitex: String,
    pub wallex: String,
    pub bitpin: String,
}

impl ExchangeUrls {
    pub fn url(&self, name: ExchangeName) -> &str {
        match name {
            ExchangeName::Nobitex => &self.nobitex,
            ExchangeName::Wallex => &self.wallex,
            ExchangeName::Bitpin => &self.bitpin,
        }
    }
}

/// Authentication data for one exchange account.
///
/// `api_key` is always required. `api_secret` is only needed by exchanges
/// that sign requests with a key/secret pair.
#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeCredentials {
    pub api_key: Secret,
    #[serde(default)]
    pub api_secret: Option<Secret>,
}

/// Per-exchange credentials. An exchange without credentials is simply
/// unavailable for trading; public market data keeps working.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Credentials {
    #[serde(default)]
    pub nobitex: Option<ExchangeCredentials>,
    #[serde(default)]
    pub wallex: Option<ExchangeCredentials>,
    #[serde(default)]
    pub bitpin: Option<ExchangeCredentials>,
}

impl Credentials {
    pub fn get(&self, name: ExchangeName) -> Option<&ExchangeCredentials> {
        match name {
            ExchangeName::Nobitex => self.nobitex.as_ref(),
            ExchangeName::Wallex => self.wallex.as_ref(),
            ExchangeName::Bitpin => self.bitpin.as_ref(),
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        // dotenvy never overrides variables that are already set, so real
        // environment variables win over `.env.local`, which wins over `.env`.
        let _ = dotenvy::from_filename(".env.local");
        let _ = dotenvy::dotenv();

        Config::builder()
            .add_source(File::with_name("config").required(true))
            // e.g. CRYPTONITE__CREDENTIALS__NOBITEX__API_KEY=...
            .add_source(
                Environment::with_prefix("CRYPTONITE")
                    .separator("__")
                    .ignore_empty(true),
            )
            .build()?
            .try_deserialize()
    }
}
