<p align="center">
  <img src="docs/assets/thumbnail.png" alt="Stellar IBC Bridge" width="100%" />
</p>

# Stellar IBC Bridge

Rust implementation of **IBC v2 (Eureka)** for the Stellar network, enabling
trustless cross-chain communication between Stellar and any IBC-enabled chain.
The first counterparty is a Cosmos chain (ibc-go v10+ with the `08-wasm`
light-client module); the same machinery extends to Cardano and beyond.

> **Status — early, under active development. This is a test implementation, not
> production-ready.** On a local devnet (live Soroban testnet + ibc-go v11
> `simd`), a single ICS-20 transfer Stellar→Cosmos has been relayed and
> **verified on-chain** by the `08-wasm` light client (SCP header + ICS-23/SMT
> commitment proof), after which Cosmos minted the IBC voucher with a success
> acknowledgement. The acknowledgement back-leg and the reverse direction
> (Cosmos→Stellar) are in progress, and error/timeout paths, broader test
> coverage, and a security review are still ahead.

---

## What it is

A bridge that connects **Stellar** (Soroban smart contracts, SCP consensus) to
the IBC network using **IBC v2 (Eureka)** — the streamlined protocol that drops
the v1 connection and channel handshakes, keeping only the packet lifecycle.

The defining property is that **no component holds bridge funds or attests to
events off-chain.** Cross-chain authenticity is verified by on-chain light
clients; the relayer, gateway, and api are untrusted transport. A malicious
relayer can stall or censor, but cannot mint, steal, or forge a transfer — the
security of a packet equals the security of the two underlying chains.

It ships as reusable **infrastructure, not a point bridge**: the marginal cost of
the next chain is one light client plus one relayer chain-endpoint, so the same
stack reaches Cosmos today and Cardano (and multi-hop routes) next.

## How it works

Authenticity is checked by an **on-chain light client of the source chain,
running inside the destination chain**. A packet sent on one chain is committed
to that chain's provable state; the relayer carries the packet plus a Merkle
proof to the other chain, whose light client verifies the proof against a header
it has already accepted.

The pieces:

- **Soroban contracts** — the `ibc-router` (IBC v2 core: client/counterparty
  registration, `send` / `recv` / `ack` / `timeout`, and the provable
  commitment/receipt/ack store), the `ibc-transfer` ICS-20 application, and the
  on-chain light clients (`tendermint`, `attestation`, `mock`).
- **`light-client-wasm`** — the Stellar light client compiled to wasm and
  deployed on the counterparty via `08-wasm`; verifies SCP `EXTERNALIZE`
  envelopes and ICS-23 proofs against the Stellar state root.
- **`stellar-hermes-gateway`** — the keyless gRPC service the relayer talks to;
  tracks the state root and produces proofs.
- **`stellar-api`** — the HTTP service that owns the Soroban RPC connection and
  the signing key, building and submitting transactions on the gateway's behalf.
- **Hermes relayer (fork)** — a `StellarChainEndpoint` plus a channel-less v2
  packet-relay worker that observes events, builds the IBC v2 messages, and
  relays them in both directions.
- **`stellaribc` CLI** — the orchestrator that deploys the contracts, uploads
  the wasm light client, creates clients, registers counterparties, and runs the
  services.

Provable state is a deterministic fixed-depth-64 **Sparse Merkle Tree** whose
root is the consensus root counterparty light clients verify against, with proofs
serialized as ICS-23 `MerkleProof`s — a format shared with Cardano so the same
machinery serves both ecosystems.

For the full trust model, component breakdown, and per-flow sequence diagrams,
see the architecture document linked below.

## Project structure

```
stellar-ibc/
├── crates/
│   ├── core/        shared library — SMT, ICS-23 proofs, commitment paths, RPC + HTTP clients
│   ├── gateway/     stellar-hermes-gateway — keyless gRPC service
│   └── api/         stellar-api — HTTP service that owns the Soroban RPC + signing key
├── cli/             stellaribc — the orchestrator CLI
├── contracts/
│   ├── soroban/     ibc-router, ibc-transfer, light-clients/{tendermint,attestation,mock}
│   └── cosmwasm/    light-client — the Stellar light client built for Cosmos 08-wasm
└── docs/            architecture, strategy, and application documents
```

The relayer-side pieces (`StellarChainEndpoint`, the `ics10-stellar` client
types, and the v2 packet worker) live in a fork of the Hermes relayer, alongside
the Cardano endpoint they mirror.

## Documentation

- [Architecture](ARCHITECTURE.md) — trust model, components, data flows, and
  Mermaid sequence diagrams for each flow.
- [Strategy](docs/STRATEGY.md) — the *why*: why IBC, why v2, why this design.
- [Application](docs/APPLICATION.md) — project background and scope.

## License

Licensed under the [Apache License 2.0](LICENSE).
