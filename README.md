<p align="center">
  <img src="docs/assets/thumbnail.png" alt="Stellar IBC Bridge" width="100%" />
</p>

# Stellar IBC Bridge

Rust implementation of **IBC v2 (Eureka)** for the Stellar network, enabling trustless
cross-chain communication between Stellar and Cosmos-compatible chains.

This repository is part of the **Cardano–Stellar IBC bridge** project. It ships:

- **`stellar-hermes-gateway`** — gRPC gateway the Hermes relayer talks to. Speaks no
  Soroban RPC directly; every chain read/write goes through `stellar-api`.
- **`stellar-api`** — standalone HTTP/REST service that owns the Soroban RPC
  connection and Stellar signing key. Exposes `/ledger/*`, `/account/*`,
  `/balance/*`, `/tx/*` for the gateway to call.
- **`stellar-ibc-core`** — shared library: SMT, ICS-23 proof serializer, IBC
  protocol context, plus the `RpcClient` (Soroban JSON-RPC) and `ApiClient`
  (HTTP client the gateway uses to reach `stellar-api`).
- **`stellar-osmosis`** — local Osmosis (`localosmosis`) lifecycle crate.
  Boots a prebuilt `osmolabs/osmosis:<ver>-alpine` image from a declarative
  `default-config.json`. Acts as the Cosmos counterparty for local devnets.
- **`light-client-wasm`** — Stellar light client compiled to
  `wasm32-unknown-unknown`, uploaded to the Cosmos chain via `08-wasm`.
- **Soroban contracts** — `router`, `transfer-app`, and light clients (`mock`,
  `attestation`, `tendermint`) under `contracts/`.
- **Hermes fork integration** — `relayer-types::clients::ics10_stellar` and
  `chain::stellar::StellarChainEndpoint` live in the
  [cardano-foundation/hermes-relayer](https://github.com/cardano-foundation/hermes-relayer)
  fork.

Related repositories:

| Repo | Role |
|---|---|
| [stellar-ibc](https://github.com/amandagonsalves/stellar-ibc) | This repo |
| [hermes-relayer](https://github.com/cardano-foundation/hermes-relayer) (fork) | Relayer with `StellarChainEndpoint` and `ics10_stellar` types |
| [cardano-ibc-incubator](https://github.com/cardano-foundation/cardano-ibc-incubator) | Cosmos entrypoint chain, `proto-types`, original `caribic` CLI |

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Repository Structure](#repository-structure)
- [gRPC API](#grpc-api)
- [HTTP API](#http-api)
- [Configuration](#configuration)
- [Running](#running)
- [Manual testing](#manual-testing)
- [Development](#development)

---

## Overview

**IBC version:** v2 (Eureka) — no connection or channel handshake.
**On-chain runtime:** Soroban (Stellar's WebAssembly smart-contract platform).
**Relayer:** Hermes fork with a `StellarChainEndpoint`.
**Counterparty:** Cosmos chain (ibc-go v10) hosting an `08-wasm` Stellar light client.

IBC v2 collapses the v1 protocol significantly:

| IBC v1 (Cardano) | IBC v2 (Stellar) |
|---|---|
| `ConnectionOpenInit/Try/Ack/Confirm` | Not needed |
| `ChannelOpenInit/Try/Ack/Confirm` | Not needed |
| Channel version negotiation | Per-packet `payload.version` field |
| Port-based app routing via channel binding | `sourcePort` / `destPort` on each payload |
| Multi-step connection / channel creation | `registerCounterparty(clientId, prefix)` — one call |
| 8 paths in the provable store | **3** packet-related paths only (ICS-24 v2 §Provable Path-space) |

After client creation, a single `registerCounterparty` call is enough; packets flow
immediately.

The three provable paths in v2:

| Value | Path bytes |
|---|---|
| Packet Commitment | `{sourceClientId} \|\| 0x01 \|\| be64(sequence)` |
| Packet Receipt | `{destClientId} \|\| 0x02 \|\| be64(sequence)` |
| Acknowledgement Commitment | `{destClientId} \|\| 0x03 \|\| be64(sequence)` |

The SMT (fixed-depth-64 binary Merkle tree, Cardano-compatible) lives in
[`crates/core/src/ibc/smt.rs`](crates/core/src/ibc/smt.rs); the ICS-23
`MerkleProof` serializer (membership + non-membership) is in
[`crates/core/src/ibc/proof.rs`](crates/core/src/ibc/proof.rs).

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                Hermes relayer fork (cardano-foundation)                 │
│  crates/relayer/src/chain/stellar/StellarChainEndpoint                  │
│  crates/relayer-types/src/clients/ics10_stellar/                        │
└───────────────────┬──────────────────────────────┬──────────────────────┘
                    │ gRPC :50052                  │ Tendermint RPC :26657
                    ▼                              │
┌─────────────────────────────────────────────┐    │
│  stellar-hermes-gateway                     │    │
│   tonic StellarGatewayQuery + Msg services  │    │
│   StateTracker (SMT root + ICS-23 proofs)   │    │
│   ApiClient (HTTP) ──────────┐              │    │
└─────────────────────────────│──────────────┘    │
                              │ HTTP :8101         │
                              ▼                    │
┌─────────────────────────────────────────────┐    │
│  stellar-api                                │    │
│   axum routes:                              │    │
│     /health                                 │    │
│     /ledger/latest · /ledger/{seq}          │    │
│     /account/{addr} · /balance/{addr}       │    │
│     /tx/xdr · /tx/sign · /tx/submit         │    │
│     /tx/{tx_hash}                           │    │
│   owns Soroban RpcClient + signing key      │    │
└──────────────────┬──────────────────────────┘    │
                   │ JSON-RPC                       │
                   ▼                                ▼
┌─────────────────────────────────┐  ┌──────────────────────────────────┐
│  Stellar Soroban RPC            │  │  localosmosis (Cosmos)           │
│  soroban-testnet.stellar.org    │  │  osmolabs/osmosis:31-alpine      │
│  or in-compose stellar-node     │  │  ibc-go v10 + 08-wasm            │
└──────────────────┬──────────────┘  └─────────────────────────────────┬┘
                   │ contract invokes                                   │
                   ▼                                                    │
┌─────────────────────────────────────────────────────────────────────┐ │
│  Soroban contracts (contracts/)                                     │ │
│   router                Routes IBC v2 packets to apps               │ │
│   transfer-app          ICS-20 transfer module                      │ │
│   light-clients/mock    Always-accept LC for development            │ │
│   light-clients/        attestation, tendermint (pending)           │ │
│   light-client-wasm     Stellar LC, packaged as wasm for 08-wasm ───┘
└─────────────────────────────────────────────────────────────────────┘
```

### IBC v2 packet flow

```
1. registerCounterparty(clientId, merklePrefix)        ← one-time per chain pair
        │  router stores (clientId → counterparty)
        ▼
2. sendPacket(Packet{sequence, sourceClient, destClient, payloads[]})
        │  Hermes observes SendPacket, fetches proof from source chain
        ▼
3. recvPacket(packet, proof, proofHeight)              ← LC verify_membership
        │  Hermes observes WriteAcknowledgement, fetches proof from dest
        ▼
4. ackPacket(packet, ack, proof, proofHeight)          ← clears source commitment
```

### Light-client verification

The Cosmos counterparty tracks Stellar via the standard ibc-go `08-wasm` mechanism:

- `light-client-wasm` compiles to `wasm32-unknown-unknown`.
- Uploaded once to the Cosmos chain via `MsgStoreCode` (see
  [`ci/flows/upload-lc-wasm.sh`](ci/flows/upload-lc-wasm.sh)).
- Verifies SCP `EXTERNALIZE` envelopes (Ed25519 signatures from a quorum of trusted
  validators) and walks the gateway-produced `MerkleProof` against
  `ConsensusState.root` (the SMT root).

The wasm crate is at [`crates/light-client-wasm`](crates/light-client-wasm); the
SMT + ICS-23 helpers it consumes live in
[`crates/core/src/ibc/`](crates/core/src/ibc/).

---

## Repository Structure

```
stellar-ibc/
  crates/
    core/                 stellar-ibc-core — shared protocol + transport libs
      src/
        rpc.rs            RpcClient (Soroban JSON-RPC wrapper)
        api_client.rs     ApiClient — HTTP client the gateway uses to reach stellar-api
        ibc/              SMT, ICS-23 proofs, IBC context, action dispatch

    gateway/              stellar-hermes-gateway — gRPC service
      src/
        main.rs, config.rs, runner.rs
        query.rs          StellarGatewayQuery handlers (ApiClient-backed)
        msg.rs            StellarGatewayMsg handlers (ApiClient-backed)
        state_tracker.rs  SMT-backed state tracker; proof_for_path()
      proto/stellar_gateway.proto

    api/                  stellar-api — standalone HTTP service (axum)
      src/
        main.rs, config.rs, runner.rs, state.rs
        services/         account, balance, events, ledgers, tx/

    osmosis/              stellar-osmosis — local Osmosis lifecycle crate
      assets/
        default-config.json   Declarative chain config (denoms, gov, keys)
        setup.sh              Container entrypoint (jq + dasel data-driven)
      src/                main.rs, runner.rs, config.rs

    integration-tests/    cargo bin: gRPC + RPC smoke tests
    light-client-wasm/    Stellar LC compiled for 08-wasm

  contracts/              Soroban contracts (workspace members)
    router/               IBC v2 router (create_client, register_counterparty, …)
    transfer-app/         ICS-20 transfer module
    light-clients/
      mock/               Always-accept LC for development
      attestation/        Federated attestation LC (pending)
      tendermint/         Tendermint LC (pending)

  ci/
    Makefile              make -C ci <target>
    hermes-config.toml    Host hermes config (127.0.0.1 endpoints)
    hermes-config.docker.toml  In-compose hermes config (service-name endpoints)
    flows/                cosmos-only, build-{gateway,api,hermes}-image,
                          deploy-contracts, upload-lc-wasm, hermes-keys,
                          f0-bootstrap, caribic-preflight

  Dockerfile              Builds both stellar-gateway + stellar-api binaries
  docker-compose.yml      Profiles: local, hermes, local-stellar, staging
  Makefile                Top-level dev targets
  .env / .env.example
```

---

## gRPC API

Services defined in
[`crates/gateway/proto/stellar_gateway.proto`](crates/gateway/proto/stellar_gateway.proto)
(package `stellar.gateway.v1`).

The gateway itself holds **no** Soroban connection — every call is fulfilled
through `ApiClient` against `stellar-api`.

### `StellarGatewayQuery`

| Method | Inputs | Outputs | Notes |
|---|---|---|---|
| `LatestHeight` | — | `revision_number`, `revision_height` | Calls `GET /ledger/latest` on the api |
| `QueryIbcHeader` | `height` | `header` (bytes) | Serialised `StellarHeader` w/ SMT root + SCP envelope; backed by `GET /ledger/{seq}` |
| `QueryPacketCommitment` | `client_id`, `sequence`, `height` | `commitment`, `proof`, `proof_height` | v2 path `{clientId} \|\| 0x01 \|\| be64(seq)` |
| `QueryPacketReceipt` | `client_id`, `sequence`, `height` | `received` (bool), `proof`, `proof_height` | v2 path `… 0x02 …` |
| `QueryAcknowledgement` | `client_id`, `sequence`, `height` | `acknowledgement`, `proof`, `proof_height` | v2 path `… 0x03 …` |
| `Events` | `contract_id`, `cursor`, `start_ledger`, `limit` | `events[]`, `latest_ledger`, `cursor` | Backed by api `GET /events` (planned) |
| `QueryClientState` / `QueryConsensusState` / `QueryNextSeqRecv` | — | — | Return `Unimplemented` — non-provable in v2 |

### `StellarGatewayMsg`

| Method | Inputs | Outputs | Notes |
|---|---|---|---|
| `SubmitSignedTx` | `tx_xdr` | `tx_hash`, `events[]` | `POST /tx/submit` on the api |
| `CreateClient` / `UpdateClient` / `RegisterCounterparty` / `RecvPacket` / `AckPacket` / `TimeoutPacket` / `SubmitMisbehaviour` | (ICS-2 / ICS-4) | (ICS-2 / ICS-4) | Encode `ScVal` args → `POST /contract/invoke` on the api, which builds, simulates, signs, and submits to Soroban |

gRPC reflection is on:

```sh
grpcurl -plaintext localhost:50052 list
grpcurl -plaintext localhost:50052 stellar.gateway.v1.StellarGatewayQuery/LatestHeight
```

---

## HTTP API

Served by **`stellar-api`** on `:8101` (not the gateway). Used both by the gateway
(via `ApiClient`) and exposable to external clients.

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | Liveness probe; returns `"Stellar IBC API is healthy."` |
| `GET` | `/ledger/latest` | Latest Stellar ledger sequence, fetched via Soroban RPC |
| `GET` | `/ledger/{sequence}` | Ledger header + close-metadata XDR (hex) at the given sequence |
| `GET` | `/account/{address}` | Soroban account info for a Stellar address |
| `GET` | `/balance/{address}` | Balance for a Stellar address |
| `GET` | `/tx/xdr` | Build an unsigned transaction envelope |
| `GET` | `/tx/{tx_hash}` | Fetch a submitted transaction by hash |
| `POST` | `/tx/sign` | Sign an unsigned envelope using `STELLAR_SIGNING_KEY` |
| `POST` | `/tx/submit` | Submit a signed envelope to Soroban; waits for inclusion |

---

## Configuration

All configuration is via environment variables. Copy `.env.example` to `.env`.

### Stellar Gateway (`gateway` service / `stellar-hermes-gateway` binary)

| Variable | Default | Description |
|---|---|---|
| `STELLAR_GATEWAY_HOST` | `0.0.0.0` | gRPC bind address |
| `STELLAR_GATEWAY_GRPC_PORT` | `50052` | gRPC listen port |
| `STELLAR_API_URL` | `http://127.0.0.1:8101` | Where the gateway reaches the api service. In docker: `http://api:8101` |
| `IBC_CONTRACT_ID` | _(empty)_ | Soroban contract address of the IBC router |

### Stellar API (`api` service / `stellar-api` binary)

| Variable | Default | Description |
|---|---|---|
| `STELLAR_API_HOST` | `0.0.0.0` | HTTP bind address |
| `STELLAR_API_PORT` | `8101` | HTTP listen port |
| `STELLAR_RPC_URL` | `https://soroban-testnet.stellar.org` | Soroban JSON-RPC endpoint |
| `NETWORK_PASSPHRASE` | `Test SDF Network ; September 2015` | Stellar network identifier |
| `STELLAR_SIGNING_KEY` | _(required for tx submission)_ | Ed25519 secret (strkey `S…`) |
| `IBC_CONTRACT_ID` | _(empty)_ | Router contract — used by `/contract/invoke` |
| `TRANSFER_CONTRACT_ID` | _(empty)_ | Transfer app contract |

### Local Osmosis (`osmosis` service / `stellar-osmosis` binary)

| Variable | Default | Description |
|---|---|---|
| `OSMOSIS_VERSION` | `31.0.3` | `osmolabs/osmosis` image tag (`-alpine` variant used) |
| `OSMOSIS_LOCAL_GENESIS_TIME` | _(now)_ | Override genesis_time; defaults to current UTC at boot |
| `COSMOS_CHAIN_ID` | `localosmosis` | Chain id |

Genesis denoms, accounts, mnemonics, gov voting period, and overrides live in
[`crates/osmosis/assets/default-config.json`](crates/osmosis/assets/default-config.json).

### Image build / CI

| Variable | Default | Description |
|---|---|---|
| `GATEWAY_IMAGE` / `GATEWAY_TAG` | `amandagonsalvesx/stellar-gateway:latest` | gateway image ref |
| `API_IMAGE` / `API_TAG` | `amandagonsalvesx/stellar-ibc-api:latest` | api image ref |
| `HERMES_IMAGE` / `HERMES_TAG` | `amandagonsalvesx/stellar-hermes-cardano:latest` | hermes-relayer fork image |
| `HERMES_REPO` | `../hermes-relayer` | Path to the hermes-relayer fork checkout |
| `DOCKER_USERNAME` / `DOCKER_TOKEN` | _(unset)_ | DockerHub creds for `make push-*` |

### Network passphrases

| Network | Passphrase |
|---|---|
| Testnet | `Test SDF Network ; September 2015` |
| Mainnet | `Public Global Stellar Network ; September 2015` |
| Local (quickstart `--local`) | `Standalone Network ; February 2017` |

---

## Running

### Prerequisites

- Docker + Docker Compose
- Rust ≥ 1.81 (for local crate builds outside Docker)
- `stellar-cli` (for Soroban contract builds + deploys) — `cargo install --locked stellar-cli`
- `hermes` ≥ 1.13 (for `make -C ci upload-lc-wasm` from the host) — see
  [`ci/README.md`](ci/README.md#prerequisites)
- `grpcurl`, `jq` (probes)

### One-command devnet

Brings up `osmosis`, `api`, `gateway`, `hermes` with healthchecks and dependency
ordering. Detached so you can inspect status; logs are followed separately.

```sh
cp .env.example .env
# edit .env: STELLAR_SIGNING_KEY (and IBC_CONTRACT_ID if already deployed)

make start-stellar-ibc          # docker compose --profile local --profile hermes up -d --build
make logs-stellar-ibc           # tails api + gateway + hermes (skips osmosis log spam)
```

On first run the gateway/api image compiles the Rust workspace (~5–10 min);
subsequent starts are seconds.

To include a fully-local Stellar node instead of testnet:

```sh
docker compose --profile local --profile local-stellar --profile hermes up -d --build
# then point STELLAR_RPC_URL at http://node:8000/rpc in .env
```

### Per-service helpers

```sh
make restart-api     # docker compose rm -sf + up -d (recreates, picks up compose changes)
make restart-gateway
make restart-hermes
make logs-api
make logs-gateway
make logs-hermes
make shell-hermes    # interactive shell inside the hermes container
make ps-stellar-ibc
make stop-stellar-ibc
```

### Local Osmosis on its own

```sh
make start-osmosis           # fresh state (wipes ~/.osmosisd-local)
make start-osmosis-stateful  # keep existing state
make health-osmosis
make stop-osmosis
```

### Bootstrap the bridge (contracts + lc-wasm)

After the stack is up:

```sh
make -C ci deploy-contracts   # build + deploy router/transfer/mock-LC on Stellar testnet,
                              # writes contract IDs into .env
make restart-gateway          # gateway reads the new IBC_CONTRACT_ID

make -C ci hermes-keys        # import testkey + stellar-relayer into the hermes-keys volume
make -C ci upload-lc-wasm     # gov-upload Stellar LC wasm to localosmosis, patch hermes config

# Or run the orchestrator that does all of the above:
make -C ci f0
```

### Image push (DockerHub)

```sh
make push-gateway              # docker build + smoke-test + push amandagonsalvesx/stellar-gateway:latest
make push-api                  # … amandagonsalvesx/stellar-ibc-api:latest
make push-hermes               # … amandagonsalvesx/stellar-hermes-cardano:latest (from your fork)

PUSH=0 make push-gateway       # build + smoke-test only, no push
```

### Probe

```sh
curl -s http://127.0.0.1:8101/health
curl -s http://127.0.0.1:8101/ledger/latest | jq .
grpcurl -plaintext 127.0.0.1:50052 stellar.gateway.v1.StellarGatewayQuery/LatestHeight
curl -s http://127.0.0.1:26658/status | jq .result.sync_info     # Osmosis
make logs-hermes
```

---

## Manual testing

The integration-test binary runs gRPC + RPC smoke tests against a running
gateway:

```sh
# In one terminal:
cargo run -p stellar-hermes-gateway
# In another:
cargo run -p stellar-integration-tests
```

`STELLAR_GATEWAY_GRPC_ADDR` (default `http://0.0.0.0:50052`) controls where it
points.

---

## Development

### Build the workspace

```sh
cargo check --workspace
cargo build --release
```

Notable build pieces:

- **`crates/gateway/build.rs`** compiles `proto/stellar_gateway.proto` with
  `tonic-build` (manual mode) at compile time.
- **`crates/integration-tests/build.rs`** does the same, additionally exporting
  `PROTOS_OUT_DIR` so `src/pb.rs` can `include!` the generated client stubs.

### Build Soroban contracts

```sh
make build-contracts          # stellar contract build --profile contract
```

Output `.wasm` files land under `target/wasm32v1-none/release/`.

### Lint and test

```sh
make check                    # fmt-check + clippy + cargo test
make fmt                      # auto-format
make lint                     # clippy only
make test                     # cargo test --locked
make audit                    # cargo audit

make check-api                # per-crate
make check-gateway
make check-ibc-core
make check-contracts
```

Run specific tests:

```sh
cargo test -p stellar-ibc-core         # SMT + proof serializer
cargo test -p stellar-hermes-gateway   # query/msg validation
cargo test -p mock-light-client
```

### CI flows

`ci/flows/` contains one script per bootstrap step (each idempotent, each a
Make target under `make -C ci <target>`). See
[`ci/flows/README.md`](ci/flows/README.md) for per-script env vars + common
failures, and [`ci/README.md`](ci/README.md) for the higher-level T-/D-/lc-wasm
test suites.

---
