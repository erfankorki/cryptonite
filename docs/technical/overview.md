# Technical Overview

## Goal

Load per-exchange credentials from the environment so the bot can read balances now
and place/cancel orders later, on Nobitex, Wallex and Bitpin.

## Layout

```
src/
  config/        Settings (config.toml + env), Secret type (redacted Debug)
  engine/domain/ Domain models: Market, OrderBook, Balance, OrderRequest, OrderInfo, ...
  exchange/
    clients.rs   Public market-data clients (order books)
    account/     Authenticated `Account` trait + nobitex.rs / wallex.rs / bitpin.rs
    decode.rs    Tolerant JSON/decimal parsing helpers
    error.rs     ExchangeError
    test_support.rs  Settings builders for tests
  main.rs        CLI: default = live order books, `balances [--amounts]`
  data/ ui/      Placeholders (no code yet)
```

## Configuration

* Non-secret values: `config.toml` (exchange base URLs).
* Secrets: environment variables, loaded from git-ignored `.env.local` then `.env`;
  real environment variables win. Empty values are ignored.
* Names: `CRYPTONITE__CREDENTIALS__<NOBITEX|WALLEX|BITPIN>__API_KEY` and `__API_SECRET`.
  See `.env.example`.
* `Secret` never prints its value in `Debug`.

## Exchange authentication

| Exchange | Scheme | Credentials |
| --- | --- | --- |
| Nobitex | Ed25519-signed API key. Headers `Nobitex-Key`, `Nobitex-Signature` (URL-safe base64), `Nobitex-Timestamp`. Signed text: `timestamp + METHOD + full_path(with query) + raw_body`. Needs an Iranian IP; User-Agent `TraderBot/Cryptonite-<ver>`. Fallback: login token (`Authorization: Token`) if only `API_KEY` is set | `API_KEY` = public `key`, `API_SECRET` = `privateKey` (shown once at creation) |
| Wallex | `X-API-Key` header | `API_KEY` |
| Bitpin | Login with key+secret -> JWT Bearer, cached ~10 min, refresh/login fallback; markets cached by numeric id; pagination `next` URLs are host-checked | `API_KEY` + `API_SECRET` (IP whitelist required by Bitpin) |

Domain mapping: IRT is `rls` on Nobitex, `TMN` on Wallex, market code `USDT_IRT` on Bitpin.

## Testing

* `cargo test` runs offline: wiremock servers imitate each exchange.
* The Ed25519 signing is checked against a reference vector generated independently in Python
  (payload `1700000000POST/market/orders/add{"a":1}`).
* Verified against the real Nobitex: signed `GET /v2/wallets` (15 funded wallets seen).

## Security rules

* Keys live only in `.env.local` on the developer's machine; never paste them in chat or commit them.
* Use read-only keys while developing; no WITHDRAW permission; use IP whitelisting.
* `balances` hides amounts unless `--amounts` is given.
* Nobitex may IP-block after many bad-auth requests.

## Decisions

* `async fn` in traits with static dispatch (no `dyn`).
* `reqwest` without the `query` feature; URLs are built manually.
* Dead code removed: `Exchange`, `Wallet`, `Order`, `ExchangeUrls::url`, `Credentials::get`, direct `uuid` dependency.

## Unverified / open items

* Price unit (rial vs toman) not yet checked live: next step is a `prices` command.
* Wallex and Bitpin implementations are tested only against mocks until keys are used.
* Place/cancel order has not been run live. Plan: tiny far-from-market order, then cancel
  (needs Nobitex TRADE permission, preferably a separate key).
* Unused crates still declared in `Cargo.toml`: `tracing`, `anyhow`, `chrono`, `diesel`, `ratatui`.
  `diesel` is the reason `libpq` is required on macOS; remove it until a database is introduced.
* `src/data` and `src/ui` are empty placeholders.
