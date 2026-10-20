//! Exchange domain models.

use rust_decimal::Decimal;
use uuid::{Uuid};

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ExchangeName {
    Nobitex,
    Wallex,
    Bitpin,
}

pub struct Exchange {
    pub name: ExchangeName,
}

pub struct Wallet {
    pub exchange: ExchangeName,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OrderStatus {
    Ongoing,
    Filled,
    Cancelled,
}

pub struct Order {
    pub id: Uuid,
    pub exchange: ExchangeName,
    pub market: Market,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    pub status: OrderStatus,
}

#[derive(Debug, Clone)]
pub struct Coin {
    pub symbol: String,
}

#[derive(Debug, Clone)]
pub struct Market {
    pub base: Coin,
    pub quote: Coin,
}

#[derive(Debug, Clone)]
pub struct OrderBookItem {
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub market: Market,
    pub bids: Vec<OrderBookItem>,
    pub asks: Vec<OrderBookItem>,
}

/// Funds of one currency in an exchange account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    /// Upper-case symbol; the Iranian fiat currency is always `IRT`.
    pub currency: String,
    pub available: Decimal,
    pub locked: Decimal,
}

/// A limit order to place on an exchange.
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub market: Market,
    pub side: OrderSide,
    /// Price in the exchange's own quote unit (see the exchange docs).
    pub price: Decimal,
    /// Quantity of the base currency.
    pub quantity: Decimal,
    /// Caller-chosen unique id: letters, digits, `-` and `_`, at most 32 chars.
    pub client_order_id: String,
}

/// State of an order as reported by an exchange.
#[derive(Debug, Clone)]
pub struct OrderInfo {
    /// The id to pass back to `get_order` / `cancel_order`.
    pub exchange_order_id: String,
    pub client_order_id: Option<String>,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
}
