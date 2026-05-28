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

RUN cargo build --release --bin stellar-gateway --bin stellar-api

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        curl \
        netcat-openbsd \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/stellar-gateway /usr/local/bin/stellar-gateway
COPY --from=builder /build/target/release/stellar-api /usr/local/bin/stellar-api

EXPOSE 50052
EXPOSE 8101

ENV STELLAR_GATEWAY_HOST=0.0.0.0
ENV STELLAR_API_HOST=0.0.0.0

ENTRYPOINT ["stellar-gateway"]
