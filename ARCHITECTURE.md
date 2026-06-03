<p align="center">
  <img src="docs/assets/thumbnail.png" alt="Stellar IBC Bridge" width="100%" />
</p>

# Architecture

How the Stellar IBC v2 (Eureka) bridge is put together: the trust model, the
components and their contracts, the packet lifecycle, and why the design extends
to any IBC chain. Written for reviewers, integrators, and contributors.

For the *why* behind each choice (why IBC, why v2, why Hermes, why Cardano),
see [`docs/STRATEGY.md`](docs/STRATEGY.md). This document covers the *how*.

---

## 1. System at a glance

The bridge connects **Stellar** (Soroban smart contracts, SCP consensus) to any
**IBC-enabled chain**. The first counterparty is a Cosmos chain (ibc-go v10+
with the `08-wasm` light-client module; the local devnet runs ibc-go v11
`simd`); the same machinery extends to Cardano and beyond.

```
┌──────────────────────────────────────────────────────────────────────────┐
│            Hermes relayer fork (cardano-foundation/hermes-relayer)         │
│  crates/relayer/src/chain/stellar/StellarChainEndpoint                     │
│  crates/relayer-types/src/clients/ics10_stellar/  (client/consensus types) │
└───────────────┬───────────────────────────────────────┬───────────────────┘
                │ gRPC :50052                            │ Tendermint RPC + gRPC
                ▼                                         │
┌──────────────────────────────────────────────┐         │
│  stellar-hermes-gateway   (gRPC, no key)      │         │
│   StellarGatewayQuery + StellarGatewayMsg     │         │
│   StateTracker: SMT root + ICS-23 proofs      │         │
│   ApiClient ───────────────┐                  │         │
└────────────────────────────│──────────────────┘         │
                             │ HTTP :8101                  │
                             ▼                             │
┌──────────────────────────────────────────────┐          │
│  stellar-api   (axum, owns RPC + signing key) │          │
│   /ledger /account /balance /events           │          │
│   /tx/prepare  /tx/submit  /stellar/clients   │          │
│   /cosmos/*  (counterparty gov + bank helpers)│          │
└───────────────┬──────────────────────────────┘          │
                │ Soroban JSON-RPC                          ▼
                ▼                          ┌────────────────────────────────┐
┌──────────────────────────────────┐      │  Cosmos counterparty           │
│  Stellar / Soroban               │      │  ibc-go v10+ · 08-wasm         │
│   soroban-testnet or local node  │      │  hosts the Stellar light client│
└───────────────┬──────────────────┘      └────────────────────────────────┘
                │ contract invokes
                ▼
┌────────────────────────────────────────────────────────────────────────┐
│  Soroban contracts (contracts/soroban)                                   │
│   ibc-router         create_client · register_counterparty · send/recv/  │
│                      ack/timeout · commitment & receipt & ack store       │
│   ibc-transfer       ICS-20 escrow / mint / refund                        │
│   light-clients/     tendermint · attestation · mock                      │
│  contracts/cosmwasm/light-client → light-client-wasm (Stellar LC, 08-wasm)│
└──────────────────────────────────────────────────────────────────────────┘
```

The defining property: **no component holds bridge funds or attests to events
off-chain.** Verification happens inside on-chain light clients. The relayer,
gateway, and api are untrusted transport.

---

## 2. Trust model

IBC is trust-minimized because packet authenticity is checked by an **on-chain
light client of the source chain, running inside the destination chain**:

- A packet sent on chain A is committed to A's provable state.
- The relayer carries the packet plus a Merkle proof to chain B.
- Chain B's light client of A verifies the proof against a header it has already
  accepted from A. If the proof is valid, the packet is genuine.

There is no validator committee, no multisig federation, no off-chain signer set
that can be compromised to forge a transfer. The security of a packet equals the
security of the two underlying chains — nothing weaker. Each role in this repo is
deliberately minimal in trust:

| Role | Holds funds? | Holds keys? | Trusted for correctness? |
|---|---|---|---|
| `ibc-router` + light clients (on-chain) | escrow only | — | **yes** — this is the verification root |
| `stellar-api` | no | yes (relayer signing key) | no — only signs what the relayer asks |
| `stellar-hermes-gateway` | no | no | no — pure transport/encoding |
| Hermes relayer | no | yes (its own fee key) | no — a wrong/missing relay cannot forge a packet, only delay it |

A malicious relayer can censor or stall, but cannot mint, steal, or forge —
the on-chain light client rejects any packet without a valid proof.

---

## 3. Components & contracts

| Component | Crate / path | Responsibility |
|---|---|---|
| **`ibc-router`** | `contracts/soroban/ibc-router` | The IBC v2 core on Stellar. Registers client types and counterparties, dispatches `send/recv/ack/timeout`, and owns the provable commitment / receipt / ack store. |
| **`ibc-transfer`** | `contracts/soroban/ibc-transfer` | ICS-20 application. Escrows on send, credits on recv, refunds on timeout/failed-ack. Encodes `FungibleTokenPacketData`. |
| **light clients** | `contracts/soroban/light-clients/{tendermint,attestation,mock}` | Verify counterparty headers + membership proofs on Stellar. `tendermint` tracks a Cosmos chain; `mock` is always-accept for development. |
| **`light-client-wasm`** | `contracts/cosmwasm/light-client` | The **Stellar** light client, compiled to `wasm32-unknown-unknown` and uploaded to the counterparty via `08-wasm`. Verifies SCP `EXTERNALIZE` envelopes + ICS-23 proofs against the SMT root. |
| **`stellar-ibc-core`** | `crates/core` | Shared library: the fixed-depth-64 SMT (`smt.rs`), ICS-23 proof serializer (`proof.rs`), IBC commitment paths (`commitment.rs`), client/consensus types, the Soroban RPC client, and the HTTP `ApiClient`. |
| **`stellar-hermes-gateway`** | `crates/gateway` | gRPC service the relayer talks to. Holds **no** Soroban connection and **no** key — every call is fulfilled through `ApiClient` against `stellar-api`. Tracks the SMT root and produces proofs. |
| **`stellar-api`** | `crates/api` | Standalone HTTP service that owns the Soroban RPC connection and the signing key. Builds unsigned txs (`/tx/prepare`), submits signed txs (`/tx/submit`), and exposes ledger/account/event reads plus Cosmos-side gov/bank helpers. |
| **`StellarChainEndpoint`** | Hermes fork `crates/relayer/src/chain/stellar` | Implements Hermes's `ChainEndpoint` for Stellar: event polling, building IBC messages, and signing+submitting via the gateway. |
| **`ics10_stellar`** | Hermes fork `crates/relayer-types/src/clients/ics10_stellar` | Stellar client/consensus state types and the v2 message encodings; unwraps the `08-wasm` envelope so the relayer can track the Stellar client on the counterparty. |
| **`stellaribc`** | `cli` | The orchestrator: deploy contracts, upload the wasm light client, create clients, register counterparties, run services. Drives Docker, the `stellar` CLI, and `stellar-api` directly. |

### Why the gateway and api are split

The relayer needs a stable, IBC-shaped gRPC surface; Soroban needs an
RPC connection and a signing key. Splitting them means the **key lives in exactly
one place** (`stellar-api`), the gateway is a stateless protocol adapter, and the
api is independently usable (Swagger UI at `/docs`). The gateway translates IBC
messages to Soroban XDR; the api owns chain I/O and signing.

---

## 4. IBC v2 provable state

IBC v2 (Eureka) keeps only **three** provable paths — the packet lifecycle —
versus eight in v1. This is decisive on Stellar, where Soroban storage is
rent-priced per byte:

| Value | Path bytes |
|---|---|
| Packet Commitment | `{sourceClientId} \|\| 0x01 \|\| be64(sequence)` |
| Packet Receipt | `{destClientId} \|\| 0x02 \|\| be64(sequence)` |
| Acknowledgement Commitment | `{destClientId} \|\| 0x03 \|\| be64(sequence)` |

These live in a **deterministic fixed-depth-64 binary Sparse Merkle Tree**
(`crates/core/src/ibc/smt.rs`), chosen to be **Cardano-compatible** so the same
proof format serves both ecosystems. The SMT root is the `ConsensusState.root`
that counterparty light clients verify against. Membership and non-membership
proofs are serialized as ICS-23 `MerkleProof`s (`crates/core/src/ibc/proof.rs`):
membership proves a commitment exists (recv/ack), non-membership proves a
receipt is absent (timeout).

Because client/connection/channel state is **not** provable in v2, the gateway's
`QueryClientState`, `QueryConsensusState`, and `QueryNextSeqRecv` intentionally
return `Unimplemented`.

---

## 5. Packet lifecycle

### Counterparty registration (once per chain pair)

IBC v2 replaces the v1 connection+channel handshake (8 messages) with a single
call per side:

```
register_counterparty(client_id, counterparty_client_id, merkle_prefix)
```

On Stellar this is an `ibc-router` call; on the Cosmos side it is
`MsgRegisterCounterparty`. After both sides register, packets flow immediately —
no version negotiation, no port binding.

### Transfer (Stellar → counterparty)

```
1. ibc-transfer.initiate_transfer(sender, source_client, denom, amount, receiver, timeout, memo)
        │  escrows the asset, builds FungibleTokenPacketData
        ▼
2. ibc-router.send_packet(source_client, timeout, payloads[])
        │  assigns sequence, writes Packet Commitment to the SMT
        ▼  (relayer observes SendPacket, fetches the commitment proof from the gateway)
3. counterparty: MsgRecvPacket(packet, proof, proofHeight)
        │  the Stellar light client (08-wasm) verifies the proof against its SMT root
        │  → app credits the receiver, writes an acknowledgement
        ▼  (relayer observes WriteAcknowledgement, fetches the ack proof)
4. Stellar: ibc-router.acknowledge_packet(packet, ack, proof, proofHeight)
        │  the tendermint LC verifies the proof; the source commitment is cleared
        ▼
   on failure / timeout → ibc-transfer refunds the escrow
```

The reverse direction (counterparty → Stellar) is symmetric: a `MsgTransfer` on
the Cosmos side, `recv_packet` on the Stellar router (proof verified by the
**tendermint** LC), and the ack relayed back.

---

## 6. Transaction model — prepare → sign → submit

Stellar transactions are built where the chain connection lives (`stellar-api`)
and signed where the key lives, but **driven** by the relayer, so the gateway
never holds a key:

```
relayer ──(IBC msg)──▶ gateway ──▶ api POST /tx/prepare ──▶ unsigned tx_xdr
   ▲                                                            │
   └──────────────── unsigned tx_xdr ◀──────────── gateway ◀────┘
   │
   └─ relayer signs tx_xdr with its key
         └─▶ gateway SubmitSignedTx ─▶ api POST /tx/submit ─▶ Soroban
```

`/tx/prepare` is method-agnostic — it re-encodes any router method's arguments to
Soroban XDR and returns an unsigned transaction — so `create_client`,
`register_counterparty`, `recv_packet`, `ack_packet`, `timeout_packet`,
`update_client`, and `submit_misbehaviour` all flow through the same path.

---

## 7. Light clients — both directions

A bridge needs each chain to verify the other. Two light clients, one per
direction:

### Counterparty → Stellar (`tendermint` LC on Soroban)

A Soroban contract registered under client type `07-tendermint`. It accepts the
Cosmos `ClientState`/`ConsensusState`, verifies header updates (`update_client`),
and verifies ICS-23 membership proofs against the stored consensus root so the
router can accept `recv_packet` / `acknowledge_packet` from Cosmos.

### Stellar → counterparty (`light-client-wasm` via `08-wasm`)

The Stellar light client compiled to `wasm32-unknown-unknown` and uploaded once
to the counterparty via `MsgStoreCode` (`stellaribc contracts upload-wasm`). It:

- verifies SCP `EXTERNALIZE` envelopes (Ed25519 signatures from a quorum of
  trusted validators) to accept a new Stellar header / SMT root, and
- walks the gateway-produced ICS-23 `MerkleProof` against `ConsensusState.root`.

`08-wasm` lets the counterparty host a pluggable light client **without forking
its chain binary** — the reason any ibc-go v10+ chain can become a Stellar
counterparty by uploading one wasm blob.

The relayer's `AnyClientState` / `AnyConsensusState` route the
`/ibc.lightclients.wasm.v1.*` envelope to the Stellar parser (which unwraps it
and preserves the code checksum), so Hermes can track the Stellar `08-wasm`
client on the counterparty like any native client.

---

## 8. Relayer integration

Hermes splits all chain-specific logic behind a `ChainEndpoint` trait. The
Cardano Foundation fork added `CardanoChainEndpoint`; this project adds
`StellarChainEndpoint` the same way. The endpoint:

- **polls** the gateway for Stellar events (`SendPacket`, `WriteAcknowledgement`),
- **builds** IBC v2 messages and obtains unsigned txs from the gateway,
- **signs** with the relayer key and **submits** via `SubmitSignedTx`,
- **queries** clients, packet commitments / receipts / acks with proofs.

Client/consensus tracking: `AnyClientState` / `AnyConsensusState` route the
`/ibc.lightclients.wasm.v1.*` envelope to the Stellar parser, so Hermes tracks
the Stellar `08-wasm` client on Cosmos like a native one; the SDK/ibc-go compat
gates are widened to admit simd v11.

### v2 (Eureka) packet relay — what Hermes doesn't give us

Hermes's stock packet relay (`Link`/`RelayPath`) is **channel-based**: a worker
spawns when the scan finds a v1 channel. IBC v2/Eureka has **no channels**, so
that worker never spawns — and everything below is the v2 equivalent we supply:

```
1. router emits send_packet (Soroban ScVal event: topics + packet map)
2. gateway decodes the event → `attributes` text on the Events RPC
3. StellarChainEndpoint poller → IbcEvent::AppModule(stellaribcrouter, …) → event bus
4. stellar_packet worker (custom, client-paired — not channel-paired):
     a. decode the v2 Packet (sequence, source/dest client, payloads)
     b. query the commitment proof from the gateway (ICS-23 vs the SMT root)
     c. build MsgUpdateClient for the dest 08-wasm client → the proof height
        (the dest client MUST be ≥ proof height or ibc-go rejects the proof —
         exactly the step Hermes's RelayPath does automatically for v1)
     d. build MsgRecvPacket (ibc.core.channel.v2)
     e. submit [update, recv] to Cosmos via the chain handle
5. ibc-go runs the 08-wasm Stellar LC to verify the commitment proof
```

The **gateway state tracker** is what makes step (b) possible: it reconstructs
the SMT by replaying ledger close-meta cumulatively (every ledger from the last
processed up to the queried height, so the send ledger's commitment write is
ingested) and parses Soroban `TransactionMeta` **V4** (the format soroban-testnet
emits). The commitment proof is generated against the same SMT root the
consensus state carries, so the on-chain verify is consistent.

Everything else — event loop, tx queueing, client refresh, fee estimation, key
management, config — is inherited from Hermes unchanged. Because the whole stack
is Rust (contracts, core, gateway, api, wasm LC, and Hermes), the relayer
integration is debuggable end-to-end in one toolchain.

---

## 9. Multi-chain extensibility

The investment is **infrastructure, not a point bridge**. With *n* IBC chains,
custom bridges cost ~n²/2 pairwise integrations; IBC costs *n* light clients + 1
shared protocol + 1 generalized relayer. The marginal cost of the next chain is
**one light client + one chain endpoint**.

Concretely, what this repo reuses for every future chain pair:

- the IBC v2 protocol layer (`ibc-router`, the SMT, ICS-23 proofs),
- the Stellar `08-wasm` light client (verifies on *any* `08-wasm` counterparty),
- the gateway/api transaction surface,
- the `StellarChainEndpoint` in the shared Hermes fork.

This yields the connectivity targets on the roadmap:

| Route | Mechanism |
|---|---|
| `stellar↔cosmos` | Stellar `08-wasm` LC on Cosmos; `07-tendermint` LC on Stellar |
| `stellar↔cardano` | Stellar `08-wasm` LC on Cardano; a Cardano LC on Stellar — **direct, no chain in the middle** |
| `cardano↔cosmos↔stellar`, `stellar↔cosmos↔cardano` | multi-hop packet forwarding across the IBC graph |

Once two non-Cosmos chains both speak IBC, they talk **directly** — no Cosmos
chain has to sit in the middle.

---

## 10. Repository map

```
stellar-ibc/
  crates/
    core/        stellar-ibc-core — SMT, ICS-23 proofs, commitment paths, RPC + HTTP clients
    gateway/     stellar-hermes-gateway — gRPC StellarGatewayQuery + StellarGatewayMsg
    api/         stellar-api — axum HTTP service; owns Soroban RPC + signing key
  cli/           stellar-ibc-cli (stellaribc) — orchestrator (ops, clients, contracts, hermes, cosmos, transfer)
  contracts/
    soroban/     ibc-router, ibc-transfer, light-clients/{tendermint,attestation,mock}
    cosmwasm/    light-client — Stellar LC compiled for Cosmos 08-wasm
  hermes-config.toml   relayer config (mounted into hermes + api)
  docker-compose.yml   profiles: local, cosmos, hermes, local-stellar, staging
```

The Hermes-side pieces (`StellarChainEndpoint`, `ics10_stellar`) live in the
[cardano-foundation/hermes-relayer](https://github.com/cardano-foundation/hermes-relayer)
fork, alongside the Cardano endpoint they mirror.

---

## 11. Runtime & deployment

Everything is driven by the `stellaribc` CLI (no shell scripts). A full local
bring-up:

```sh
stellaribc cosmos start --fresh     # local Cosmos devnet (ibc-go v11 simd + 08-wasm)
stellaribc start --force-redeploy   # deploy contracts, upload the wasm LC, import relayer keys
stellaribc clients cosmos           # 07-tendermint client on Stellar
stellaribc clients stellar          # 08-wasm Stellar client on Cosmos
stellaribc clients counterparty stellar
stellaribc clients counterparty cosmos
stellaribc transfer                 # originate a Stellar → Cosmos ICS-20 transfer
```

`docker-compose.yml` profiles compose the moving parts — the Cosmos chain, the
gateway, the api, and the Hermes relayer — with healthchecks and dependency
ordering. See the [README](README.md) for the full command reference and
configuration.
