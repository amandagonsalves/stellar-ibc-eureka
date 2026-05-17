.PHONY: fmt fmt-check lint test audit check

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt -- --check

lint:
	cargo clippy --locked --all-targets -- \
		-D warnings \
		-A clippy::manual_is_multiple_of \
		-A clippy::too_many_arguments

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
