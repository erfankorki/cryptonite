# Cryptonite developer commands (run `make help`).
# Works on macOS and Linux. On macOS it auto-detects Homebrew's libpq,
# which the diesel/postgres dependency needs at link time.

LIBPQ := $(shell brew --prefix libpq 2>/dev/null)
ifneq ($(LIBPQ),)
export LIBRARY_PATH := $(LIBPQ)/lib:$(LIBRARY_PATH)
endif

.PHONY: help setup build test lint fmt run prices balances balances-amounts

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-18s %s\n", $$1, $$2}'

setup: ## Create .env.local from the template (never overwrites)
	@test -f .env.local && echo ".env.local already exists" || (cp .env.example .env.local && echo "Created .env.local - fill in your keys")

build: ## Compile the project
	cargo build

test: ## Run all tests (offline, mock exchange servers)
	cargo test

lint: ## Run clippy
	cargo clippy --all-targets

fmt: ## Format the code
	cargo fmt

run: ## Live order books (default mode, refreshes every 3s)
	cargo run

balances: ## Show funded wallets on every configured exchange (amounts hidden)
	cargo run -- balances

balances-amounts: ## Same as balances, but print the amounts
	cargo run -- balances --amounts
