# Build Award Application

## Submission Title 
* Make a unique title that's different from your project name and reflects what you’re asking funding for.

> Trust-Minimized IBC for Stellar

## Project Type 
> Bridge

## Project URL
> [Github](https://github.com/amandagonsalves/stellar-ibc)

## Technical Architecture Document 
> [Architecture](https://github.com/amandagonsalves/stellar-ibc/blob/main/ARCHITECTURE.md) · [Strategy & rationale](https://github.com/amandagonsalves/stellar-ibc/blob/main/docs/STRATEGY.md)

## Video URL


## Products & Services
* Keep it succinct, and for each feature add how Stellar is used and how the improvements will impact your project.

> 1. **On-chain IBC v2 light-client verification on Soroban.** A full IBC v2 (Eureka) stack implemented as Soroban contracts: an `ibc-router`, light clients (`tendermint`, `attestation`, `mock`), a deterministic fixed-depth-64 SMT, and an ICS-23 membership/non-membership proof serializer. *How Stellar is used:* counterparty packet commitments, receipts, and acknowledgements are committed to a Cardano-compatible SMT and verified on-chain by Soroban contracts — no multisig committee, no federated signers; packet security equals the security of the connected chains. *Impact:* makes Stellar a first-class IBC chain, the foundation every transfer and every connected chain depends on.
> 2. **Trust-minimized cross-chain transfers (ICS-20) with a Hermes relayer.** An `ibc-transfer` Soroban app plus a `StellarChainEndpoint` in the [Cardano Foundation Hermes fork](https://github.com/cardano-foundation/hermes-relayer), fronted by a gRPC `gateway` and an HTTP `api` that build unsigned Soroban transactions the relayer signs and submits (the gateway holds no key). *How Stellar is used:* the transfer app escrows/credits Stellar assets and emits IBC v2 packets; a Stellar light client compiled to wasm is uploaded to the counterparty chain via `08-wasm` so it can verify Stellar proofs. *Impact:* Stellar stablecoins (USDC, EURC) and native assets reach the entire IBC graph, and IBC-native assets reach Stellar's payment and anchor rails — both directions, trust-minimized.
> 3. **`stellaribc` orchestration CLI + a reusable multi-chain stack.** A single Rust binary that deploys the contracts, uploads the Stellar `08-wasm` light client, creates clients, registers counterparties, and runs the relayer — no shell scripts. *How Stellar is used:* it drives the Soroban CLI, `stellar-api`, and Docker to stand up and operate a complete Stellar IBC deployment reproducibly. *Impact:* the same protocol layer, relayer, and tooling that connect Stellar to Cosmos extend to Cardano and any future IBC chain — the marginal cost of the next chain is one light client + one endpoint, so the bridge scales O(n), not O(n²).

## Traction Evidence

The Stellar IBC v2 stack is implemented and demonstrably working end-to-end on a local devnet against a real ibc-go v11 + `08-wasm` Cosmos chain (`ghcr.io/cosmos/ibc-go-wasm-simd`):

- **Both light clients are live.** A Tendermint client (`07-tendermint`) created on the Stellar router, and a Stellar `08-wasm` client created on the Cosmos chain (the Stellar light client compiled to wasm and gov-uploaded via `MsgStoreCode`).
- **Counterparties registered in both directions** (IBC v2 `registerCounterparty`) — the chains are bound and ready to carry packets, with no v1 connection/channel handshake.
- **ICS-20 transfer origination working on Stellar** — the transfer app escrows assets and emits an IBC v2 `SendPacket` through the router, driven by `stellaribc transfer`.
- **Soroban protocol layer complete** — `ibc-router`, `ibc-transfer`, light clients, the Cardano-compatible SMT, and the ICS-23 proof serializer, all in Rust with unit tests.
- **Relayer integration** — a `StellarChainEndpoint` and `ics10_stellar` client types in the shared Hermes fork; the relayer recognizes and tracks the Stellar `08-wasm` client and health-checks the Cosmos chain.
- **Built on the only production non-Cosmos IBC stack.** This project reuses the Cardano Foundation's Hermes fork, light-client patterns, and reference Cosmos entrypoint — cutting years off the work and validating that the architecture generalizes across consensus families (Ouroboros, SCP, Tendermint).

The one piece between here and a fully relayed transfer is the relayer's packet worker (observe `SendPacket` → submit `RecvPacket` with proof → `AckPacket`), which is the first MVP deliverable below.

## SCF Build Tranche Deliverables

> Budget figures below are placeholders — set the amounts to match your target total and team size.

### Tranche 0 (Approval)

> **Deliverable 1 — Demonstrated IBC v2 foundation (Stellar ↔ Cosmos).**
> - *Description:* The implemented stack proving the approach: both light clients created, counterparties registered in both directions, and ICS-20 transfer origination on Stellar, all running against a real ibc-go v11 + `08-wasm` Cosmos chain. Public repo, architecture/strategy docs, and a reproducible `stellaribc` devnet.
> - *Completion:* Done at submission (verifiable from the repository).
> - *Budget:* _(TBD)_

### Tranche 1 - MVP
> **Deliverable 1 — Fully relayed Stellar ↔ Cosmos ICS-20 transfers, both directions.**
> - *Description:* Complete the Hermes packet-relay worker for the Stellar endpoint (observe `SendPacket`, fetch the commitment proof, submit `RecvPacket`; observe `WriteAcknowledgement`, submit `AckPacket`; handle timeouts). Deliver an end-to-end **`stellar→cosmos`** transfer (Stellar asset escrowed → received on Cosmos → acknowledged) and the reverse **`cosmos→stellar`** transfer (Cosmos `MsgTransfer` → minted/credited on Stellar → acknowledged), exercised by the `stellaribc transfer` command.
> - *Completion:* ~6–8 weeks.
> - *Budget:* _(TBD)_
>
> **Deliverable 2 — Light-client correctness + proof verification.**
> - *Description:* Validate the Tendermint light client on Stellar against real Cosmos headers (`update_client`, `verify_membership`) and the Stellar `08-wasm` light client against Stellar SCP proofs + ICS-23 membership/non-membership, so packet proofs are verified on-chain in both directions.
> - *Completion:* ~4 weeks (overlaps Deliverable 1).
> - *Budget:* _(TBD)_

### Tranche 2 - Testnet
> **Deliverable 1 — Public testnet deployment (Stellar ↔ Cosmos).**
> - *Description:* Deploy the contracts to Stellar testnet, connect to a public Cosmos testnet counterparty, run the relayer continuously, and publish operator documentation + monitoring. Relayed transfers observable on public explorers.
> - *Completion:* ~4 weeks.
> - *Budget:* _(TBD)_
>
> **Deliverable 2 — Direct Stellar ↔ Cardano IBC (no chain in the middle).**
> - *Description:* Stand up a Cardano light client on the Stellar router and the Stellar `08-wasm` client on the Cardano side, register counterparties, and deliver direct **`stellar→cardano`** and **`cardano→stellar`** ICS-20 transfers over the shared Hermes fork. This is the payoff of the reusable architecture: a second non-Cosmos chain pair with no new bridge — just one light client + one endpoint per side.
> - *Completion:* ~8–10 weeks.
> - *Budget:* _(TBD)_
>
> **Deliverable 3 — Multi-hop routing through the IBC graph.**
> - *Description:* Packet-forwarding so assets route across more than one hop: **`cardano→cosmos→stellar`** and **`stellar→cosmos→cardano`**. Demonstrates Stellar participating in the full IBC topology, where any IBC chain pair is reachable without a bilateral bridge.
> - *Completion:* ~4–6 weeks.
> - *Budget:* _(TBD)_

### Tranche 3 - Mainnet
> **Deliverable 1 — Security audit + hardening.**
> - *Description:* Third-party audit of the Soroban contracts (router, transfer, light clients), the wasm light client, and the relayer integration; remediation; fuzzing of the SMT/proof paths; key-management and operational-security review.
> - *Completion:* ~8 weeks.
> - *Budget:* _(TBD)_
>
> **Deliverable 2 — Mainnet launch + production relayer operations.**
> - *Description:* Deploy contracts and light clients to Stellar mainnet, register mainnet counterparties for the Cosmos and Cardano routes, run a production relayer with monitoring/alerting and rate limits, and ship operator + integrator documentation (so anyone can run a Stellar IBC relayer and any app can plug in).
> - *Completion:* ~6 weeks.
> - *Budget:* _(TBD)_
