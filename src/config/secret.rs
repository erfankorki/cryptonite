//! A string wrapper for sensitive values (API keys, secrets, tokens).
//!
//! The value never appears in `Debug` output, so it can't leak through
//! `println!("{:?}")`, `tracing` fields, or panic messages. The only way to
//! read it is the explicit [`Secret::expose`] call.

use std::fmt;

use serde::{Deserialize, Deserializer};

#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the raw value. Use only where it is actually sent to an exchange.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_value() {
        let secret = Secret::new("super-secret-value");
        assert!(!format!("{secret:?}").contains("super-secret-value"));
        assert_eq!(secret.expose(), "super-secret-value");
    }
}
