mod config;
mod data;
mod engine;
mod exchange;
mod ui;

use std::io::IsTerminal;
use std::time::{Duration, Instant};

use config::Settings;
use engine::domain::models::{Balance, Coin, Market, OrderBook};
use exchange::{Account, BitpinClient, ExchangeError, MarketData, NobitexClient, WallexClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::load()?;
    let client = reqwest::Client::new();

    let nobitex_client = NobitexClient::from_settings(&settings);
    let wallex_client = WallexClient::from_settings(&settings);
    let bitpin_client = BitpinClient::from_settings(&settings);

    // `cargo run -- balances [--amounts]`: read-only check of the credentials.
    if std::env::args().nth(1).as_deref() == Some("balances") {
        let show_amounts = std::env::args().any(|arg| arg == "--amounts");
        let style = Style::detect();

        println!("{}", style.bold(&format!("Cryptonite v{} · balance check", env!("CARGO_PKG_VERSION"))));
        println!("{}\n", style.dim("Read-only: no orders are placed."));

        print_balances("[1/3] nobitex", &nobitex_client, &client, show_amounts, &style).await;
        print_balances("[2/3] wallex", &wallex_client, &client, show_amounts, &style).await;
        print_balances("[3/3] bitpin", &bitpin_client, &client, show_amounts, &style).await;

        if !show_amounts {
            println!("{}", style.dim("Amounts are hidden. Run `cargo run -- balances --amounts` to show them."));
        }
        return Ok(());
    }

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

/// Minimal ANSI styling; plain text when stdout is not a terminal or `NO_COLOR` is set.
struct Style {
    color: bool,
}

impl Style {
    fn detect() -> Self {
        Self {
            color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    fn green(&self, text: &str) -> String {
        self.paint("32", text)
    }

    fn yellow(&self, text: &str) -> String {
        self.paint("33", text)
    }

    fn red(&self, text: &str) -> String {
        self.paint("31", text)
    }
}

/// Fetches and prints the funded wallets of one exchange. Amounts are hidden
/// unless asked for, so the output can be shared without revealing balances.
async fn print_balances<A: Account>(
    title: &str,
    account: &A,
    client: &reqwest::Client,
    show_amounts: bool,
    style: &Style,
) {
    println!("{} {}", style.bold(title), style.dim("fetching balances…"));
    let started = Instant::now();
    let result = account.get_balances(client).await;
    let took = style.dim(&format!("({:.1}s)", started.elapsed().as_secs_f32()));

    match result {
        Ok(balances) => {
            let funded: Vec<&Balance> = balances
                .iter()
                .filter(|balance| !balance.available.is_zero() || !balance.locked.is_zero())
                .collect();
            if funded.is_empty() {
                println!("  {} no funded wallets {took}", style.yellow("!"));
            } else {
                println!("  {} {} funded wallet(s) {took}\n", style.green("✓"), funded.len());
                for line in balance_lines(&funded, show_amounts) {
                    println!("      {line}");
                }
            }
        }
        Err(ExchangeError::MissingCredentials(_)) => {
            println!("  {} skipped: no credentials configured", style.dim("–"));
        }
        Err(err) => println!("  {} {err}", style.red("✗")),
    }
    println!();
}

/// Table of wallets (IRT first, then alphabetical), or just the currency
/// names in a grid when amounts are hidden.
fn balance_lines(funded: &[&Balance], show_amounts: bool) -> Vec<String> {
    let mut sorted: Vec<&Balance> = funded.to_vec();
    sorted.sort_by(|a, b| (a.currency != "IRT", &a.currency).cmp(&(b.currency != "IRT", &b.currency)));

    if !show_amounts {
        let width = sorted.iter().map(|b| b.currency.len()).max().unwrap_or(0);
        return sorted
            .chunks(5)
            .map(|row| {
                let cells: Vec<String> = row.iter().map(|b| format!("{:<width$}", b.currency)).collect();
                cells.join("  ").trim_end().to_owned()
            })
            .collect();
    }

    let rows: Vec<(String, String, String)> = sorted
        .iter()
        .map(|b| {
            (
                b.currency.clone(),
                b.available.normalize().to_string(),
                b.locked.normalize().to_string(),
            )
        })
        .collect();
    let w0 = rows.iter().map(|r| r.0.len()).max().unwrap_or(0).max("Currency".len());
    let w1 = rows.iter().map(|r| r.1.len()).max().unwrap_or(0).max("Available".len());
    let w2 = rows.iter().map(|r| r.2.len()).max().unwrap_or(0).max("Locked".len());

    let mut lines = vec![
        format!("{:<w0$}  {:>w1$}  {:>w2$}", "Currency", "Available", "Locked"),
        "─".repeat(w0 + w1 + w2 + 4),
    ];
    for (currency, available, locked) in rows {
        lines.push(format!("{currency:<w0$}  {available:>w1$}  {locked:>w2$}"));
    }
    lines
}
