<p align="center">
  <img src="docs/assets/thumbnail.png" alt="Stellar IBC Bridge" width="100%" />
</p>

# Stellar IBC Bridge

Rust implementation of **IBC v2 (Eureka)** for the Stellar network, enabling trustless
cross-chain communication between Stellar and Cosmos-compatible chains.

This repository is part of the **Cardano–Stellar IBC bridge** project. It ships:

- **`stellar-hermes-gateway`** — gRPC + HTTP gateway the Hermes relayer talks to.
- **`stellar-ibc-core`** — shared library: RPC client, SMT, ICS-23 proof serializer,
  and the IBC protocol context used by both the gateway and the Soroban contracts.
- **`mock-light-client`** + **`stellar-ibc`** (Soroban contracts) — v2 ICS-2 client
  interface and the ICS-2/ICS-4 router.
- **Hermes fork integration** — `relayer-types::clients::ics10_stellar` and
  `chain::stellar::StellarChainEndpoint` live in the
  [cardano-foundation/hermes-relayer](https://github.com/cardano-foundation/hermes-relayer)
  fork.

Related repositories:

| Repo | Role |
|---|---|
| [stellar-ibc](https://github.com/amandagonsalves/stellar-ibc) | This repo |
| [hermes-relayer](https://github.com/cardano-foundation/hermes-relayer) (fork) | Relayer with `StellarChainEndpoint` and `ics10_stellar` types |
| [cardano-ibc-incubator](https://github.com/cardano-foundation/cardano-ibc-incubator) | Cosmos entrypoint chain, `proto-types`, `caribic` CLI |

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Repository Structure](#repository-structure)
- [gRPC API](#grpc-api)
- [HTTP API](#http-api)
- [Configuration](#configuration)
- [Running](#running)
- [Local CometBFT testing](#local-cometbft-testing)
- [Integration with caribic](#integration-with-caribic)
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

See [`docs/stellar-ibc/08-ics23-proof-generation-for-ibcv2.md`](docs/stellar-ibc/08-ics23-proof-generation-for-ibcv2.md)
for the SMT design and ICS-23 wire format.

---

## Architecture

```
┌───────────────────────────────────────────────────────────────────────┐
│                  Hermes relayer fork (Rust)                           │
│  cardano-foundation/hermes-relayer                                    │
│  crates/relayer/src/chain/stellar/                                    │
│    StellarChainEndpoint  ──  build_header, query_*, send_messages*    │
│  crates/relayer-types/src/clients/ics10_stellar/                      │
│    ClientState · ConsensusState · Header · Misbehaviour               │
└──────────────────────────────┬────────────────────────────────────────┘
                               │ gRPC (port 50052)
                               ▼
┌───────────────────────────────────────────────────────────────────────┐
│                stellar-hermes-gateway (this repo)                     │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  StellarGatewayQuery   (tonic)                                  │  │
│  │   LatestHeight  ·  QueryIbcHeader                               │  │
│  │   QueryPacketCommitment / Receipt / Acknowledgement (+proofs)   │  │
│  │   QueryClientState / ConsensusState (non-provable in v2)        │  │
│  │                                                                 │  │
│  │  StellarGatewayMsg     (tonic)  [stubs, Task 7 follow-on]       │  │
│  │   CreateClient · RegisterCounterparty · UpdateClient            │  │
│  │   SubmitSignedTx · RecvPacket · AckPacket · TimeoutPacket       │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  StateTracker  →  Smt (stellar-ibc-core::smt::Smt)              │  │
│  │   processes LedgerCloseMeta diffs, maintains the SMT root       │  │
│  │   generates ICS-23 MerkleProof bytes for provable paths         │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  HTTP server  (Axum)  :8005                                     │  │
│  │   /health  ·  /account/{addr}  ·  /balance/{addr}               │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────┬────────────────────────────────────────┘
                               │ JSON-RPC (HTTPS)
                               ▼
┌───────────────────────────────────────────────────────────────────────┐
│   Stellar Soroban RPC                                                 │
│   https://soroban-testnet.stellar.org  (or local stellar/quickstart)  │
└──────────────────────────────┬────────────────────────────────────────┘
                               │ host-function invocations
                               ▼
┌───────────────────────────────────────────────────────────────────────┐
│   Soroban contracts (this repo, contracts/)                           │
│                                                                       │
│   stellar-ibc          ICS-2 router + (pending) ICS-4 packet handler  │
│                        create_client · register_counterparty          │
│                        update_client · register_client_type           │
│                                                                       │
│   mock-light-client    ICS-2 light-client interface (always-accept    │
│                        stub used to develop the router)               │
│                                                                       │
│   stellar-tendermint   pending — verifies Cosmos headers on Stellar   │
│   stellar-lc-wasm      pending — packaged separately as a WASM blob   │
│                        and loaded into Cosmos via 08-wasm             │
└───────────────────────────────────────────────────────────────────────┘
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

- A Rust crate (`stellar-lc-wasm`, pending) compiles to `wasm32-unknown-unknown`.
- Uploaded once to the Cosmos chain via `MsgStoreCode`, instantiated per client.
- Verifies SCP `EXTERNALIZE` envelopes (Ed25519 signatures from a quorum of trusted
  validators) and walks the gateway-produced `MerkleProof` against
  `ConsensusState.root` (the SMT root).

See [`docs/stellar-ibc/07-proof-availability-and-smt.md`](docs/stellar-ibc/07-proof-availability-and-smt.md)
and [`docs/stellar-ibc/08-ics23-proof-generation-for-ibcv2.md`](docs/stellar-ibc/08-ics23-proof-generation-for-ibcv2.md)
for the full design.

---

## Repository Structure

```
stellar-ibc/
  crates/
    core/                         ← stellar-ibc-core: shared protocol + RPC library
      src/
        lib.rs                    ← re-exports submodules at crate root + ::ibc alias
        rpc.rs                    ← RpcClient (soroban-client wrapper): get_ledger_entry,
                                    get_ledger, submit_and_wait, latest_ledger_sequence
        ibc/
          mod.rs                  ← re-exports submodules + ::ibc as stellar_ibc
          smt.rs                  ← fixed-depth-64 binary Merkle tree (Cardano-compatible)
          proof.rs                ← ICS-23 MerkleProof serializer (membership + absence)
          actions.rs              ← top-level IBC action dispatch
          context/                ← StellarIbcContext<S> — generic over storage backend
          msg.rs, event.rs, error.rs, storage.rs, trace.rs

    gateway/                      ← stellar-hermes-gateway binary
      src/
        main.rs                   ← entry point
        config.rs                 ← GatewayConfig (env vars)
        runner.rs                 ← spins up Axum + Tonic concurrently
        query.rs                  ← StellarGatewayQuery handlers (v2 path proofs wired)
        msg.rs                    ← StellarGatewayMsg handlers (stubs)
        state_tracker.rs          ← SMT-backed state tracker; proof_for_path()
        rpc.rs, state.rs, proto.rs
        api/account.rs            ← HTTP /account, /balance
      proto/stellar_gateway.proto
      build.rs                    ← tonic-build (manual mode)

    integration-tests/            ← cargo bin: testnet RPC + gateway gRPC smoke tests
      src/
        main.rs                   ← live testnet checks
        gateway_tests.rs          ← gRPC checks against a running gateway
        pb.rs                     ← include!()s the tonic client stubs
      build.rs                    ← prost-build + tonic-build manual builders;
                                    exports PROTOS_OUT_DIR for pb.rs

  contracts/                      ← Soroban smart contracts (workspace members)
    mock-light-client/            ← ICS-2 always-accept stub (4 unit tests)
    stellar-ibc/                  ← ICS-2 router: create_client, register_counterparty,
                                    update_client, register_client_type (6 unit tests)

  ci/                             ← local integration scripts (WASM upload, health)

  Dockerfile
  docker-compose.yml              ← default = testnet, --profile local = local node
  .env.example
  Makefile
```

---

## gRPC API

Services defined in
[`crates/gateway/proto/stellar_gateway.proto`](crates/gateway/proto/stellar_gateway.proto)
(package `stellar.gateway.v1`).

Copy-paste `grpcurl` recipes for every endpoint:
[`docs/stellar-ibc/09-grpc-commands.md`](docs/stellar-ibc/09-grpc-commands.md).

### `StellarGatewayQuery`

| Method | Inputs | Outputs | Notes |
|---|---|---|---|
| `LatestHeight` | — | `revision_number`, `revision_height` | Latest Stellar ledger sequence |
| `QueryIbcHeader` | `height` | `header` (bytes) | Serialised `StellarHeader` w/ SMT root + SCP envelope |
| `QueryPacketCommitment` | `client_id`, `sequence`, `height` | `commitment`, `proof`, `proof_height` | v2 path `{clientId} \|\| 0x01 \|\| be64(seq)`; membership or SENTINEL non-membership |
| `QueryPacketReceipt` | `client_id`, `sequence`, `height` | `received` (bool), `proof`, `proof_height` | v2 path `{clientId} \|\| 0x02 \|\| be64(seq)` |
| `QueryAcknowledgement` | `client_id`, `sequence`, `height` | `acknowledgement`, `proof`, `proof_height` | v2 path `{clientId} \|\| 0x03 \|\| be64(seq)` |
| `QueryClientState` | `client_id`, `height` | `client_state`, `proof`, `proof_height` | **Returns `Unimplemented`** — non-provable in v2 |
| `QueryConsensusState` | `client_id`, `revision_number`, `revision_height` | `consensus_state`, `proof`, `proof_height` | **Returns `Unimplemented`** — non-provable in v2 |
| `QueryNextSeqRecv` | `client_id` | `next_seq_recv`, `proof`, `proof_height` | **Returns `Unimplemented`** — path removed in IBC v2 |

### `StellarGatewayMsg`

| Method | Inputs | Outputs | Notes |
|---|---|---|---|
| `SubmitSignedTx` | `tx_xdr` | `tx_hash`, `events[]` | Submit a pre-signed Soroban TX |
| `CreateClient` | `client_state`, `consensus_state`, `signer` | `client_id` | ICS-2 §CreateClient |
| `UpdateClient` | `client_id`, `header`, `signer` | — | ICS-2 §Update |
| `RegisterCounterparty` | `client_id`, `counterparty_client_id`, `merkle_prefix` | — | ICS-2 §RegisterCounterparty (replaces v1 connection + channel handshakes) |
| `RecvPacket` | `packet`, `proof`, `proof_height`, `signer` | — | ICS-4 §RecvPacket |
| `AckPacket` | `packet`, `acknowledgement`, `proof`, `proof_height`, `signer` | — | ICS-4 §AcknowledgePacket |
| `TimeoutPacket` | `packet`, `proof`, `proof_height`, `signer` | — | ICS-4 §TimeoutPacket |


gRPC reflection is on:

```bash
grpcurl -plaintext localhost:50052 list
grpcurl -plaintext localhost:50052 stellar.gateway.v1.StellarGatewayQuery/LatestHeight
```

---

## HTTP API

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | `"Server is up."` — used by caribic health checks |
| `GET` | `/account/{address}` | Soroban account info for a Stellar address |
| `GET` | `/balance/{address}` | XLM balance for a Stellar address |

---

## Configuration

All configuration is via environment variables. Copy `.env.example` to `.env`.

| Variable | Default | Description |
|---|---|---|
| `STELLAR_GATEWAY_HOST` | `0.0.0.0` | Bind address for both servers |
| `STELLAR_GATEWAY_GRPC_PORT` | `50052` | gRPC listen port |
| `STELLAR_GATEWAY_HTTP_PORT` | `8005` | HTTP listen port |
| `STELLAR_RPC_URL` | `https://soroban-testnet.stellar.org` | Soroban JSON-RPC endpoint |
| `NETWORK_PASSPHRASE` | `Test SDF Network ; September 2015` | Stellar network identifier |
| `STELLAR_SIGNING_KEY` | _(required for tx submission)_ | Ed25519 secret (strkey `S…`) |
| `IBC_CONTRACT_ID` | _(empty)_ | Soroban contract address of the IBC router (`contracts/stellar-ibc`) |
| `TRANSFER_CONTRACT_ID` | _(empty)_ | Soroban contract address of the ICS-20 transfer app (future) |
| `STELLAR_GATEWAY_GRPC_ADDR` | `http://0.0.0.0:50052` | Read by `integration-tests` to locate the gateway |

Network passphrases:

| Network | Passphrase |
|---|---|
| Testnet | `Test SDF Network ; September 2015` |
| Mainnet | `Public Global Stellar Network ; September 2015` |
| Local (quickstart `--local`) | `Standalone Network ; February 2017` |

---

## Running

### Prerequisites

- Rust ≥ 1.81 (workspace pin)
- `protobuf-compiler` — `brew install protobuf` / `apt-get install protobuf-compiler`
- Docker + Docker Compose (only for container-based runs)
- `stellar-cli` (only for building Soroban contracts) — `cargo install --locked stellar-cli`

### Local binary (testnet)

```bash
cargo build --release -p stellar-hermes-gateway

cp .env.example .env
# edit .env: set STELLAR_SIGNING_KEY

./target/release/stellar-gateway
```

### Docker — testnet (default)

```bash
cp .env.example .env
# edit .env: set STELLAR_SIGNING_KEY

docker compose up
```

gRPC on `:50052`, HTTP on `:8005`, RPC pointed at `soroban-testnet.stellar.org`.

### Docker — local Stellar node

```bash
# .env additions
STELLAR_RPC_URL=http://stellar-node:8000/soroban/rpc
NETWORK_PASSPHRASE=Standalone Network ; February 2017

docker compose --profile local up
```

The gateway waits for the local node to be healthy before starting.

### Verify

```bash
curl http://localhost:8005/health
grpcurl -plaintext localhost:50052 stellar.gateway.v1.StellarGatewayQuery/LatestHeight
```

---

## Local CometBFT testing

For end-to-end testing without spinning up the full Cardano entrypoint chain, follow
[`docs/stellar-ibc/10-local-cometbft-testing.md`](docs/stellar-ibc/10-local-cometbft-testing.md).
It covers:

- Why `cometbft node --proxy_app=kvstore` alone isn't enough (no IBC modules).
- The three realistic Cosmos counterparty options:
  1. **`ibc-go simd`** (recommended for v2 + `08-wasm`).
  2. **`basecoin-rs`** (Rust reference for the ABCI split pattern — v1 only).
  3. **`cardano-entrypoint`** (heaviest, but already wired in this monorepo).
- A testability matrix for what works today vs. what's blocked on which task.
- Four numbered flows from "gateway-only smoke" up to "full 08-wasm against simd".

---

## Integration with caribic

The `caribic` CLI in
[cardano-ibc-incubator/caribic](https://github.com/cardano-foundation/cardano-ibc-incubator/tree/main/caribic)
manages the gateway lifecycle:

```bash
caribic chain start  --chain stellar     # initialise, build if needed, start
caribic chain health --chain stellar     # TCP probes on gRPC :50052, HTTP :8005, RPC :443
caribic chain stop   --chain stellar     # SIGTERM
```

When the Hermes fork binary exists at `relayer/target/release/hermes`, caribic also writes
the Stellar chain block to `~/.hermes/config.toml` so the fork's `hermes health-check`
covers `stellar-testnet`. The upstream `hermes` binary is never given the Stellar block —
it does not understand `type = 'Stellar'`.

Relayer key resolution order:

1. `STELLAR_SECRET_KEY` env var (preferred — never written to disk).
2. `caribic/config/stellar-testnet-key.txt` file fallback.

---

## Development

### Build the workspace

```bash
cargo check --workspace
cargo build --release
```

Notable build pieces:

- **`crates/gateway/build.rs`** compiles `proto/stellar_gateway.proto` with
  `tonic-build` (manual mode) at compile time.
- **`crates/integration-tests/build.rs`** does the same, additionally exporting
  `PROTOS_OUT_DIR` so `src/pb.rs` can `include!` the generated client stubs.

### Build Soroban contracts

```bash
cd contracts/mock-light-client && stellar contract build
cd contracts/stellar-ibc       && stellar contract build
```

Output `.wasm` files land under `target/wasm32v1-none/release/`.

### Lint and test

```bash
make check      # fmt-check + clippy + cargo test
make fmt        # auto-format
make lint       # clippy only
make test       # cargo test --locked
make audit      # cargo audit
```

Run specific crates:

```bash
cargo test -p stellar-ibc-core         # SMT + proof serializer tests
cargo test -p mock-light-client        # always-accept LC tests
cargo test -p stellar-ibc              # router tests (contract crate)
```

### Running the gateway integration tests

The `integration-tests` binary points at a running gateway via
`STELLAR_GATEWAY_GRPC_ADDR` (default `http://0.0.0.0:50052`). It does **not** spawn or
manage the server:

```bash
# In one terminal:
cargo run --release -p stellar-hermes-gateway

# In another:
cargo run -p stellar-integration-tests
```

The test binary prints PASS/FAIL for each Soroban RPC sanity check and every
gateway gRPC endpoint.

### CI integration tests

Scripts in `ci/` exercise WASM upload and Hermes connectivity against a live Cosmos
chain. They skip automatically when the chain is not reachable.

```bash
# Prerequisites: Hermes binary on PATH, wasm32 target, Cosmos entrypoint running
bash ci/entrypoint.sh
```

See [`ci/README.md`](ci/README.md) for full setup.

---
