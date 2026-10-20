//! Authenticated account operations: balances and orders.
//!
//! Public market data lives in [`super::MarketData`]; everything here needs
//! credentials from `Settings::credentials` and fails with
//! `ExchangeError::MissingCredentials` when they are not configured.

mod bitpin;
mod nobitex;
mod wallex;

use crate::engine::domain::models::{Balance, OrderInfo, OrderRequest, OrderSide};

use super::error::ExchangeError;

pub trait Account {
    /// All wallets of the account. Currencies are upper-case, fiat is `IRT`.
    async fn get_balances(&self, client: &reqwest::Client) -> Result<Vec<Balance>, ExchangeError>;

    /// Places a limit order.
    async fn place_order(
        &self,
        client: &reqwest::Client,
        order: &OrderRequest,
    ) -> Result<OrderInfo, ExchangeError>;

    /// Looks an order up by the `exchange_order_id` returned from `place_order`.
    async fn get_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<OrderInfo, ExchangeError>;

    /// Cancels an order by the `exchange_order_id` returned from `place_order`.
    async fn cancel_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<(), ExchangeError>;
}

fn side_lower(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}
