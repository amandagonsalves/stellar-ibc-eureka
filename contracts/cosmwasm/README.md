# CosmWasm Contracts

The Cosmos side of the bridge's light-client support.

## `light-client/` — `light-client-wasm`

The Stellar light client compiled to `wasm32-unknown-unknown` and uploaded to
the Cosmos chain via `08-wasm` (ibc-go). It conforms to the `08-wasm` storage
ABI and verifies Stellar SCP `EXTERNALIZE` consensus plus ICS-23 membership /
non-membership proofs against the SMT root.

Unlike the Soroban contracts (which form their own nested workspace), this crate
is a **member of the root workspace** at `contracts/cosmwasm/light-client`.

| Source | Role |
|---|---|
| `src/entrypoint.rs` | `instantiate` / `sudo` / `query` 08-wasm entry points |
| `src/store.rs` · `src/smt.rs` · `src/merkle.rs` | 08-wasm storage ABI, SMT, ICS-23 proof walking |
| `src/types.rs` · `src/msg.rs` · `src/error.rs` | client/consensus state, sudo+query messages, errors |

### Upload

```sh
# from the repo root — gov-submits store-code to the Cosmos chain
stellaribc contracts upload-wasm
```

`08-wasm` store-code is governance-gated, so upload goes through a gov proposal
(auto-handled against the local Cosmos `simd-1` devnet). The SMT + ICS-23 helpers it
shares with the rest of the stack live in
[`crates/core/src/ibc/`](../../crates/core/src/ibc/).
