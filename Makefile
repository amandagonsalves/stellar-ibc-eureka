.PHONY: build push fmt test cargo-build

build:
	@if [ -z "$(SERVICE)" ]; then echo "usage: make build SERVICE=gateway|hermes|api"; exit 1; fi
	cargo run -q -p interstellar -- services build --$(SERVICE)

push:
	@if [ -z "$(SERVICE)" ]; then echo "usage: make push SERVICE=gateway|hermes|api"; exit 1; fi
	cargo run -q -p interstellar -- services push --$(SERVICE)

fmt:
	cargo fmt --all

test:
	cargo test --locked

cargo-build:
	cargo build

lint:
	cargo clippy --locked --all-targets -- -D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments -A clippy::result_large_err

install:
	cargo run -p interstellar -- install

push-all:
	cargo run -q -p interstellar -- services push