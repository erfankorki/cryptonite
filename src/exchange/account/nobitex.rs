//! Nobitex account API (https://apidocs.nobitex.ir).
//!
//! Two ways to authenticate, chosen by which credentials are configured:
//!
//! * **API key** (`api_key` = public key, `api_secret` = private key): every
//!   request is signed with Ed25519 and carries `Nobitex-Key`,
//!   `Nobitex-Signature` and `Nobitex-Timestamp`. The signed text is
//!   `timestamp + METHOD + full_path + raw_body`. Recommended: the key can be
//!   limited to `READ` / `TRADE` permissions and an IP whitelist.
//! * **Login token** (only `api_key`): `Authorization: Token <token>`.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::{Engine, alphabet};
use ed25519_dalek::{Signer, SigningKey};
use reqwest::Method;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::config::Secret;
use crate::engine::domain::models::{Balance, OrderInfo, OrderRequest, OrderStatus};
use crate::exchange::clients::NobitexClient;
use crate::exchange::decode::{Flex, read_json, validate_id};
use crate::exchange::error::ExchangeError;
use crate::exchange::helper::join_url;

use super::{Account, side_lower};

const CLIENT_ORDER_ID_MAX_LEN: usize = 32;

/// Nobitex asks bots to identify themselves with `TraderBot/<name-version>`.
const BOT_USER_AGENT: &str = concat!("TraderBot/Cryptonite-", env!("CARGO_PKG_VERSION"));

/// URL-safe base64 as used by Nobitex keys and signatures: padded on output,
/// padding optional on input.
const KEY_ENGINE: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// Nobitex calls the Iranian rial `rls`; the domain uses `IRT` everywhere.
fn to_nobitex_currency(symbol: &str) -> String {
    if symbol.eq_ignore_ascii_case("IRT") {
        "rls".to_owned()
    } else {
        symbol.to_lowercase()
    }
}

fn from_nobitex_currency(symbol: &str) -> String {
    if symbol.eq_ignore_ascii_case("rls") {
        "IRT".to_owned()
    } else {
        symbol.to_uppercase()
    }
}

fn map_status(status: &str) -> Result<OrderStatus, ExchangeError> {
    match status.to_ascii_lowercase().as_str() {
        "new" | "active" | "inactive" => Ok(OrderStatus::Ongoing),
        "done" => Ok(OrderStatus::Filled),
        "canceled" | "cancelled" => Ok(OrderStatus::Cancelled),
        other => Err(ExchangeError::Api(format!("unknown nobitex order status: {other}"))),
    }
}

impl NobitexClient {
    /// Sends one authenticated request.
    ///
    /// `body` is the exact JSON text to send: with API-key auth that same text
    /// is signed, so it must not be re-serialized afterwards.
    async fn send(
        &self,
        client: &reqwest::Client,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<reqwest::Response, ExchangeError> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::MissingCredentials("nobitex"))?;
        let raw_body = body.unwrap_or_default();
        let mut request = client
            .request(method.clone(), join_url(&self.base_url, path))
            .header(USER_AGENT, BOT_USER_AGENT);

        request = match &credentials.api_secret {
            Some(secret) => {
                let timestamp = unix_timestamp()?;
                let full_path = format!("/{}", path.trim_start_matches('/'));
                let signature = sign_request(secret, &timestamp, method.as_str(), &full_path, &raw_body)?;
                request
                    .header("Nobitex-Key", credentials.api_key.expose())
                    .header("Nobitex-Signature", signature)
                    .header("Nobitex-Timestamp", timestamp)
            }
            None => request.header(AUTHORIZATION, format!("Token {}", credentials.api_key.expose())),
        };

        if !raw_body.is_empty() {
            request = request.header(CONTENT_TYPE, "application/json").body(raw_body);
        }
        Ok(request.send().await?)
    }
}

fn unix_timestamp() -> Result<String, ExchangeError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExchangeError::Auth("system clock is before 1970".into()))?
        .as_secs();
    Ok(seconds.to_string())
}

/// Ed25519 signature over `timestamp + METHOD + full_path + raw_body`,
/// URL-safe base64 encoded. `secret` is the key's 32-byte private key in
/// URL-safe base64, as returned once when the API key is created.
fn sign_request(
    secret: &Secret,
    timestamp: &str,
    method: &str,
    full_path: &str,
    raw_body: &str,
) -> Result<String, ExchangeError> {
    let bytes = KEY_ENGINE
        .decode(secret.expose().trim())
        .map_err(|_| ExchangeError::Auth("nobitex api_secret is not valid URL-safe base64".into()))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ExchangeError::Auth("nobitex api_secret must decode to 32 bytes".into()))?;

    let payload = format!("{timestamp}{method}{full_path}{raw_body}");
    let signature = SigningKey::from_bytes(&seed).sign(payload.as_bytes());
    Ok(KEY_ENGINE.encode(signature.to_bytes()))
}

impl Account for NobitexClient {
    async fn get_balances(&self, client: &reqwest::Client) -> Result<Vec<Balance>, ExchangeError> {
        let response = self.send(client, Method::GET, "v2/wallets", None).await?;
        let body: WalletsResponse = read_json(response).await?;
        ensure_ok(&body.status, None, None)?;

        body.wallets
            .into_iter()
            .map(|(currency, wallet)| {
                // `balance` is the total, `blocked` the part frozen in open orders.
                let total = wallet.balance.to_decimal()?;
                let locked = wallet.blocked.to_decimal()?;
                Ok(Balance {
                    currency: from_nobitex_currency(&currency),
                    available: total - locked,
                    locked,
                })
            })
            .collect()
    }

    async fn place_order(
        &self,
        client: &reqwest::Client,
        order: &OrderRequest,
    ) -> Result<OrderInfo, ExchangeError> {
        validate_id(&order.client_order_id, CLIENT_ORDER_ID_MAX_LEN)?;
        let body = OrderBody {
            kind: side_lower(order.side),
            execution: "limit",
            src_currency: to_nobitex_currency(&order.market.base.symbol),
            dst_currency: to_nobitex_currency(&order.market.quote.symbol),
            amount: order.quantity.to_string(),
            price: order.price.to_string(),
            client_order_id: order.client_order_id.clone(),
        };

        let response = self
            .send(client, Method::POST, "market/orders/add", Some(serde_json::to_string(&body)?))
            .await?;
        let parsed: OrderResponse = read_json(response).await?;
        parsed.into_info()
    }

    async fn get_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<OrderInfo, ExchangeError> {
        let body = StatusBody { id: parse_order_id(order_id)? };
        let response = self
            .send(client, Method::POST, "market/orders/status", Some(serde_json::to_string(&body)?))
            .await?;
        let parsed: OrderResponse = read_json(response).await?;
        parsed.into_info()
    }

    async fn cancel_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<(), ExchangeError> {
        let body = CancelBody { order: parse_order_id(order_id)?, status: "canceled" };
        let response = self
            .send(
                client,
                Method::POST,
                "market/orders/update-status",
                Some(serde_json::to_string(&body)?),
            )
            .await?;
        let parsed: AckResponse = read_json(response).await?;
        ensure_ok(&parsed.status, parsed.code.as_deref(), parsed.message.as_deref())
    }
}

fn parse_order_id(order_id: &str) -> Result<u64, ExchangeError> {
    order_id
        .parse()
        .map_err(|_| ExchangeError::InvalidOrder(format!("nobitex order id must be a number: {order_id}")))
}

/// Nobitex reports many failures with HTTP 200 and `"status": "failed"`.
fn ensure_ok(status: &str, code: Option<&str>, message: Option<&str>) -> Result<(), ExchangeError> {
    if status == "ok" {
        Ok(())
    } else {
        Err(ExchangeError::Api(format!(
            "nobitex {status}: {} {}",
            code.unwrap_or_default(),
            message.unwrap_or_default()
        )))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderBody {
    #[serde(rename = "type")]
    kind: &'static str,
    execution: &'static str,
    src_currency: String,
    dst_currency: String,
    amount: String,
    price: String,
    client_order_id: String,
}

#[derive(Debug, Serialize)]
struct StatusBody {
    id: u64,
}

#[derive(Debug, Serialize)]
struct CancelBody {
    order: u64,
    status: &'static str,
}

#[derive(Debug, Deserialize)]
struct WalletsResponse {
    status: String,
    #[serde(default)]
    wallets: HashMap<String, Wallet>,
}

#[derive(Debug, Deserialize)]
struct Wallet {
    balance: Flex,
    blocked: Flex,
}

#[derive(Debug, Deserialize)]
struct AckResponse {
    status: String,
    code: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrderResponse {
    status: String,
    code: Option<String>,
    message: Option<String>,
    order: Option<NobitexOrder>,
}

impl OrderResponse {
    fn into_info(self) -> Result<OrderInfo, ExchangeError> {
        ensure_ok(&self.status, self.code.as_deref(), self.message.as_deref())?;
        let order = self
            .order
            .ok_or_else(|| ExchangeError::Api("nobitex response has no order".into()))?;
        Ok(OrderInfo {
            exchange_order_id: order.id.to_string(),
            client_order_id: order.client_order_id,
            status: map_status(&order.status)?,
            filled_quantity: order
                .matched_amount
                .map(|amount| amount.to_decimal())
                .transpose()?
                .unwrap_or(Decimal::ZERO),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NobitexOrder {
    id: u64,
    status: String,
    matched_amount: Option<Flex>,
    client_order_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    use super::*;
    use crate::engine::domain::models::{Coin, Market, OrderSide};
    use crate::exchange::test_support::{
        settings_for, settings_with_nobitex_api_key, settings_without_credentials,
    };

    /// Matches requests whose `Nobitex-Signature` is a valid Ed25519 signature,
    /// by `verifying_key`, of the timestamp, method, path+query and exact body.
    fn valid_signature(verifying_key: VerifyingKey) -> impl Fn(&Request) -> bool + Send + Sync + 'static {
        move |request: &Request| {
            let header = |name: &str| {
                request
                    .headers
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
            };
            let (Some(timestamp), Some(signature)) =
                (header("nobitex-timestamp"), header("nobitex-signature"))
            else {
                return false;
            };
            let Ok(bytes) = KEY_ENGINE.decode(&signature) else {
                return false;
            };
            let Ok(signature) = Signature::from_slice(&bytes) else {
                return false;
            };
            let full_path = match request.url.query() {
                Some(query) => format!("{}?{query}", request.url.path()),
                None => request.url.path().to_owned(),
            };
            let payload = format!(
                "{timestamp}{}{full_path}{}",
                request.method.as_str(),
                String::from_utf8_lossy(&request.body)
            );
            verifying_key.verify(payload.as_bytes(), &signature).is_ok()
        }
    }

    #[test]
    fn signature_matches_python_reference_vector() {
        // Generated with python `cryptography` using the algorithm from the Nobitex docs.
        let secret = Secret::new("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=");
        let signature =
            sign_request(&secret, "1700000000", "POST", "/market/orders/add", r#"{"a":1}"#).unwrap();
        assert_eq!(
            signature,
            "B7Xx_3sYOuz56y4c1dh0OiGe_Cw3-e8YPTGK1YzbJQNVWFycpftrGYXcXLFES3teuBpJpMldrRhgxpjVvBAkAg=="
        );
    }

    #[test]
    fn malformed_private_keys_are_rejected_without_leaking_them() {
        for bad in ["not base64 !!", "AAEC"] {
            let err = sign_request(&Secret::new(bad), "1", "GET", "/x", "").unwrap_err();
            assert!(matches!(err, ExchangeError::Auth(_)));
            assert!(!err.to_string().contains(bad));
        }
    }

    #[tokio::test]
    async fn api_key_mode_signs_get_and_post_requests() {
        let seed = [7u8; 32];
        let verifying_key = SigningKey::from_bytes(&seed).verifying_key();
        let settings =
            settings_with_nobitex_api_key("http://placeholder", "pub-key", &KEY_ENGINE.encode(seed));

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/wallets"))
            .and(header("nobitex-key", "pub-key"))
            .and(valid_signature(verifying_key))
            .and(|request: &Request| !request.headers.contains_key("authorization"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok", "wallets": {"USDT": {"balance": "2", "blocked": "0"}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/market/orders/add"))
            .and(header("nobitex-key", "pub-key"))
            .and(valid_signature(verifying_key))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok", "order": {"id": 1, "status": "Active", "matchedAmount": 0}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let mut settings = settings;
        settings.exchanges.nobitex = server.uri();
        let nobitex = NobitexClient::from_settings(&settings);
        let http = reqwest::Client::new();

        let balances = nobitex.get_balances(&http).await.unwrap();
        assert_eq!(balances[0].currency, "USDT");
        nobitex.place_order(&http, &usdt_irt_buy()).await.unwrap();
    }

    fn usdt_irt_buy() -> OrderRequest {
        OrderRequest {
            market: Market {
                base: Coin { symbol: "USDT".into() },
                quote: Coin { symbol: "IRT".into() },
            },
            side: OrderSide::Buy,
            price: Decimal::from(1_000_000),
            quantity: Decimal::from(10),
            client_order_id: "bot-1".into(),
        }
    }

    #[tokio::test]
    async fn balances_subtract_blocked_and_map_rials() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/wallets"))
            .and(header("authorization", "Token test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok",
                "wallets": {
                    "RLS": {"balance": "1000", "blocked": "250"},
                    "USDT": {"balance": "5.5", "blocked": "0"}
                }
            })))
            .mount(&server)
            .await;

        let nobitex = NobitexClient::from_settings(&settings_for(&server.uri()));
        let mut balances = nobitex.get_balances(&reqwest::Client::new()).await.unwrap();
        balances.sort_by(|a, b| a.currency.cmp(&b.currency));

        assert_eq!(balances[0].currency, "IRT");
        assert_eq!(balances[0].available, Decimal::from(750));
        assert_eq!(balances[0].locked, Decimal::from(250));
        assert_eq!(balances[1].currency, "USDT");
    }

    #[tokio::test]
    async fn place_order_sends_expected_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/market/orders/add"))
            .and(header("authorization", "Token test-key"))
            .and(body_json(json!({
                "type": "buy",
                "execution": "limit",
                "srcCurrency": "usdt",
                "dstCurrency": "rls",
                "amount": "10",
                "price": "1000000",
                "clientOrderId": "bot-1"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok",
                "order": {"id": 25, "status": "Active", "matchedAmount": 0, "clientOrderId": "bot-1"}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let nobitex = NobitexClient::from_settings(&settings_for(&server.uri()));
        let info = nobitex
            .place_order(&reqwest::Client::new(), &usdt_irt_buy())
            .await
            .unwrap();

        assert_eq!(info.exchange_order_id, "25");
        assert_eq!(info.status, OrderStatus::Ongoing);
        assert_eq!(info.filled_quantity, Decimal::ZERO);
        assert_eq!(info.client_order_id.as_deref(), Some("bot-1"));
    }

    #[tokio::test]
    async fn failed_status_becomes_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/market/orders/add"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "failed", "code": "SmallOrder", "message": "too small"
            })))
            .mount(&server)
            .await;

        let nobitex = NobitexClient::from_settings(&settings_for(&server.uri()));
        let err = nobitex
            .place_order(&reqwest::Client::new(), &usdt_irt_buy())
            .await
            .unwrap_err();

        assert!(err.to_string().contains("SmallOrder"), "{err}");
    }

    #[tokio::test]
    async fn status_and_cancel_use_numeric_ids() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/market/orders/status"))
            .and(body_json(json!({"id": 25})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok",
                "order": {"id": 25, "status": "Done", "matchedAmount": "10.0000000000"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/market/orders/update-status"))
            .and(body_json(json!({"order": 25, "status": "canceled"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok", "updatedStatus": "Canceled"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let nobitex = NobitexClient::from_settings(&settings_for(&server.uri()));
        let http = reqwest::Client::new();

        let info = nobitex.get_order(&http, "25").await.unwrap();
        assert_eq!(info.status, OrderStatus::Filled);
        assert_eq!(info.filled_quantity, Decimal::from(10));

        nobitex.cancel_order(&http, "25").await.unwrap();
        assert!(matches!(
            nobitex.cancel_order(&http, "abc").await,
            Err(ExchangeError::InvalidOrder(_))
        ));
    }

    #[tokio::test]
    async fn missing_credentials_fail_before_any_request() {
        let nobitex = NobitexClient::from_settings(&settings_without_credentials("http://127.0.0.1:1"));
        let err = nobitex.get_balances(&reqwest::Client::new()).await.unwrap_err();
        assert!(matches!(err, ExchangeError::MissingCredentials("nobitex")));
    }
}
