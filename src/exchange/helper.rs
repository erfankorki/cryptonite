use crate::engine::domain::models::{ExchangeName, Market};

pub fn market_param(exchange: ExchangeName, market: &Market) -> String {
    let base = market.base.symbol.to_uppercase();
    let quote = match exchange {
        ExchangeName::Wallex => {
            if market.quote.symbol.eq_ignore_ascii_case("IRT") {
                "TMN".to_string()
            } else {
                market.quote.symbol.to_uppercase()
            }
        }
        ExchangeName::Nobitex => market.quote.symbol.to_uppercase(),
        ExchangeName::Bitpin => market.quote.symbol.to_uppercase(),
    };

    match exchange {
        ExchangeName::Bitpin => format!("{base}_{quote}"),
        ExchangeName::Nobitex => format!("{base}{quote}"),
        ExchangeName::Wallex => format!("{base}{quote}"),
    }
}

pub fn orderbook_path(exchange: ExchangeName, market: &Market) -> String {
    let param = market_param(exchange, market);
    match exchange {
        ExchangeName::Nobitex => format!("v3/orderbook/{param}"),
        ExchangeName::Wallex => format!("v1/depth?symbol={param}"),
        ExchangeName::Bitpin => format!("api/v1/mth/orderbook/{param}/"),
    }
}

pub fn join_url(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}
