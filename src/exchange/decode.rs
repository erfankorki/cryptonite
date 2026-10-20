//! Helpers shared by the exchange clients for decoding HTTP responses.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, de::DeserializeOwned};

use super::error::ExchangeError;

/// A number that an exchange may send as a JSON string or as a JSON number.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Flex {
    Str(String),
    Int(i64),
    Float(f64),
}

impl Flex {
    pub fn to_decimal(&self) -> Result<Decimal, ExchangeError> {
        match self {
            Flex::Str(value) => parse_decimal(value),
            Flex::Int(value) => Ok(Decimal::from(*value)),
            Flex::Float(value) => Ok(Decimal::try_from(*value)?),
        }
    }
}

/// Parses plain (`"12.5"`) and scientific (`"0E-10"`) decimal strings.
pub fn parse_decimal(value: &str) -> Result<Decimal, ExchangeError> {
    Decimal::from_str(value)
        .or_else(|_| Decimal::from_scientific(value))
        .map_err(ExchangeError::from)
}

/// Returns the response body, or an `Api` error that includes the status and
/// the (truncated) body when the HTTP status is not a success.
pub async fn success_body(response: reqwest::Response) -> Result<String, ExchangeError> {
    let status = response.status();
    let text = response.text().await?;
    if status.is_success() {
        Ok(text)
    } else {
        let snippet: String = text.chars().take(300).collect();
        Err(ExchangeError::Api(format!("http {status}: {snippet}")))
    }
}

pub async fn read_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, ExchangeError> {
    let text = success_body(response).await?;
    Ok(serde_json::from_str(&text)?)
}

/// Ids are placed into URLs and request bodies, so only a safe alphabet is allowed.
pub fn validate_id(id: &str, max_len: usize) -> Result<(), ExchangeError> {
    let valid = !id.is_empty()
        && id.len() <= max_len
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(ExchangeError::InvalidOrder(format!(
            "id must be 1-{max_len} chars of letters, digits, '-' or '_'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scientific_and_plain_decimals() {
        assert_eq!(parse_decimal("12.50").unwrap(), Decimal::new(1250, 2));
        assert_eq!(parse_decimal("0E-10").unwrap(), Decimal::ZERO);
    }

    #[test]
    fn flex_accepts_strings_and_numbers() {
        let values: Vec<Flex> = serde_json::from_str(r#"["1.5", 2, 0.25]"#).unwrap();
        let parsed: Vec<Decimal> = values.iter().map(|v| v.to_decimal().unwrap()).collect();
        assert_eq!(parsed, vec![Decimal::new(15, 1), Decimal::from(2), Decimal::new(25, 2)]);
    }

    #[test]
    fn rejects_unsafe_ids() {
        assert!(validate_id("bot-1_a", 32).is_ok());
        assert!(validate_id("", 32).is_err());
        assert!(validate_id("a/b", 32).is_err());
        assert!(validate_id(&"x".repeat(33), 32).is_err());
    }
}
