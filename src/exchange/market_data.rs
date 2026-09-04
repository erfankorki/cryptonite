use crate::engine::domain::models::{Market, OrderBook};
use super::error::ExchangeError;


pub trait MarketData {
    async fn get_order_book(
        &self,
        client: &reqwest::Client,
        market: &Market,
    ) -> Result<OrderBook, ExchangeError>;
}
