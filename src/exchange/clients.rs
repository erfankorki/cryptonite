use crate::config::Settings;
use crate::engine::domain::models::{Market, OrderBook};

use super::error::ExchangeError;
use super::market_data::MarketData;

pub struct NobitexClient {
    base_url: String,
}

impl NobitexClient {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            base_url: settings.exchanges.nobitex.clone(),
        }
    }

    pub fn url(&self) -> &str {
        &self.base_url
    }
}

impl MarketData for NobitexClient {
    async fn get_order_book(
        &self,
        client: &reqwest::Client,
        _market: &Market,
    ) -> Result<OrderBook, ExchangeError> {
        todo!()
    }
}

pub struct WallexClient {
    base_url: String,
}

impl WallexClient {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            base_url: settings.exchanges.wallex.clone(),
        }
    }

    pub fn url(&self) -> &str {
        &self.base_url
    }
}

impl MarketData for WallexClient {
    async fn get_order_book(
        &self,
        client: &reqwest::Client,
        _market: &Market,
    ) -> Result<OrderBook, ExchangeError> {
        todo!()
    }
}

pub struct BitpinClient {
    base_url: String,
}

impl BitpinClient {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            base_url: settings.exchanges.bitpin.clone(),
        }
    }

    pub fn url(&self) -> &str {
        &self.base_url
    }
}

impl MarketData for BitpinClient {
    async fn get_order_book(
        &self,
        client: &reqwest::Client,
        _market: &Market,
    ) -> Result<OrderBook, ExchangeError> {
        todo!()
    }
}
