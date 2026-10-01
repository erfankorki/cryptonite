#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api error: {0}")]
    Api(String),
    #[error("decimal parse error: {0}")]
    Decimal(#[from] rust_decimal::Error),
}
