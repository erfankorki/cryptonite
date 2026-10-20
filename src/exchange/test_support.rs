//! Helpers for tests that run the clients against a mock (wiremock) server.

use crate::config::{Credentials, ExchangeCredentials, ExchangeUrls, Secret, Settings};

fn credentials(secret: Option<&str>) -> Option<ExchangeCredentials> {
    Some(ExchangeCredentials {
        api_key: Secret::new("test-key"),
        api_secret: secret.map(Secret::new),
    })
}

/// Settings that point every exchange at `base_url`, with test credentials.
/// Bitpin additionally gets the secret `test-secret`.
pub fn settings_for(base_url: &str) -> Settings {
    Settings {
        exchanges: ExchangeUrls {
            nobitex: base_url.to_owned(),
            wallex: base_url.to_owned(),
            bitpin: base_url.to_owned(),
        },
        credentials: Credentials {
            nobitex: credentials(None),
            wallex: credentials(None),
            bitpin: credentials(Some("test-secret")),
        },
    }
}

/// Same URLs, but no credentials at all.
pub fn settings_without_credentials(base_url: &str) -> Settings {
    Settings {
        credentials: Credentials::default(),
        ..settings_for(base_url)
    }
}

/// Like `settings_for`, but Nobitex uses API-key (signed) authentication.
pub fn settings_with_nobitex_api_key(base_url: &str, public_key: &str, private_key: &str) -> Settings {
    let mut settings = settings_for(base_url);
    settings.credentials.nobitex = Some(ExchangeCredentials {
        api_key: Secret::new(public_key),
        api_secret: Some(Secret::new(private_key)),
    });
    settings
}
