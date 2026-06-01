# Smart Contracts

The bridge's on-chain code, split by target chain / runtime:

- [`soroban/`](soroban) — Stellar Soroban contracts (the IBC v2 router, the
  ICS-20 transfer app, and the light clients). Their own nested Cargo workspace,
  built to wasm with the Soroban `contract` profile.
- [`cosmwasm/`](cosmwasm) — the Stellar light client compiled for the Cosmos
  side and packaged for `08-wasm`. A member of the root workspace.

| Path | Crate | Runtime | Role |
|---|---|---|---|
| `soroban/ibc-router` | `stellar-ibc-router` | Soroban | IBC v2 router: clients, counterparties, packet routing |
| `soroban/ibc-transfer` | `stellar-ibc-transfer` | Soroban | ICS-20 fungible-token transfer app |
| `soroban/light-clients/mock` | `stellar-mock-light-client` | Soroban | always-accept LC (development) |
| `soroban/light-clients/attestation` | `stellar-attestation-light-client` | Soroban | federated attestation LC (pending) |
| `soroban/light-clients/tendermint` | `stellar-tendermint-light-client` | Soroban | Tendermint LC (pending) |
| `cosmwasm/light-client` | `light-client-wasm` | CosmWasm | Stellar LC for Cosmos `08-wasm` |

Each subdirectory has its own README with the contract's API.
