# Soroban Contracts

The Stellar-side IBC v2 contracts, written for Soroban. This directory is its
**own Cargo workspace** (separate from the root workspace) so it can use the
Soroban `contract` build profile and `soroban-sdk`.

## Layout

| Path | Crate | Role |
|---|---|---|
| `ibc-router/` | `stellar-ibc-router` | IBC v2 router — `create_client`, `register_counterparty`, packet commitment/receipt/ack via the SMT |
| `ibc-transfer/` | `stellar-ibc-transfer` | ICS-20 fungible-token transfer application |
| `light-clients/mock/` | `stellar-mock-light-client` | always-accept light client for development |
| `light-clients/attestation/` | `stellar-attestation-light-client` | federated attestation light client (pending) |
| `light-clients/tendermint/` | `stellar-tendermint-light-client` | Tendermint light client (pending) |

Each contract crate is `crate-type = ["lib", "cdylib"]`, so it works both as a
Rust dependency (in cross-contract tests) and as a deployable wasm contract.

## Build

```sh
stellar contract build --profile contract
# or, from the repo root:
stellaribc contracts build
```

Artifacts land under the workspace `target/wasm32v1-none/contract/`. See each
contract's own README for its API and storage layout.
