//! Wallex account API (https://api-docs.wallex.ir).
//!
//! Auth: `X-API-Key: <key>` header, no request signature.
//! Orders are addressed by `clientOrderId` (which Wallex generates itself).

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::engine::domain::models::{Balance, ExchangeName, OrderInfo, OrderRequest, OrderStatus};
use crate::exchange::clients::WallexClient;
use crate::exchange::decode::{Flex, read_json, success_body, validate_id};
use crate::exchange::error::ExchangeError;
use crate::exchange::helper::{join_url, market_param};

use super::{Account, side_lower};

const API_KEY_HEADER: &str = "X-API-Key";
const CLIENT_ID_MAX_LEN: usize = 64;

/// Wallex calls the Iranian toman `TMN`; the domain uses `IRT` everywhere.
fn from_wallex_currency(symbol: &str) -> String {
    if symbol.eq_ignore_ascii_case("TMN") {
        "IRT".to_owned()
    } else {
        symbol.to_uppercase()
    }
}

fn map_status(status: &str) -> Result<OrderStatus, ExchangeError> {
    match status.to_ascii_uppercase().as_str() {
        "NEW" | "PARTIALLY_FILLED" => Ok(OrderStatus::Ongoing),
        "FILLED" => Ok(OrderStatus::Filled),
        "CANCELED" | "CANCELLED" | "EXPIRED" | "REJECTED" => Ok(OrderStatus::Cancelled),
        other => Err(ExchangeError::Api(format!("unknown wallex order status: {other}"))),
    }
}

impl WallexClient {
    fn api_key(&self) -> Result<&str, ExchangeError> {
        self.credentials
            .as_ref()
            .map(|credentials| credentials.api_key.expose())
            .ok_or(ExchangeError::MissingCredentials("wallex"))
    }

    fn url(&self, path: &str) -> String {
        join_url(&self.base_url, path)
    }
}

impl Account for WallexClient {
    async fn get_balances(&self, client: &reqwest::Client) -> Result<Vec<Balance>, ExchangeError> {
        let response = client
            .get(self.url("v1/account/balances"))
            .header(API_KEY_HEADER, self.api_key()?)
            .send()
            .await?;
        let body: Envelope<BalancesResult> = read_json(response).await?;
        let result = body.into_result()?;

        result
            .balances
            .into_iter()
            .map(|(currency, wallet)| {
                Ok(Balance {
                    currency: from_wallex_currency(&currency),
                    available: wallet.value.to_decimal()?,
                    locked: wallet.locked.to_decimal()?,
                })
            })
            .collect()
    }

    async fn place_order(
        &self,
        client: &reqwest::Client,
        order: &OrderRequest,
    ) -> Result<OrderInfo, ExchangeError> {
        validate_id(&order.client_order_id, CLIENT_ID_MAX_LEN)?;
        let body = OrderBody {
            symbol: market_param(ExchangeName::Wallex, &order.market),
            kind: "LIMIT",
            side: side_lower(order.side).to_uppercase(),
            price: order.price.to_string(),
            quantity: order.quantity.to_string(),
            client_id: order.client_order_id.clone(),
        };

        let response = client
            .post(self.url("v1/account/orders"))
            .header(API_KEY_HEADER, self.api_key()?)
            .json(&body)
            .send()
            .await?;
        let parsed: Envelope<WallexOrder> = read_json(response).await?;
        parsed.into_result()?.into_info()
    }

    async fn get_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<OrderInfo, ExchangeError> {
        validate_id(order_id, CLIENT_ID_MAX_LEN)?;
        let response = client
            .get(self.url(&format!("v1/account/orders/{order_id}")))
            .header(API_KEY_HEADER, self.api_key()?)
            .send()
            .await?;
        let parsed: Envelope<WallexOrder> = read_json(response).await?;
        parsed.into_result()?.into_info()
    }

    async fn cancel_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<(), ExchangeError> {
        validate_id(order_id, CLIENT_ID_MAX_LEN)?;
        let response = client
            .delete(self.url(&format!("v1/account/orders?clientOrderId={order_id}")))
            .header(API_KEY_HEADER, self.api_key()?)
            .send()
            .await?;
        // The docs show no response body for cancellation, so only a clear
        // `"success": false` is treated as a failure besides a bad HTTP status.
        let text = success_body(response).await?;
        match serde_json::from_str::<Ack>(&text) {
            Ok(Ack { success: Some(false), message }) => Err(ExchangeError::Api(format!(
                "wallex cancel failed: {}",
                message.unwrap_or_default()
            ))),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Serialize)]
struct OrderBody {
    symbol: String,
    #[serde(rename = "type")]
    kind: &'static str,
    side: String,
    price: String,
    quantity: String,
    client_id: String,
}

#[derive(Debug, Deserialize)]
struct Ack {
    success: Option<bool>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    success: bool,
    message: Option<String>,
    result: Option<T>,
}

impl<T> Envelope<T> {
    fn into_result(self) -> Result<T, ExchangeError> {
        match (self.success, self.result) {
            (true, Some(result)) => Ok(result),
            _ => Err(ExchangeError::Api(format!(
                "wallex request failed: {}",
                self.message.unwrap_or_default()
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BalancesResult {
    balances: HashMap<String, WalletBalance>,
}

#[derive(Debug, Deserialize)]
struct WalletBalance {
    value: Flex,
    locked: Flex,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WallexOrder {
    client_order_id: String,
    status: String,
    executed_qty: Option<Flex>,
}

impl WallexOrder {
    fn into_info(self) -> Result<OrderInfo, ExchangeError> {
        Ok(OrderInfo {
            exchange_order_id: self.client_order_id.clone(),
            client_order_id: Some(self.client_order_id),
            status: map_status(&self.status)?,
            filled_quantity: self
                .executed_qty
                .map(|quantity| quantity.to_decimal())
                .transpose()?
                .unwrap_or(Decimal::ZERO),
        })
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::engine::domain::models::{Coin, Market, OrderSide};
    use crate::exchange::test_support::{settings_for, settings_without_credentials};

    fn usdt_irt_sell() -> OrderRequest {
        OrderRequest {
            market: Market {
                base: Coin { symbol: "USDT".into() },
                quote: Coin { symbol: "IRT".into() },
            },
            side: OrderSide::Sell,
            price: Decimal::from(100_000),
            quantity: Decimal::new(15, 1),
            client_order_id: "bot-2".into(),
        }
    }

    #[tokio::test]
    async fn balances_map_toman_and_read_string_amounts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/account/balances"))
            .and(header("x-api-key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "result": {"balances": {
                    "TMN": {"asset": "TMN", "value": "500000", "locked": "100"},
                    "USDT": {"asset": "USDT", "value": "10.00000000", "locked": "0.00000000"}
                }}
            })))
            .mount(&server)
            .await;

        let wallex = WallexClient::from_settings(&settings_for(&server.uri()));
        let mut balances = wallex.get_balances(&reqwest::Client::new()).await.unwrap();
        balances.sort_by(|a, b| a.currency.cmp(&b.currency));

        assert_eq!(balances[0].currency, "IRT");
        assert_eq!(balances[0].available, Decimal::from(500_000));
        assert_eq!(balances[0].locked, Decimal::from(100));
        assert_eq!(balances[1].currency, "USDT");
    }

    #[tokio::test]
    async fn place_order_sends_expected_body_and_returns_client_order_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/account/orders"))
            .and(header("x-api-key", "test-key"))
            .and(body_json(json!({
                "symbol": "USDTTMN",
                "type": "LIMIT",
                "side": "SELL",
                "price": "100000",
                "quantity": "1.5",
                "client_id": "bot-2"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "result": {
                    "clientOrderId": "LIMIT-abc",
                    "status": "NEW",
                    "executedQty": "0.0000000000000000"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let wallex = WallexClient::from_settings(&settings_for(&server.uri()));
        let info = wallex
            .place_order(&reqwest::Client::new(), &usdt_irt_sell())
            .await
            .unwrap();

        assert_eq!(info.exchange_order_id, "LIMIT-abc");
        assert_eq!(info.status, OrderStatus::Ongoing);
        assert_eq!(info.filled_quantity, Decimal::ZERO);
    }

    #[tokio::test]
    async fn status_and_cancel_use_client_order_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/account/orders/LIMIT-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "result": {"clientOrderId": "LIMIT-abc", "status": "FILLED", "executedQty": "1.5"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/v1/account/orders"))
            .and(query_param("clientOrderId", "LIMIT-abc"))
            .and(header("x-api-key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
            .expect(1)
            .mount(&server)
            .await;

        let wallex = WallexClient::from_settings(&settings_for(&server.uri()));
        let http = reqwest::Client::new();

        let info = wallex.get_order(&http, "LIMIT-abc").await.unwrap();
        assert_eq!(info.status, OrderStatus::Filled);
        assert_eq!(info.filled_quantity, Decimal::new(15, 1));

        wallex.cancel_order(&http, "LIMIT-abc").await.unwrap();
        assert!(matches!(
            wallex.cancel_order(&http, "a/b").await,
            Err(ExchangeError::InvalidOrder(_))
        ));
    }

    #[tokio::test]
    async fn success_false_becomes_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/account/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": false, "message": "insufficient balance"
            })))
            .mount(&server)
            .await;

        let wallex = WallexClient::from_settings(&settings_for(&server.uri()));
        let err = wallex
            .place_order(&reqwest::Client::new(), &usdt_irt_sell())
            .await
            .unwrap_err();

        assert!(err.to_string().contains("insufficient balance"), "{err}");
    }

    #[tokio::test]
    async fn missing_credentials_fail_before_any_request() {
        let wallex = WallexClient::from_settings(&settings_without_credentials("http://127.0.0.1:1"));
        let err = wallex.get_balances(&reqwest::Client::new()).await.unwrap_err();
        assert!(matches!(err, ExchangeError::MissingCredentials("wallex")));
    }
}
