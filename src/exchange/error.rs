#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api error: {0}")]
    Api(String),
    #[error("decimal parse error: {0}")]
    Decimal(#[from] rust_decimal::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no credentials configured for {0}")]
    MissingCredentials(&'static str),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("invalid order: {0}")]
    InvalidOrder(String),
}
