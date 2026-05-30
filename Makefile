.PHONY: fmt fmt-check lint test audit check

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt -- --check

lint:
	cargo clippy --locked --all-targets -- -D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments

test:
	cargo test --locked

audit:
	cargo audit --file Cargo.lock \
		--ignore RUSTSEC-2026-0104 \
		--ignore RUSTSEC-2026-0098 \
		--ignore RUSTSEC-2026-0099 \
		--ignore RUSTSEC-2026-0049 \
		--ignore RUSTSEC-2026-0009

check: fmt-check lint test

# crates/gateway
build-gateway:
	cargo build -p stellar-hermes-gateway

start-gateway:
	cargo run -p stellar-hermes-gateway

test-gateway:
	cargo test -p stellar-hermes-gateway

check-gateway:
	cd crates/gateway && cargo fmt && cargo clippy --locked --all-targets -- -D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments

# crates/api
build-api:
	cargo build -p stellar-api

start-api:
	cargo run -p stellar-api

test-api:
	cargo test -p stellar-api

check-api:
	cd crates/api && cargo fmt && cargo clippy --locked --all-targets -- -D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments

# crates/core
build-ibc-core:
	cargo build -p stellar-ibc-core

test-ibc-core:
	cargo test -p stellar-ibc-core

check-ibc-core:
	cd crates/stellar-ibc && cargo fmt && cargo clippy --locked --all-targets -- -D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments

# contracts
build-contracts:
	stellar contract build --profile contract

test-contracts:
	cd contracts && cargo test

check-contracts:
	cd contracts && cargo fmt && cargo clippy --locked --all-targets -- -D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments

# crates/integration-tests
run-integration-tests:
	cargo run -p stellar-integration-tests

# crates/osmosis
start-osmosis:
	cargo run -p stellar-osmosis -- start

start-osmosis-stateful:
	cargo run -p stellar-osmosis -- start --stateful

stop-osmosis:
	cargo run -p stellar-osmosis -- stop

health-osmosis:
	cargo run -p stellar-osmosis -- health

push-hermes:
	bash ci/flows/build-hermes-image.sh

push-gateway:
	bash ci/flows/build-gateway-image.sh

push-api:
	bash ci/flows/build-api-image.sh

COMPOSE := docker compose --profile local --profile hermes

# Whole stack
start-stellar-ibc:
	$(COMPOSE) up -d --build
	@echo ""
	@$(COMPOSE) ps

logs-stellar-ibc:
	$(COMPOSE) logs -f api gateway hermes

stop-stellar-ibc:
	$(COMPOSE) down

ps-stellar-ibc:
	$(COMPOSE) ps

restart-api:
	$(COMPOSE) rm -sf api
	$(COMPOSE) up -d api

logs-api:
	$(COMPOSE) logs -f api

restart-gateway:
	$(COMPOSE) rm -sf gateway
	$(COMPOSE) up -d gateway

restart-osmosis:
	$(COMPOSE) rm -sf osmosis
	$(COMPOSE) up -d osmosis

logs-gateway:
	$(COMPOSE) logs -f gateway

restart-hermes:
	$(COMPOSE) rm -sf hermes
	$(COMPOSE) up -d hermes

logs-hermes:
	$(COMPOSE) logs -f hermes

shell-hermes:
	$(COMPOSE) exec hermes sh

restart: restart-api restart-gateway restart-hermes

up: push-api push-gateway push-hermes restart-api restart-gateway restart-hermes

hermes-keys:
	@$(MAKE) -C ci hermes-keys

deploy-contracts:
	bash ci/flows/upload-and-deploy-contracts.sh
