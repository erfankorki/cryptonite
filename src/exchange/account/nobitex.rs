//! Nobitex account API (https://apidocs.nobitex.ir).
//!
//! Auth: `Authorization: Token <token>`; the token goes in `api_key`.

use std::collections::HashMap;

use reqwest::header::AUTHORIZATION;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::engine::domain::models::{Balance, OrderInfo, OrderRequest, OrderStatus};
use crate::exchange::clients::NobitexClient;
use crate::exchange::decode::{Flex, read_json, validate_id};
use crate::exchange::error::ExchangeError;
use crate::exchange::helper::join_url;

use super::{Account, side_lower};

const CLIENT_ORDER_ID_MAX_LEN: usize = 32;

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
    fn auth_value(&self) -> Result<String, ExchangeError> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::MissingCredentials("nobitex"))?;
        Ok(format!("Token {}", credentials.api_key.expose()))
    }

    fn url(&self, path: &str) -> String {
        join_url(&self.base_url, path)
    }
}

impl Account for NobitexClient {
    async fn get_balances(&self, client: &reqwest::Client) -> Result<Vec<Balance>, ExchangeError> {
        let response = client
            .get(self.url("v2/wallets"))
            .header(AUTHORIZATION, self.auth_value()?)
            .send()
            .await?;
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

        let response = client
            .post(self.url("market/orders/add"))
            .header(AUTHORIZATION, self.auth_value()?)
            .json(&body)
            .send()
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
        let response = client
            .post(self.url("market/orders/status"))
            .header(AUTHORIZATION, self.auth_value()?)
            .json(&body)
            .send()
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
        let response = client
            .post(self.url("market/orders/update-status"))
            .header(AUTHORIZATION, self.auth_value()?)
            .json(&body)
            .send()
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
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::engine::domain::models::{Coin, Market, OrderSide};
    use crate::exchange::test_support::{settings_for, settings_without_credentials};

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
