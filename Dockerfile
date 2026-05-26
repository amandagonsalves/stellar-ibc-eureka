FROM rust:1.88-slim-bookworm AS builder

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

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY contracts/ contracts/

RUN cargo build --release -p stellar-hermes-gateway --bin stellar-gateway

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/stellar-gateway /usr/local/bin/stellar-gateway

EXPOSE 50052
EXPOSE 8101

ENV STELLAR_GATEWAY_HOST=0.0.0.0

HEALTHCHECK --interval=10s --timeout=3s --start-period=15s --retries=3 \
    CMD curl -sf "http://127.0.0.1:${STELLAR_GATEWAY_HTTP_PORT:-8101}/health" > /dev/null || exit 1

ENTRYPOINT ["stellar-gateway"]
