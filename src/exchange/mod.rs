//! Fetch and normalize market data from crypto exchanges.

mod clients;
mod error;
mod market_data;

pub use clients::{BitpinClient, NobitexClient, WallexClient};
pub use error::ExchangeError;
pub use market_data::MarketData;
