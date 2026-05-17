# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.91-slim-bookworm AS builder

WORKDIR /build

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        protobuf-compiler \
        make \
        gcc \
        libc-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency graph before copying full source.
COPY Cargo.toml Cargo.lock ./
COPY crates/ibc/Cargo.toml      crates/ibc/Cargo.toml
COPY crates/gateway/Cargo.toml  crates/gateway/Cargo.toml

RUN mkdir -p crates/ibc/src crates/gateway/src \
    && echo "pub fn _stub() {}" > crates/ibc/src/lib.rs \
    && echo "fn main() {}"      > crates/gateway/src/main.rs \
    && cargo build --release -p stellar-hermes-gateway 2>/dev/null || true

# Build the real binary.
COPY crates/ crates/
RUN touch crates/ibc/src/lib.rs crates/gateway/src/main.rs \
    && cargo build --release -p stellar-hermes-gateway

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/stellar-gateway ./stellar-gateway

# gRPC — Hermes fork connects here
EXPOSE 50052
# HTTP — health endpoint + REST queries
EXPOSE 8001

ENV STELLAR_GATEWAY_HOST=0.0.0.0 \
    STELLAR_GATEWAY_GRPC_PORT=50052 \
    STELLAR_GATEWAY_HTTP_PORT=8001 \
    STELLAR_RPC_URL=https://soroban-testnet.stellar.org \
    NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

ENTRYPOINT ["./stellar-gateway"]
