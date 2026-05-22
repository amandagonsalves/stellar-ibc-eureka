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
