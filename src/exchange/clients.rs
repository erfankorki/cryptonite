use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::config::Settings;
use crate::engine::domain::models::{ExchangeName, Market, OrderBook, OrderBookItem};

use super::error::ExchangeError;
use super::helper::{join_url, orderbook_path};
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
}

impl MarketData for NobitexClient {
    async fn get_order_book(
        &self,
        client: &reqwest::Client,
        market: &Market,
    ) -> Result<OrderBook, ExchangeError> {
        let url = join_url(&self.base_url, &orderbook_path(ExchangeName::Nobitex, market));
        let body: NobitexOrderBookResponse = client.get(url).send().await?.error_for_status()?.json().await?;

        if body.status != "ok" {
            return Err(ExchangeError::Api(format!("nobitex status: {}", body.status)));
        }

        Ok(OrderBook {
            market: market.clone(),
            bids: parse_string_levels(&body.bids)?,
            asks: parse_string_levels(&body.asks)?,
        })
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
}

impl MarketData for WallexClient {
    async fn get_order_book(
        &self,
        client: &reqwest::Client,
        market: &Market,
    ) -> Result<OrderBook, ExchangeError> {
        let url: String = join_url(&self.base_url, &orderbook_path(ExchangeName::Wallex, market));
        let body: WallexDepthResponse = client.get(url).send().await?.error_for_status()?.json().await?;

        if !body.success {
            return Err(ExchangeError::Api("wallex request failed".into()));
        }

        Ok(OrderBook {
            market: market.clone(),
            bids: parse_wallex_levels(&body.result.bid)?,
            asks: parse_wallex_levels(&body.result.ask)?,
        })
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
}

impl MarketData for BitpinClient {
    async fn get_order_book(
        &self,
        client: &reqwest::Client,
        market: &Market,
    ) -> Result<OrderBook, ExchangeError> {
        let url = join_url(&self.base_url, &orderbook_path(ExchangeName::Bitpin, market));
        let body: BitpinOrderBookResponse = client.get(url).send().await?.error_for_status()?.json().await?;

        Ok(OrderBook {
            market: market.clone(),
            bids: parse_string_levels(&body.bids)?,
            asks: parse_string_levels(&body.asks)?,
        })
    }
}

fn parse_string_levels(levels: &[[String; 2]]) -> Result<Vec<OrderBookItem>, ExchangeError> {
    levels
        .iter()
        .map(|[price, quantity]| {
            Ok(OrderBookItem {
                price: Decimal::from_str(price)?,
                quantity: Decimal::from_str(quantity)?,
            })
        })
        .collect()
}

fn parse_wallex_levels(levels: &[WallexLevel]) -> Result<Vec<OrderBookItem>, ExchangeError> {
    levels
        .iter()
        .map(|level| {
            Ok(OrderBookItem {
                price: Decimal::from(level.price),
                quantity: Decimal::try_from(level.quantity)?,
            })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct NobitexOrderBookResponse {
    status: String,
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}

#[derive(Debug, Deserialize)]
struct WallexDepthResponse {
    success: bool,
    result: WallexDepthResult,
}

#[derive(Debug, Deserialize)]
struct WallexDepthResult {
    bid: Vec<WallexLevel>,
    ask: Vec<WallexLevel>,
}

#[derive(Debug, Deserialize)]
struct WallexLevel {
    price: u64,
    quantity: f64,
}

#[derive(Debug, Deserialize)]
struct BitpinOrderBookResponse {
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}
