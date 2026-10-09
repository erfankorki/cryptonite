//! Fetch and normalize market data from crypto exchanges.

mod account;
mod clients;
mod decode;
mod error;
mod helper;
mod market_data;
#[cfg(test)]
mod test_support;

pub use account::Account;
pub use clients::{BitpinClient, NobitexClient, WallexClient};
pub use error::ExchangeError;
pub use market_data::MarketData;
