# Commands

All commands run from the repository root. `make help` lists them.

| Command | Cargo equivalent | What it does |
| --- | --- | --- |
| `make setup` | - | Creates `.env.local` from `.env.example` (never overwrites) |
| `make build` | `cargo build` | Compiles |
| `make test` | `cargo test` | Runs all tests offline (wiremock fake exchanges) |
| `make lint` | `cargo clippy --all-targets` | Lints |
| `make fmt` | `cargo fmt` | Formats |
| `make run` | `cargo run` | Live order books, refreshed every 3 s |
| `make balances` | `cargo balances` | Funded wallets per exchange, **amounts hidden** |
| `make balances-amounts` | `cargo balances-amounts` | Same, amounts shown |

`cargo balances*` come from the aliases in `.cargo/config.toml`.
The Makefile also adds Homebrew's `libpq` to `LIBRARY_PATH` (macOS link requirement of `diesel`).
Set `NO_COLOR=1` to disable colored output.

## First run

```sh
make setup            # then edit .env.local with your keys
make test
make balances
```

Exchanges without credentials are reported as "skipped"; the others still run.
