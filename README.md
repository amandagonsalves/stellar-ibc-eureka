<p align="center">
  <img src="docs/assets/thumbnail.png" alt="Stellar IBC Bridge" width="100%" />
</p>

# Stellar IBC Bridge

Rust implementation of **IBC v2 (Eureka)** for the Stellar network, enabling trustless
cross-chain communication between Stellar and Cosmos-compatible chains.

This repository is part of the **Cardano–Stellar IBC bridge** project. It ships:

| Component | Role |
|---|---|
| **`stellar-hermes-gateway`** | gRPC gateway the Hermes relayer talks to. Speaks no Soroban RPC directly; every chain read/write goes through `stellar-api`. |
| **`stellar-api`** | Standalone HTTP/REST service that owns the Soroban RPC connection and Stellar signing key. Exposes `/ledger/*`, `/account/*`, `/balance/*`, `/tx/*` for the gateway to call. |
| **`stellar-ibc-core`** | Shared library: SMT, ICS-23 proof serializer, IBC protocol context, plus the `RpcClient` (Soroban JSON-RPC) and `ApiClient` (HTTP client the gateway uses to reach `stellar-api`). |
| **Local Cosmos** (`cli/src/cosmos`) | Local Cosmos chain (`simd-1`) lifecycle, driven by the `stellaribc cosmos` commands. Boots the prebuilt `ghcr.io/cosmos/ibc-go-wasm-simd:v11.0.0` image (ibc-go v11 + `08-wasm`) from a declarative `default-config.json`. Acts as the Cosmos counterparty for local devnets. |
| **`light-client-wasm`** | Stellar light client compiled to `wasm32-unknown-unknown`, uploaded to the Cosmos chain via `08-wasm` (`contracts/cosmwasm/light-client`). |
| **Soroban contracts** | `ibc-router`, `ibc-transfer`, and light clients (`mock`, `attestation`, `tendermint`) under `contracts/soroban/`. |
| **`stellar-ibc-cli`** (`stellaribc`) | The orchestrator CLI under `cli/`. One binary for the whole bridge: bring the stack up, pull + run images, deploy contracts, upload the light client, create clients, register counterparties, and check status. Drives docker, the `stellar` CLI, and `stellar-api` directly — no shell scripts. |
| **Hermes fork integration** | `relayer-types::clients::ics10_stellar` and `chain::stellar::StellarChainEndpoint` live in the [cardano-foundation/hermes-relayer](https://github.com/cardano-foundation/hermes-relayer) fork. |

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
- [CLI (`stellaribc`)](#cli-stellaribc)
- [Running](#running)
- [Manual testing](#manual-testing)
- [Development](#development)

---

## Overview

**IBC version:** v2 (Eureka) — no connection or channel handshake.
**On-chain runtime:** Soroban (Stellar's WebAssembly smart-contract platform).
**Relayer:** Hermes fork with a `StellarChainEndpoint`.
**Counterparty:** Cosmos chain (ibc-go v11 `simd`) hosting an `08-wasm` Stellar light client.

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
│     /tx/prepare · /tx/submit                │    │
│     /stellar/clients · /events              │    │
│   owns Soroban RpcClient + signing key      │    │
└──────────────────┬──────────────────────────┘    │
                   │ JSON-RPC                       │
                   ▼                                ▼
┌─────────────────────────────────┐  ┌──────────────────────────────────┐
│  Stellar Soroban RPC            │  │  cosmos (simd-1)                 │
│  soroban-testnet.stellar.org    │  │  ibc-go-wasm-simd:v11.0.0        │
│  or in-compose stellar-node     │  │  ibc-go v11 + 08-wasm            │
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
- Uploaded once to the Cosmos chain via `MsgStoreCode` (`stellaribc contracts upload-wasm`).
- Verifies SCP `EXTERNALIZE` envelopes (Ed25519 signatures from a quorum of trusted
  validators) and walks the gateway-produced `MerkleProof` against
  `ConsensusState.root` (the SMT root).

The wasm crate is at
[`contracts/cosmwasm/light-client`](contracts/cosmwasm/light-client); the
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

  cli/                    stellar-ibc-cli — the `stellaribc` orchestrator
    src/
      main.rs             clap command tree + dispatch
      config.rs repo.rs run.rs probe.rs logger.rs shared.rs   base config + support
      ops/                install · check · status · stack(up/down) · start
      cosmos/             local Cosmos (simd-1) lifecycle + config (start/stop/status/keygen)
        assets/           default-config.json + setup.sh (mounted into the cosmos container)
      stellar/            stellar chain config + lifecycle
      clients/            cosmos · stellar · counterparty · list (+ config)
      hermes/  gateway/  api/   start · stop · restart (pull-and-run; each owns its config)
      contracts/          build · upload · deploy · invoke · deploy-all · wasm (+ config)
      tx/                 clients · msg · query  (low-level surface)

  contracts/
    soroban/              Soroban contracts (own nested workspace, `--profile contract`)
      ibc-router/         stellar-ibc-router — IBC v2 router (create_client, register_counterparty, …)
      ibc-transfer/       stellar-ibc-transfer — ICS-20 transfer module
      light-clients/
        mock/             stellar-mock-light-client — always-accept LC for development
        attestation/      stellar-attestation-light-client — federated attestation LC (pending)
        tendermint/       stellar-tendermint-light-client — Tendermint LC (pending)
    cosmwasm/
      light-client/       light-client-wasm — Stellar LC compiled for Cosmos 08-wasm

  hermes-config.toml      Hermes relayer config (mounted into the hermes + api containers)
  Dockerfile              Builds the stellar-gateway + stellar-api binaries
  docker-compose.yml      Profiles: local, cosmos, hermes, local-stellar, staging
  Makefile                Image build/push (SERVICE=gateway|hermes|api) + fmt/test/cargo-build
  .env / .env.example
```

> The bootstrap/flow shell scripts (previously under `ci/`) have been fully
> migrated into the `stellaribc` CLI; the `ci/` directory no longer exists.

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
| `SubmitSignedTx` | `tx_xdr` | `tx_hash`, `return_value` | `POST /tx/submit` on the api — submits a relayer-signed tx |
| `CreateClient` / `UpdateClient` / `RegisterCounterparty` / `RecvPacket` / `AckPacket` / `TimeoutPacket` / `SubmitMisbehaviour` | (ICS-2 / ICS-4) | unsigned `tx_xdr` | Gateway re-encodes args to Soroban XDR and builds an **unsigned** tx via `POST /tx/prepare`; the relayer signs with its key and submits via `SubmitSignedTx`. The gateway holds no key. (All message types are wired end-to-end through this prepare→sign→submit path.) |

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
| `GET` | `/health` | Liveness probe |
| `GET` | `/ledger/latest` · `/ledger/{sequence}` | Latest ledger sequence / ledger header + close-meta |
| `GET` | `/events` | Soroban contract events (paginated) |
| `GET` | `/account/{address}` · `/balance/{address}` | Account info / balance for a Stellar address |
| `GET` | `/stellar/clients` | Clients created on the router (by `client_type`) |
| `POST` | `/tx/prepare` | Build an **unsigned** Soroban tx (source = api signing key) |
| `POST` | `/tx/submit` | Submit a relayer-signed tx; waits for inclusion; returns hash + return value |
| `GET` | `/cosmos/node-info` · `/cosmos/proposer` · `/cosmos/funder` | Cosmos chain + signer info |
| `POST` | `/cosmos/bank/send` · `/cosmos/gov/vote` | Signed Cosmos txs (proposer/funder keys) |
| `GET`/`POST` | `/cosmos/gov/proposals*` · `/cosmos/ibc-wasm/{checksums,store-code}` | Gov proposals + 08-wasm store-code |
| `POST` | `/hermes/wasm-checksum` | Patch `wasm_checksum_hex` in the bound hermes config |

Swagger UI is served at `/docs` (`/api-docs/openapi.json`).

---

## Configuration

All configuration is via environment variables. Copy `.env.example` to `.env`.

### Stellar Gateway (`gateway` service / `stellar-hermes-gateway` binary)

| Variable | Default | Description |
|---|---|---|
| `STELLAR_GATEWAY_HOST` | `0.0.0.0` | gRPC bind address |
| `STELLAR_GATEWAY_GRPC_PORT` | `50052` | gRPC listen port |
| `STELLAR_API_URL` | `http://127.0.0.1:8101` | Where the gateway reaches the api service. In docker: `http://api:8101` |
| `ROUTER_CONTRACT_ADDRESS` | _(empty)_ | Soroban contract address of the IBC router |

### Stellar API (`api` service / `stellar-api` binary)

| Variable | Default | Description |
|---|---|---|
| `STELLAR_API_HOST` | `0.0.0.0` | HTTP bind address |
| `STELLAR_API_PORT` | `8101` | HTTP listen port |
| `STELLAR_RPC_URL` | `https://soroban-testnet.stellar.org` | Soroban JSON-RPC endpoint |
| `NETWORK_PASSPHRASE` | `Test SDF Network ; September 2015` | Stellar network identifier |
| `STELLAR_SIGNING_KEY` | _(required for tx submission)_ | Ed25519 secret (strkey `S…`) |
| `ROUTER_CONTRACT_ADDRESS` | _(empty)_ | Router contract — re-encoded + invoked via `/tx/prepare` |
| `TRANSFER_CONTRACT_ADDRESS` | _(empty)_ | Transfer app contract |

### Local Cosmos (`cosmos` compose service / `stellaribc cosmos`)

| Variable | Default | Description |
|---|---|---|
| `COSMOS_CHAIN_IMAGE` | `ghcr.io/cosmos/ibc-go-wasm-simd:v11.0.0` | simd image (ibc-go v11 + `08-wasm`, multi-arch) |
| `COSMOS_CHAIN_ID` | `simd-1` | Chain id |
| `COSMOS_VALIDATOR_MNEMONIC` | _(empty)_ | Validator account mnemonic, recovered + funded at genesis |
| `COSMOS_RELAYER_MNEMONIC` | _(empty)_ | Relayer account mnemonic, funded at genesis + imported into Hermes |

Genesis denoms, balances, gov voting period, and overrides live in
[`cli/src/cosmos/assets/default-config.json`](cli/src/cosmos/assets/default-config.json);
the account **mnemonics are sourced from env** (above), not the JSON, and are
passed into the `cosmos` container by `docker-compose.yml`.

### Image build / CI

| Variable | Default | Description |
|---|---|---|
| `GATEWAY_IMAGE` / `GATEWAY_TAG` | `amandagonsalvesx/stellar-gateway:latest` | gateway image ref |
| `API_IMAGE` / `API_TAG` | `amandagonsalvesx/stellar-ibc-api:latest` | api image ref |
| `HERMES_IMAGE` / `HERMES_TAG` | `amandagonsalvesx/stellar-hermes-cardano:latest` | hermes-relayer fork image |
| `HERMES_REPO` | `../hermes-relayer` | Path to the hermes-relayer fork checkout |
| `DOCKER_USERNAME` / `DOCKER_TOKEN` | _(unset)_ | DockerHub creds for `make push SERVICE=…` |

### Network passphrases

| Network | Passphrase |
|---|---|
| Testnet | `Test SDF Network ; September 2015` |
| Mainnet | `Public Global Stellar Network ; September 2015` |
| Local (quickstart `--local`) | `Standalone Network ; February 2017` |

---

## CLI (`stellaribc`)

`stellar-ibc-cli` (binary **`stellaribc`**) is the single entry point for bringing
the bridge up and driving the flows. It discovers the repo root (walking up for
`docker-compose.yml`, or `STELLAR_IBC_ROOT`), reads `.env`, drives docker / the
`stellar` CLI / `stellar-api` directly, and probes service health natively.

Install (any of):

```sh
cargo run -p stellar-ibc-cli -- install   # self-install to the cargo bin dir
cargo install --path cli
```

Command groups:

| Group | Commands |
|---|---|
| ops | `install` · `doctor` · `status` · `up [--cosmos\|--stellar]` · `down [--volumes]` · `start` |
| `cosmos` | `keygen` · `start [--fresh]` · `stop` · `status` |
| `clients` | `cosmos` · `stellar` · `counterparty <stellar\|cosmos>` · `list` |
| `transfer` | `<stellar\|cosmos>` — originate an ICS-20 transfer (`--denom --amount --receiver --memo --timeout-secs --no-mint`) |
| `hermes` | `start` · `stop` · `restart` · `keys-import` |
| `gateway` | `start` · `stop` · `restart` · `query` |
| `api` | `start` · `stop` · `restart` |
| `contracts` | `build` · `upload` · `deploy` · `invoke` · `deploy-all` · `upload-wasm` |
| `tx` | `clients` · `msg` · `query` (low-level; some pending) |

Full reference: [`cli/README.md`](cli/README.md). The `Makefile` is only for
image build/push (`make build`/`push SERVICE=<gateway|hermes|api>`) plus `make
fmt`/`test`/`cargo-build`; everything else runs through `stellaribc` directly.

---

## Running

### Prerequisites

- Docker + Docker Compose
- Rust ≥ 1.81 (for local crate builds outside Docker)
- `stellar-cli` (for Soroban contract builds + deploys) — `cargo install --locked stellar-cli`
- `binaryen` (`wasm-opt`, used by `contracts upload-wasm` to lower bulk-memory ops)
- `grpcurl` (gRPC probes)

### One-command devnet

Brings up `cosmos`, `api`, `gateway`, `hermes` with healthchecks and dependency
ordering. Detached so you can inspect status; logs are followed separately.

```sh
cp .env.example .env
# edit .env: STELLAR_SIGNING_KEY (and ROUTER_CONTRACT_ADDRESS if already deployed)

stellaribc up                   # cosmos + api + gateway via docker compose
docker compose --profile local --profile hermes logs -f api gateway hermes
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
stellaribc api restart       # or: gateway restart / hermes restart (--pull to refresh the image)
stellaribc down              # stop the stack

# logs / ps / shell run through docker compose directly:
COMPOSE="docker compose --profile local --profile hermes"
$COMPOSE logs -f api gateway hermes
$COMPOSE exec hermes sh      # interactive shell inside the hermes container
$COMPOSE ps
```

### Local Cosmos on its own

```sh
stellaribc cosmos keygen            # generate validator+relayer mnemonics + signer keys → .env
stellaribc cosmos start             # start the local simd-1 devnet
stellaribc cosmos start --fresh     # wipe the cosmos-home volume first, then start
stellaribc cosmos status
stellaribc cosmos stop
```

### Start the bridge (via `stellaribc`)

One command does everything — pull images, bring up the chains/services, deploy
the Soroban contracts, gov-upload the light-client wasm, and import the relayer
keys:

```sh
stellaribc start            # flags: --skip-images/-contracts/-wasm/-keys, --force-redeploy
```

Or step by step:

```sh
stellaribc up                   # docker compose up cosmos + api + gateway
stellaribc contracts deploy-all # deploy router/transfer/mock-LC, wire router, write .env
stellaribc gateway restart --pull      # pull latest + pick up the new ROUTER_CONTRACT_ADDRESS
stellaribc contracts upload-wasm       # gov-upload Stellar LC wasm, patch hermes config
stellaribc hermes keys-import          # import relayer keys into the hermes-keys volume
stellaribc status               # everything green?
```

Then create the clients:

```sh
stellaribc clients cosmos       # Cosmos (Tendermint) client on Stellar
stellaribc clients stellar      # Stellar (08-wasm) client on Cosmos
stellaribc clients list
```

### Images (build / push / pull)

Building and pushing images is done **only via the Makefile** — the CLI just
pulls and runs them:

```sh
make build SERVICE=api       # docker build amandagonsalvesx/stellar-ibc-api:latest
make push  SERVICE=gateway   # build + push amandagonsalvesx/stellar-gateway:latest
make push  SERVICE=hermes    # … from the hermes-relayer fork ($HERMES_REPO/ci/release/hermes.Dockerfile)
```

These read the image refs + `DOCKER_USERNAME`/`DOCKER_TOKEN` from `.env`. To
pull-and-run a published image: `stellaribc <service> start --pull` (or
`restart --pull`).

### Probe

```sh
curl -s http://127.0.0.1:8101/health
curl -s http://127.0.0.1:8101/ledger/latest | jq .
grpcurl -plaintext 127.0.0.1:50052 stellar.gateway.v1.StellarGatewayQuery/LatestHeight
curl -s http://127.0.0.1:26657/status | jq .result.sync_info     # Cosmos (simd-1)
docker compose --profile local --profile hermes logs -f hermes
```

---

## Manual testing

Bring the gateway up and smoke-test it directly — `stellaribc status` / `check`
run the same health probes, or hit the services with `curl` / `grpcurl`:

```sh
# In one terminal:
cargo run -p stellar-hermes-gateway      # or: stellaribc gateway start
# In another:
stellaribc status
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

### Build Soroban contracts

```sh
stellaribc contracts build    # stellar contract build --profile contract
```

Output `.wasm` files land under `target/wasm32v1-none/release/`.

### Lint and test

```sh
make fmt                      # cargo fmt --all
make test                     # cargo test --locked
make cargo-build              # cargo build

cargo clippy --locked --all-targets -- -D warnings   # lint
cargo audit --file Cargo.lock                         # audit
```

Run specific tests:

```sh
cargo test -p stellar-ibc-core         # SMT + proof serializer
cargo test -p stellar-hermes-gateway   # query/msg validation
cargo test -p mock-light-client
```

### Bootstrap & flows

All bootstrap/flow steps live in the `stellaribc` CLI — see
[CLI (`stellaribc`)](#cli-stellaribc) above and [`cli/README.md`](cli/README.md).

---
