mod config;
mod data;
mod engine;
mod exchange;
mod ui;

use std::time::Duration;

use config::Settings;
use engine::domain::models::{Coin, Market, OrderBook};
use exchange::{BitpinClient, ExchangeError, MarketData, NobitexClient, WallexClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::load()?;
    let client = reqwest::Client::new();

    let nobitex_client = NobitexClient::from_settings(&settings);
    let wallex_client = WallexClient::from_settings(&settings);
    let bitpin_client = BitpinClient::from_settings(&settings);

    let market = Market {
        base: Coin {
            symbol: "USDT".into(),
        },
        quote: Coin {
            symbol: "IRT".into(),
        },
    };

    loop {
        log_order_book("nobitex", nobitex_client.get_order_book(&client, &market).await);
        log_order_book("wallex", wallex_client.get_order_book(&client, &market).await);
        log_order_book("bitpin", bitpin_client.get_order_book(&client, &market).await);
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

fn log_order_book(name: &str, result: Result<OrderBook, ExchangeError>) {
    match result {
        Ok(order_book) => {
            println!("{name}: {order_book:#?}");
        }
        Err(err) => eprintln!("{name}: {err}"),
    }
}
