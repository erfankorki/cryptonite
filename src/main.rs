mod config;
mod data;
mod engine;
mod exchange;
mod ui;

use config::Settings;
use exchange::{BitpinClient, NobitexClient, WallexClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::load()?;
    let client = reqwest::Client::new();

    let nobitex_client = NobitexClient::from_settings(&settings);
    let wallex_client = WallexClient::from_settings(&settings);
    let bitpin_client = BitpinClient::from_settings(&settings);

    Ok(())
}
