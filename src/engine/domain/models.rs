//! Exchange domain models.

use rust_decimal::Decimal;
use uuid::{Uuid};

#[derive(PartialEq, Eq, Copy, Clone)]
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

#[derive(PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(PartialEq, Eq)]
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
