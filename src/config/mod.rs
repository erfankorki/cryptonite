//! Application configuration loaded via the `config` crate.

use config::{Config, ConfigError, File};
use serde::Deserialize;

use crate::engine::domain::models::ExchangeName;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub exchanges: ExchangeUrls,
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

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        let _ = dotenvy::from_filename(".env.local");

        Config::builder()
            .add_source(File::with_name("config").required(true))
            .build()?
            .try_deserialize()
    }
}
