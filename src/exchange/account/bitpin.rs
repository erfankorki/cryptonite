//! Bitpin account API (https://docs.bitpin.ir).
//!
//! Auth: the API key + secret are exchanged for a short-lived JWT pair via
//! `POST /v1/usr/api/login/`; requests then carry `Authorization: Bearer <access>`.
//! The access token lives ~15 min, so it is cached and renewed here.

use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::{ExchangeCredentials, Secret};
use crate::engine::domain::models::{
    Balance, ExchangeName, Market, OrderInfo, OrderRequest, OrderStatus,
};
use crate::exchange::clients::{BitpinClient, BitpinTokens};
use crate::exchange::decode::{Flex, parse_decimal, read_json, success_body, validate_id};
use crate::exchange::error::ExchangeError;
use crate::exchange::helper::{join_url, market_param};

use super::{Account, side_lower};

/// Renew the access token well before its ~15 minute lifetime ends.
const ACCESS_TTL: Duration = Duration::from_secs(10 * 60);
const IDENTIFIER_MAX_LEN: usize = 50;
const MAX_PAGES: usize = 20;
/// How many pages of recent orders `get_order` searches (Bitpin has no by-id lookup).
const ORDER_LOOKUP_PAGES: usize = 5;

fn map_state(state: &str, req_to_cancel: bool) -> Result<OrderStatus, ExchangeError> {
    match state.to_ascii_lowercase().as_str() {
        "initial" | "active" => Ok(OrderStatus::Ongoing),
        // A closed order that had a cancel request was cancelled, otherwise it filled.
        "closed" if req_to_cancel => Ok(OrderStatus::Cancelled),
        "closed" => Ok(OrderStatus::Filled),
        other => Err(ExchangeError::Api(format!("unknown bitpin order state: {other}"))),
    }
}

impl BitpinClient {
    fn credentials(&self) -> Result<&ExchangeCredentials, ExchangeError> {
        self.credentials
            .as_ref()
            .ok_or(ExchangeError::MissingCredentials("bitpin"))
    }

    fn url(&self, path: &str) -> String {
        join_url(&self.base_url, path)
    }

    /// Pagination links are followed with our token attached, so only links
    /// that stay on the configured host are accepted.
    fn check_next(&self, next: String) -> Result<String, ExchangeError> {
        if next.starts_with(self.base_url.trim_end_matches('/')) {
            Ok(next)
        } else {
            Err(ExchangeError::Api(
                "bitpin returned a next-page URL on a different host".into(),
            ))
        }
    }

    async fn login(
        &self,
        client: &reqwest::Client,
        credentials: &ExchangeCredentials,
    ) -> Result<BitpinTokens, ExchangeError> {
        let secret = credentials
            .api_secret
            .as_ref()
            .ok_or_else(|| ExchangeError::Auth("bitpin needs both api_key and api_secret".into()))?;
        let body = LoginBody {
            api_key: credentials.api_key.expose(),
            secret_key: secret.expose(),
        };

        let response = client
            .post(self.url("v1/usr/api/login/"))
            .json(&body)
            .send()
            .await?;
        let parsed: LoginResponse = read_json(response)
            .await
            .map_err(|err| ExchangeError::Auth(err.to_string()))?;

        Ok(BitpinTokens {
            access: Secret::new(parsed.access),
            refresh: Secret::new(parsed.refresh),
            obtained_at: Instant::now(),
        })
    }

    async fn refresh(&self, client: &reqwest::Client, refresh: &Secret) -> Result<String, ExchangeError> {
        let body = RefreshBody { refresh: refresh.expose() };
        let response = client
            .post(self.url("v1/usr/refresh_token/"))
            .json(&body)
            .send()
            .await?;
        let parsed: RefreshResponse = read_json(response).await?;
        Ok(parsed.access)
    }

    /// Returns a valid access token, refreshing or logging in when needed.
    async fn access_token(&self, client: &reqwest::Client) -> Result<String, ExchangeError> {
        let credentials = self.credentials()?;
        let mut guard = self.tokens.lock().await;

        if let Some(tokens) = guard.as_mut() {
            if tokens.obtained_at.elapsed() < ACCESS_TTL {
                return Ok(tokens.access.expose().to_owned());
            }
            // The refresh token lasts about a week; if it fails, log in again.
            if let Ok(access) = self.refresh(client, &tokens.refresh).await {
                tokens.access = Secret::new(access.clone());
                tokens.obtained_at = Instant::now();
                return Ok(access);
            }
        }

        let tokens = self.login(client, credentials).await?;
        let access = tokens.access.expose().to_owned();
        *guard = Some(tokens);
        Ok(access)
    }

    async fn get_authed<T: DeserializeOwned>(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<T, ExchangeError> {
        let token = self.access_token(client).await?;
        let response = client.get(url).bearer_auth(token).send().await?;
        read_json(response).await
    }

    /// Bitpin orders reference a numeric market id instead of a symbol.
    async fn market_id(&self, client: &reqwest::Client, market: &Market) -> Result<u64, ExchangeError> {
        let code = market_param(ExchangeName::Bitpin, market);
        if let Some(id) = self.market_ids.lock().await.get(&code) {
            return Ok(*id);
        }

        let response = client.get(self.url("v1/mkt/markets/")).send().await?;
        let markets = match read_json::<MarketsResponse>(response).await? {
            MarketsResponse::List(list) => list,
            MarketsResponse::Page { results } => results,
        };

        let mut cache = self.market_ids.lock().await;
        for market in markets {
            cache.insert(market.code.to_uppercase(), market.id);
        }
        cache
            .get(&code)
            .copied()
            .ok_or_else(|| ExchangeError::Api(format!("bitpin market {code} not found")))
    }
}

impl Account for BitpinClient {
    async fn get_balances(&self, client: &reqwest::Client) -> Result<Vec<Balance>, ExchangeError> {
        let mut url = self.url("v1/wlt/wallets/");
        let mut balances = Vec::new();

        for _ in 0..MAX_PAGES {
            let page: Page<Wallet> = self.get_authed(client, &url).await?;
            for wallet in page.results {
                balances.push(Balance {
                    currency: wallet.currency.code.to_uppercase(),
                    available: parse_decimal(&wallet.balance)?,
                    locked: parse_decimal(&wallet.frozen)?,
                });
            }
            match page.next {
                Some(next) => url = self.check_next(next)?,
                None => return Ok(balances),
            }
        }
        Err(ExchangeError::Api("bitpin wallets: too many pages".into()))
    }

    async fn place_order(
        &self,
        client: &reqwest::Client,
        order: &OrderRequest,
    ) -> Result<OrderInfo, ExchangeError> {
        validate_id(&order.client_order_id, IDENTIFIER_MAX_LEN)?;
        let market = self.market_id(client, &order.market).await?;
        let price = order.price.to_string();
        let body = OrderBody {
            market,
            amount1: order.quantity.to_string(),
            price: price.clone(),
            price_limit: price,
            mode: "limit",
            kind: side_lower(order.side),
            identifier: order.client_order_id.clone(),
        };

        let token = self.access_token(client).await?;
        let response = client
            .post(self.url("v1/odr/orders/"))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await?;
        let parsed: BitpinOrder = read_json(response).await?;
        parsed.into_info()
    }

    async fn get_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<OrderInfo, ExchangeError> {
        let id: u64 = order_id.parse().map_err(|_| {
            ExchangeError::InvalidOrder(format!("bitpin order id must be a number: {order_id}"))
        })?;

        // No by-id endpoint is documented: search the newest orders instead.
        let mut url = self.url("v1/odr/orders/");
        for _ in 0..ORDER_LOOKUP_PAGES {
            let page: Page<BitpinOrder> = self.get_authed(client, &url).await?;
            if let Some(found) = page.results.into_iter().find(|order| order.id == id) {
                return found.into_info();
            }
            match page.next {
                Some(next) => url = self.check_next(next)?,
                None => break,
            }
        }
        Err(ExchangeError::Api(format!("bitpin order {id} not found in recent orders")))
    }

    async fn cancel_order(
        &self,
        client: &reqwest::Client,
        order_id: &str,
    ) -> Result<(), ExchangeError> {
        let id: u64 = order_id.parse().map_err(|_| {
            ExchangeError::InvalidOrder(format!("bitpin order id must be a number: {order_id}"))
        })?;

        let token = self.access_token(client).await?;
        let response = client
            .delete(self.url(&format!("v1/odr/orders/{id}/")))
            .bearer_auth(token)
            .send()
            .await?;
        // 204 means the cancel request was accepted; the order closes shortly after.
        success_body(response).await.map(|_| ())
    }
}

#[derive(Debug, Serialize)]
struct LoginBody<'a> {
    api_key: &'a str,
    secret_key: &'a str,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    access: String,
    refresh: String,
}

#[derive(Debug, Serialize)]
struct RefreshBody<'a> {
    refresh: &'a str,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access: String,
}

#[derive(Debug, Deserialize)]
struct Page<T> {
    next: Option<String>,
    results: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct Wallet {
    currency: WalletCurrency,
    balance: String,
    frozen: String,
}

#[derive(Debug, Deserialize)]
struct WalletCurrency {
    code: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MarketsResponse {
    List(Vec<BitpinMarket>),
    Page { results: Vec<BitpinMarket> },
}

#[derive(Debug, Deserialize)]
struct BitpinMarket {
    id: u64,
    code: String,
}

#[derive(Debug, Serialize)]
struct OrderBody {
    market: u64,
    amount1: String,
    price: String,
    price_limit: String,
    mode: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    identifier: String,
}

#[derive(Debug, Deserialize)]
struct BitpinOrder {
    id: u64,
    state: String,
    #[serde(default)]
    req_to_cancel: bool,
    exchanged1: Option<Flex>,
    identifier: Option<String>,
}

impl BitpinOrder {
    fn into_info(self) -> Result<OrderInfo, ExchangeError> {
        Ok(OrderInfo {
            exchange_order_id: self.id.to_string(),
            client_order_id: self.identifier,
            status: map_state(&self.state, self.req_to_cancel)?,
            filled_quantity: self
                .exchanged1
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
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::engine::domain::models::{Coin, OrderSide};
    use crate::exchange::test_support::{settings_for, settings_without_credentials};

    async fn mock_login(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/v1/usr/api/login/"))
            .and(body_json(json!({"api_key": "test-key", "secret_key": "test-secret"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access": "access-1", "refresh": "refresh-1"
            })))
            .expect(1)
            .mount(server)
            .await;
    }

    fn usdt_irt_buy() -> OrderRequest {
        OrderRequest {
            market: Market {
                base: Coin { symbol: "USDT".into() },
                quote: Coin { symbol: "IRT".into() },
            },
            side: OrderSide::Buy,
            price: Decimal::from(100_000),
            quantity: Decimal::from(10),
            client_order_id: "bot-3".into(),
        }
    }

    #[tokio::test]
    async fn balances_log_in_once_and_follow_pages_on_same_host() {
        let server = MockServer::start().await;
        mock_login(&server).await;

        let second_page = format!("{}/v1/wlt/wallets/?page=2", server.uri());
        Mock::given(method("GET"))
            .and(path("/v1/wlt/wallets/"))
            .and(header("authorization", "Bearer access-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "count": 2, "next": second_page, "previous": null,
                "results": [{"currency": {"code": "usdt"}, "balance": "5.5", "frozen": "1.5"}]
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/wlt/wallets/"))
            .and(header("authorization", "Bearer access-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "count": 2, "next": null, "previous": null,
                "results": [{"currency": {"code": "IRT"}, "balance": "1000", "frozen": "0"}]
            })))
            .mount(&server)
            .await;

        let bitpin = BitpinClient::from_settings(&settings_for(&server.uri()));
        let balances = bitpin.get_balances(&reqwest::Client::new()).await.unwrap();

        assert_eq!(balances.len(), 2);
        assert_eq!(balances[0].currency, "USDT");
        assert_eq!(balances[0].locked, Decimal::new(15, 1));
        assert_eq!(balances[1].currency, "IRT");
    }

    #[tokio::test]
    async fn next_page_on_another_host_is_rejected() {
        let server = MockServer::start().await;
        mock_login(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/wlt/wallets/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "next": "https://evil.example/steal", "results": []
            })))
            .mount(&server)
            .await;

        let bitpin = BitpinClient::from_settings(&settings_for(&server.uri()));
        let err = bitpin.get_balances(&reqwest::Client::new()).await.unwrap_err();

        assert!(err.to_string().contains("different host"), "{err}");
    }

    #[tokio::test]
    async fn place_order_looks_up_market_id_and_sends_expected_body() {
        let server = MockServer::start().await;
        mock_login(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/mkt/markets/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": 7, "code": "BTC_IRT"}, {"id": 12, "code": "USDT_IRT"}
            ])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/odr/orders/"))
            .and(header("authorization", "Bearer access-1"))
            .and(body_json(json!({
                "market": 12,
                "amount1": "10",
                "price": "100000",
                "price_limit": "100000",
                "mode": "limit",
                "type": "buy",
                "identifier": "bot-3"
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": 99, "state": "active", "identifier": "bot-3", "exchanged1": "0"
            })))
            .expect(2)
            .mount(&server)
            .await;

        let bitpin = BitpinClient::from_settings(&settings_for(&server.uri()));
        let http = reqwest::Client::new();

        let info = bitpin.place_order(&http, &usdt_irt_buy()).await.unwrap();
        assert_eq!(info.exchange_order_id, "99");
        assert_eq!(info.status, OrderStatus::Ongoing);

        // Second order: market id comes from the cache (markets mock expects 1 call).
        bitpin.place_order(&http, &usdt_irt_buy()).await.unwrap();
    }

    #[tokio::test]
    async fn get_order_searches_recent_orders_and_cancel_calls_delete() {
        let server = MockServer::start().await;
        mock_login(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/odr/orders/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "next": null,
                "results": [
                    {"id": 100, "state": "active"},
                    {"id": 99, "state": "closed", "req_to_cancel": true, "exchanged1": "2.5"}
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/v1/odr/orders/99/"))
            .and(header("authorization", "Bearer access-1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let bitpin = BitpinClient::from_settings(&settings_for(&server.uri()));
        let http = reqwest::Client::new();

        let info = bitpin.get_order(&http, "99").await.unwrap();
        assert_eq!(info.status, OrderStatus::Cancelled);
        assert_eq!(info.filled_quantity, Decimal::new(25, 1));
        assert!(bitpin.get_order(&http, "5").await.is_err());

        bitpin.cancel_order(&http, "99").await.unwrap();
    }

    #[tokio::test]
    async fn login_failure_is_an_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/usr/api/login/"))
            .respond_with(ResponseTemplate::new(406).set_body_json(json!({"detail": "bad credentials"})))
            .mount(&server)
            .await;

        let bitpin = BitpinClient::from_settings(&settings_for(&server.uri()));
        let err = bitpin.get_balances(&reqwest::Client::new()).await.unwrap_err();

        assert!(matches!(err, ExchangeError::Auth(_)), "{err}");
    }

    #[tokio::test]
    async fn missing_credentials_fail_before_any_request() {
        let bitpin = BitpinClient::from_settings(&settings_without_credentials("http://127.0.0.1:1"));
        let err = bitpin.get_balances(&reqwest::Client::new()).await.unwrap_err();
        assert!(matches!(err, ExchangeError::MissingCredentials("bitpin")));
    }
}
