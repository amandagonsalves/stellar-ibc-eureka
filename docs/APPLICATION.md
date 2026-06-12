# Build Award Application

## Submission Title
> Eureka: Trust-Minimized IBC for Stellar

## Project Type
> Bridge

## Project URL
> [Overview](https://amandagonsalves.github.io/sdf-ibc-proposal/index.html)

## Technical Architecture Document
> [Architecture](https://amandagonsalves.github.io/sdf-ibc-proposal/architecture.html)

## Video URL
[TODO: add youtube url]

## Products & Services

**On-chain IBC v2 light-client verification on Soroban.** A full IBC v2 (Eureka) protocol stack as Soroban contracts implementing the Interchain Standards: an `ibc-router` (**ICS-26** routing + **ICS-04** packet semantics), light clients (`tendermint`, `attestation`, `mock`) (**ICS-02**), a deterministic fixed-depth-64 Sparse Merkle Tree for the **ICS-24** host paths, and an **ICS-23** membership/non-membership proof serializer.
- *How Stellar is used:* counterparty packet commitments, receipts, and acknowledgements are committed to the SMT and verified **on-chain by Soroban contracts** (`VerifyClientMessage`/`UpdateState`, `VerifyMembership`/`VerifyNonMembership`) — no multisig committee, no federated signers; packet security equals the security of the connected chains.
- *Impact:* this is the verification root everything else depends on. It makes Stellar a first-class IBC chain and proves Soroban is production-ready for serious systems work (on-chain SMT + proof verification).

**Trust-minimized cross-chain transfers (ICS-20) with a Hermes relayer.** An `ibc-transfer` Soroban app plus a `StellarChainEndpoint` in the shared Cardano Foundation Hermes fork, fronted by a gRPC `gateway` and an HTTP `api` that build unsigned Soroban transactions the relayer signs and submits (the gateway holds no key).
- *How Stellar is used:* the transfer app runs the **ICS-20** routing callbacks (`OnSendPacket` escrow, `OnRecvPacket` mint/credit, `OnAcknowledgementPacket` settle, `OnTimeoutPacket` refund) over **ICS-04** packets; the Stellar light client is compiled to wasm and uploaded to the counterparty via `08-wasm` so it can verify Stellar proofs.
- *Impact:* Stellar stablecoins (USDC, EURC) and native assets reach the entire IBC graph, and IBC-native assets reach Stellar's payment and anchor rails — both directions, trust-minimized.

> **A transfer in ICS terms.** Setup is `RegisterCounterparty` per side (**ICS-26**). Stellar→Cosmos: `ibc-transfer` escrows + builds `FungibleTokenPacketData` (**ICS-20** `OnSendPacket`) → `ibc-router.send_packet` writes the commitment (**ICS-04**/**ICS-24**) → relayer proves it (**ICS-23**) → the Cosmos `08-wasm` Stellar LC verifies the SCP header (**ICS-02**) and commitment (**ICS-23**) on-chain → mints the voucher (**ICS-20** `OnRecvPacket`). Ack back: the success ack `{"result":"AQ=="}` is proven (**ICS-23**) and relayed to `acknowledge_packet` (**ICS-04**), which settles the escrow (**ICS-20**); timeouts refund via an **ICS-23** non-membership proof.

**`interstellar` orchestration CLI + a reusable multi-chain stack.** A single Rust binary that deploys the contracts, uploads the Stellar `08-wasm` light client, creates clients, registers counterparties, and runs the relayer — no shell scripts.
- *How Stellar is used:* it drives the Soroban CLI, `stellar-api`, and Docker to stand up and operate a complete Stellar IBC deployment reproducibly.
- *Impact:* the same protocol layer, relayer, and tooling that connect Stellar to Cosmos extend to Cardano and any future IBC chain — the marginal cost of the next chain is one light client + one endpoint, so the bridge scales O(n), not O(n²).

**End-user transfer dApp + operator/integrator tooling.** A web app (Freighter + Keplr) that turns the transfer flow into a one-click product with a live status stepper, plus an operator runbook, integrator guide, and monitoring dashboard.
- *How Stellar is used:* the dApp signs the `initiate_transfer` Soroban invocation with Freighter and tracks the resulting voucher; nothing is custodial and no key leaves the user.
- *Impact:* satisfies the SCF mainnet UX-readiness bar and lowers adoption cost — any Stellar app can plug into the transfer flow, and any operator can run a Stellar IBC relayer.

## Requested Budget

> $150.0K

## Traction Evidence

This is a pre-launch infrastructure project, so traction is **technical proof + market validation + demand signals**, not live user metrics yet.

### Technical traction

Already built and demonstrably working, tracked against the Interchain Standards the stack implements. The Stellar IBC v2 stack runs end-to-end on a local devnet against a real ibc-go v11 + `08-wasm` Cosmos chain:

- **ICS-26 (Routing) — done.** The `ibc-router` Soroban contract dispatches `send` / `recv` / `ack` / `timeout`, and IBC v2 counterparty registration (`registerCounterparty`) is complete on both sides — no v1 connection or channel handshake.
- **ICS-24 (Host requirements) — done.** Packet commitment, receipt, and acknowledgement paths live in a deterministic fixed-depth-64, Cardano-compatible Sparse Merkle Tree whose root is the consensus root counterparty clients verify against.
- **ICS-02 (Client semantics) — done, verification proven on-chain.** A `07-tendermint` client on the Stellar router, and a Stellar `08-wasm` client on Cosmos (the Stellar LC compiled to wasm and gov-uploaded via `MsgStoreCode`); the `08-wasm` client runs `VerifyClientMessage` (SCP quorum) → `UpdateState` on-chain.
- **ICS-23 (Vector commitments) — membership proven on-chain.** The `08-wasm` LC runs `VerifyMembership` (ICS-23 over the SMT) on-chain for `recv`; non-membership (for `timeout`) is implemented.
- **ICS-20 (Fungible token transfer) — Stellar→Cosmos proven on-chain.** `interstellar transfer` escrows + emits a `SendPacket`; the relayer fetches the commitment proof and submits `MsgRecvPacket`; on-chain verification passes and Cosmos mints an `ibc/<hash>` voucher with a success acknowledgement. The reverse direction (Cosmos→Stellar) is next.
- **IBC v2 relayer on the shared Hermes fork:** `StellarChainEndpoint`, `ics10_stellar` types, and a custom v2/Eureka packet-relay worker drive ICS-04 packet semantics (`send` + `recv` + `acknowledge` verified end-to-end, closing the Stellar→Cosmos round trip on-chain; `timeout` implemented).

**From the demo video**

Transactions:
[create_client](https://stellar.expert/explorer/testnet/tx/13070912626638848) | 
[register_counterparty](https://stellar.expert/explorer/testnet/tx/13070925511528448) | 
[update_client](https://stellar.expert/explorer/testnet/tx/13070964166250496) | 
[acknowledge_packet](https://stellar.expert/explorer/testnet/tx/13070968461209600)

Contracts:
[IBC Router](https://stellar.expert/explorer/testnet/contract/CCI4Q3XPN33J7NGFRZASFXB4H6LWVKOMOJUFYQZE6O2MOBZOYXFILZUH) | 
[IBC Transfer](https://stellar.expert/explorer/testnet/contract/CBPQ6JJSKMKGQ4TRZKFUFS5RD2B2EYMJJDORZPKYRGQOBJYSCAMWCCN7) | 
[Tendermint Light Client](https://stellar.expert/explorer/testnet/contract/CAJM575RWFTPWBGAIYIVWS4XKHAQLZPZI4H3GWPZPRZ467WHFMZM7YHL)

### Market validation

- **The interop market is enormous and the trust-minimized slice is unserved on Stellar.** IBC has moved hundreds of billions in cumulative volume with no consensus-level exploit; the Cosmos DeFi venues it connects (Osmosis, Injective, dYdX, Noble) hold billions in liquidity. No trust-minimized, light-client-secured Stellar↔interchain path exists today — existing Stellar bridges are federated/multisig.
- **The problem is expensive and proven.** The five largest bridge hacks (Ronin, Poly, Wormhole, Nomad, Harmony) all stem from the trusted-signer model IBC eliminates.
- **Stellar's distinctive value on the other side:** Stellar is the only trust-minimized payments chain that would be plugged into the largest interop graph — its anchors, regulated stablecoins (USDC, EURC), and cash-out network (MoneyGram) become reachable from Cosmos DeFi, and vice versa.

### Demand signals

> [TODO: add support note from cardano foundation tech head]

## Tranche 1 (Deliverable Roadmap) — MVP

**Goal:** Close the Stellar↔Cosmos ICS-20 loop in both directions, with on-chain proof verification on both sides, on the devnet where the forward leg is already proven.

**Deliverable 1 — Full ICS-04 + ICS-20 round-trip, both directions (Stellar↔Cosmos).**
- *Description:* Complete ICS-04 packet semantics (`send` / `recv` / `acknowledge` / `timeout`) end-to-end via the Hermes v2 packet-relay worker for the Stellar endpoint — relay `SendPacket`→`RecvPacket`, `WriteAcknowledgement`→`AckPacket`, and timeout handling — closing the ICS-20 transfer loop. Deliver `stellar→cosmos` (escrow → received → acknowledged) and the reverse `cosmos→stellar` (`MsgTransfer` → credited/minted on Stellar → acknowledged), driven by `interstellar transfer`.
- *Completion criteria:* A single command runs a full round-trip in each direction on the devnet; relayer logs show ack relayed back and the source commitment cleared; screen recording + GitHub commit range.
- *Estimated completion:* 4 weeks after approval.
- *Budget:* $19,000.

**Deliverable 2 — ICS-02 + ICS-23 conformance on both light clients.**
- *Description:* Bring both light clients to full ICS-02 (client semantics) and ICS-23 (vector commitments) conformance, in both directions. **ICS-02:** validate the `07-tendermint` LC on Stellar against real Cosmos headers (`update_client`) and the Stellar `08-wasm` LC against SCP `EXTERNALIZE` proofs (`VerifyClientMessage` → `UpdateState`). **ICS-23:** verify membership (`verify_membership`, for recv/ack) and non-membership (for timeout) proofs on-chain on both clients, against the Cosmos consensus root and the Stellar SMT root respectively.
- *Completion criteria:* Test suite demonstrating ICS-02 header updates and ICS-23 membership + non-membership proof verification passing on both clients; GitHub link to tests + results.
- *Estimated completion:* 6 weeks after approval.
- *Budget:* $11,000.

**Deliverable 3 — Real-asset escrow via the Stellar Asset Contract (SAC).**
- *Description:* Wire `ibc-transfer`'s ICS-20 escrow path to the canonical **Stellar Asset Contract (SAC)** token interface, so real Stellar assets — native XLM and issued stablecoins (USDC, EURC) — move through the transfer app by their SAC token addresses rather than a development token. `OnSendPacket` escrows the SAC asset into the contract (`transfer` under `require_auth`); `OnRecvPacket` mints/credits the voucher; `OnAcknowledgementPacket` settles and `OnTimeoutPacket` releases the escrow back to the sender.
- *Completion criteria:* A transfer of a real SAC asset on the devnet/testnet (e.g. testnet USDC) escrowed on send and released on a successful ack, plus a timeout-refund path; tx hashes + screen recording + GitHub commit range.
- *Estimated completion:* 8 weeks after approval.
- *Budget:* $15,000.

## Tranche 2 (Deliverable Roadmap) — Testnet

**Goal:** Run continuously on public testnets, prove the architecture generalizes by adding a second non-Cosmos chain (direct Cardano), and open it to community testing.

**Deliverable 1 — Public testnet deployment + continuous relayer ops (Stellar↔Cosmos).**
- *Description:* Deploy contracts to Stellar testnet, connect to a public Cosmos testnet counterparty, run the relayer continuously with monitoring/alerting, and publish operator documentation. Transfers observable on public explorers.
- *Completion criteria:* Public contract addresses; a live relayer running ≥7 days; ≥1 transfer per direction visible on stellar.expert + a Cosmos explorer; operator runbook published.
- *Dependency (third party, outside our control):* requires a public ibc-go v10+ chain with the `08-wasm` module to **approve and store the Stellar light client via on-chain governance** — a `store-code` proposal voted in by that chain's validators, with the checksum allow-listed in the `08-wasm` module. We can build, deploy, and run everything else, but a public Cosmos chain must agree to host the Stellar light client. We de-risk by targeting operators amenable to new `08-wasm` clients and coordinating the proposal ahead of time (or using a permissioned testnet where we can drive the governance directly).
- *Estimated completion:* 11 weeks after approval.
- *Budget:* $20,000.

**Deliverable 2 — Direct Stellar↔Cardano IBC (no chain in the middle).**
- *Description:* Stand up a Cardano light client on the Stellar router and the Stellar `08-wasm` client on the Cardano side, register counterparties, and deliver direct `stellar→cardano` and `cardano→stellar` ICS-20 transfers over the shared Hermes fork — the payoff of the reusable architecture: a second non-Cosmos pair with no new bridge.
- *Completion criteria:* Both clients created and counterparties registered; ≥1 transfer per direction completed on testnet with on-chain proof verification; screen recording + addresses.
- *Dependency (third party, outside our control):* (a) **Cardano IBC must be working and solid on its public testnet** — the Cardano-side IBC stack and light client operational, which is driven by the Cardano Foundation, not by us; and (b) the **Stellar light client approved and stored on a public IBC-v2 + `08-wasm` chain via that chain's governance** (per Deliverable 1's dependency). Both are coordinated ahead of time (we are building in collaboration with the Cardano Foundation), but neither is unilaterally in this team's control.
- *Estimated completion:* 15 weeks after approval.
- *Budget:* $18,000.

**Deliverable 3 — Integration test suite + community testable build.**
- *Description:* End-to-end integration tests covering both corridors and edge cases (timeouts, failed acks, client refresh); a documented, reproducible `interstellar` devnet/testnet build shared with the Stellar Discord for feedback.
- *Completion criteria:* Passing CI test suite (GitHub link); testable build + instructions shared in Discord; collected feedback summary.
- *Estimated completion:* 17 weeks after approval.
- *Budget:* $7,000.

## Tranche 3 (Deliverable Roadmap) — Mainnet

**Goal:** Ship to mainnet with production relayer operations, and meet the SCF 7.0 UX-readiness gate with both an end-user dApp and operator/integrator UX.

**Deliverable 1 — Security review + hardening.**
- *Description:* Internal security review of the Soroban contracts, the wasm LC, and the relayer integration; fuzzing of the SMT/proof paths; key-management/operational-security review; remediation of findings from the LaunchKit Audit Bank third-party audit (credits unlocked at T2).
- *Completion criteria:* Published internal review summary + fuzzing report; audit findings triaged and remediated (diff links); no open critical/high issues.
- *Estimated completion:* 20 weeks after approval.
- *Budget:* $19,000. (External audit via LaunchKit — not budgeted.)

**Deliverable 2 — Mainnet launch + production relayer operations.**
- *Description:* Deploy contracts and light clients to Stellar mainnet; register mainnet counterparties for the Cosmos (and, if D2/T2 landed, Cardano) routes; run a production relayer with monitoring/alerting and rate limits.
- *Completion criteria:* Mainnet contract addresses; ≥1 live mainnet transfer per direction (tx hashes); relayer running stably ≥7 consecutive days with monitoring dashboard.
- *Dependency (third party, outside our control):* the **Stellar light client must be approved and stored on the target Cosmos mainnet** (and on Cardano mainnet, if that leg landed) via each chain's **on-chain governance** — controlled by those chains' validators/operators, not by us. Mainnet is additionally gated on the security review + external audit (Deliverable 1) completing with no open critical/high findings before any value moves.
- *Estimated completion:* 23 weeks after approval.
- *Budget:* $18,000.

**Deliverable 3 — End-user transfer dApp (UX readiness).**
- *Description:* Connect Freighter (Stellar) + Keplr (Cosmos), enter amount + receiver, sign `initiate_transfer`, and watch a status stepper (`escrowed → relaying → received`) as the voucher appears — plus a `GET /config` api endpoint so nothing is hardcoded. Includes onboarding (wallet setup guidance, test-token button), error handling, and an FAQ.
- *Completion criteria:* Live demo URL; screen recording of a full transfer; onboarding flow + FAQ present; works against testnet and (post-D2) mainnet.
- *Estimated completion:* 25 weeks after approval.
- *Budget:* $16,000.

**Deliverable 4 — Operator/integrator UX + documentation.**
- *Description:* Polished `interstellar` operator UX, an operator runbook (run your own Stellar IBC relayer), an integrator guide (plug an app into the transfer flow), a public monitoring dashboard, and a published docs site.
- *Completion criteria:* Docs site live at a public URL (with TOC); operator + integrator guides published; dashboard URL showing relay/transfer activity.
- *Estimated completion:* 26 weeks after approval.
- *Budget:* $7,000.
