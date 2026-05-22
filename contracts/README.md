# `contracts/` — Stellar IBC v2 contract set

Soroban implementation of the IBC v2 (Eureka) protocol stack, adapted to:

- Soroban has no inheritance → uses modules within a contract crate instead
  of Eureka's `Upgradeable` mixins.
- Soroban contracts can't share a WASM binary (one `__constructor` per WASM)
  → each role is its own crate with its own deployable artifact.
- Cross-contract calls go through `env.invoke_contract` (router → LC,
  router → app); contract addresses are the wire.
- Wire shapes (Packet, Payload, callback structs) are repeated per crate as
  `#[contracttype]` rather than shared via a types crate. The XDR shape is
  the contract — if you change one, change all consumers to match.

## Layout

```
contracts/
├── router/                              -- stellar-ibc-router
├── light-clients/
│   ├── mock/                            -- stellar-mock-light-client (dev only)
│   ├── attestation/                     -- stellar-attestation-light-client
│   └── tendermint/                      -- stellar-tendermint-light-client (scaffold)
└── transfer-app/                        -- stellar-transfer-app (ICS-20 v2)
```

Each subdirectory has its own README with the per-contract surface,
entrypoint list, and dedicated flow diagram:

- [`router/README.md`](router/README.md)
- [`light-clients/mock/README.md`](light-clients/mock/README.md)
- [`light-clients/attestation/README.md`](light-clients/attestation/README.md)
- [`light-clients/tendermint/README.md`](light-clients/tendermint/README.md)
- [`transfer-app/README.md`](transfer-app/README.md)

## Roles at a glance

| Crate | Role |
|---|---|
| `router` | ICS-26 router + ICS-02 client registry + ICS-24 store + ICS-05 port router. Single source of truth for packet commitments, receipts, and acks. |
| `light-clients/mock` | Accept-everything LC. Tests + local dev only. |
| `light-clients/attestation` | m-of-n Ed25519 attestor set. Fastest non-mock LC. |
| `light-clients/tendermint` | Cosmos counterparty LC (Tendermint headers + IAVL proofs). Scaffolded; crypto verifier TODO. |
| `transfer-app` | ICS-20 v2 fungible token transfers. Demonstrates the full ICS-26 app callback surface. |

## How they connect

The router is the hub. Light clients and apps are separate contracts the
router holds addresses for and dials via `env.invoke_contract`.

```
            [admin]
                |
   register_client_type("attestation", addr)
   register_client_type("07-tendermint", addr)
   register_client_type("mock", addr)
   register_port("transfer", transfer-app)
                |
                v
+------------------------------------------------+
|                                                |
|                IbcRouter                       |
|                                                |
| client_id -> lc_address ┐    port_id -> app   |
| client_id -> Counterparty                     |
| v2 path bytes -> commit / receipt / ack       |
|                          |                     |
+-----+----------+---------+----+----------------+
      |          |              |
      |          |              |  on_recv_packet
      |          |              |  on_ack_packet
      |          |              |  on_timeout_packet
      |          |              v
      |          |     +--------+--------+
      |          |     |  IbcTransferApp |
      |          |     |   (or any app)  |
      |          |     +-----------------+
      |          |
      |          | verify_membership
      |          | verify_non_membership
      |          | update_state
      |          | check_for_misbehaviour
      |          | get_timestamp_at_height
      |          v
      |  +-------+---------------------------+
      |  | AttestationLightClient            |
      |  | TendermintLightClient             |
      |  | MockLightClient                   |
      |  +-----------------------------------+
      |
      |  send_packet (from app, app auths itself)
      |  update_client, recv_packet,
      |  acknowledge_packet, timeout_packet
      |    (from relayer via gateway)
      v
   [users / relayer]
```

## Bootstrap order

What an operator runs once per deployment, in this exact order, before any
packet can flow:

1. **Deploy** all five contract WASMs to Stellar.
2. **Construct** the router: `IbcRouter::__constructor(admin)`.
3. **Construct** each LC with its own state (no router pointer — LCs are
   stateless about the router; the router holds their address).
4. **Construct** apps that point at the router:
   `IbcTransferApp::__constructor(router_addr, admin)`.
5. `router.register_client_type("mock", mock_lc_addr)` (and for any other
   client_type you intend to use).
6. `router.register_port("transfer", transfer_app_addr)` (and for any other
   port).
7. For each counterparty chain you want to bridge to:
   - `router.create_client(client_type, client_state, consensus_state, height)`
     → returns `client_id` (e.g. `mock-0`).
   - `router.register_counterparty(client_id, counterparty_client_id, prefix)`
     to lock in the counterparty's `client_id` + commitment prefix.

## End-to-end flow — Stellar → counterparty transfer

```
[user wallet]
    |  initiate_transfer(sender, source_client_id, denom, amount, receiver, timeout, memo)
    v
[IbcTransferApp] ----enforce_rate_limit / debit sender / credit escrow---------+
    |  env.invoke_contract(router, "send_packet", [client_id, timeout, vec![payload]])
    v
[IbcRouter] -----+
    |            +-- port_app(payload.source_port).require_auth()   <-- transfer-app auths
    |            +-- counterparty(source_client_id)                 <-- lookup dest_client
    |            +-- commit_v2_packet(packet)                       <-- ICS-24 commit hash
    |            +-- set_packet_commitment(client_id, seq, hash)    <-- v2 ICS-24 path store
    |            +-- emit SendPacket(client_id, sequence, packet)
    v
[returns sequence]

  [relayer subscribes to SendPacket events via the gateway, builds a
   MsgRecvPacket for the counterparty chain, and submits it there]

  [eventually the counterparty's ack returns via MsgAckPacket / MsgTimeoutPacket]
```

When the ack returns:

```
[relayer]
    |  StellarGatewayMsg/AckPacket
    v
[stellar-hermes-gateway]
    |  Soroban tx with router.acknowledge_packet(packet, acks, proof, proof_height)
    v
[IbcRouter] ----+
    |           +-- packet_commitment(source_client, seq)   <-- match stored hash
    |           +-- counterparty(source_client).client_id   <-- match dest_client
    |           +-- env.invoke_contract(LC, "verify_membership", ...)
    |           +-- env.invoke_contract(transfer-app, "on_acknowledgement_packet", cb)
    |           +-- delete_packet_commitment(source_client, seq)
    |           +-- emit AckPacket
    v
[IbcTransferApp]
    |  if ack == [0x01] (success): no-op (escrow already settled on counterparty)
    |  else: refund(sender, denom, amount) from escrow
```

## End-to-end flow — counterparty → Stellar transfer

```
[relayer]
    |  StellarGatewayMsg/RecvPacket(packet, proof, proof_height)
    v
[stellar-hermes-gateway]
    |  Soroban tx with router.recv_packet(packet, proof, proof_height)
    v
[IbcRouter] ----+
    |           +-- counterparty(packet.dest_client).client_id == packet.source_client
    |           +-- timeout_timestamp > now
    |           +-- !has_packet_receipt(dest_client, seq)         <-- replay guard
    |           +-- commit_v2_packet(packet) -> hash               <-- recompute
    |           +-- env.invoke_contract(LC, "verify_membership",
    |                  [client_id, proof_height, proof, counterparty_path, hash])
    |           +-- set_packet_receipt(dest_client, seq)
    |           +-- env.invoke_contract(transfer-app, "on_recv_packet", cb)
    |                    -> returns ack bytes
    |           +-- commit_v2_acknowledgement(acks) -> ack_hash
    |           +-- set_ack_commitment(dest_client, seq, ack_hash)
    |           +-- emit RecvPacket + WriteAck
    v
[IbcTransferApp]
    |  decode FungibleTokenPacketData from payload.value
    |  credit(receiver, denom, amount)
    |  return ack = [0x01]    <-- success sentinel
```

The relayer then picks up the WriteAck event and submits MsgAckPacket on
the counterparty so the source side can settle (or refund on error ack).

## Client lifecycle (any direction)

```
[admin]
    |  router.register_client_type(client_type, lc_addr)
    v
[IbcRouter] stores DataKey::ClientTypeAddr(client_type) -> lc_addr

[admin]
    |  router.create_client(client_type, client_state, consensus_state, height)
    v
[IbcRouter] -- mint client_id ("{type}-{N}")
    |        -- env.invoke_contract(lc_addr, "initialise",
    |              [client_id, client_state, consensus_state, height])
    |        -- store DataKey::ClientType(client_id) -> client_type
    |        -- store DataKey::ClientLcAddr(client_id) -> lc_addr
    v
[returns client_id]

[admin]
    |  router.register_counterparty(client_id, counterparty_client_id, prefix)
    v
[IbcRouter] stores DataKey::Counterparty(client_id) -> {counterparty_client_id, prefix}

[relayer]
    |  router.update_client(client_id, client_message)
    v
[IbcRouter] -- env.invoke_contract(LC, "check_for_misbehaviour", ...)
    |        -- if misbehaviour: env.invoke_contract(LC, "update_state_on_misbehaviour", ...)
    |                            + set DataKey::Frozen(client_id) -> true
    |        -- else:             env.invoke_contract(LC, "update_state", ...) -> new_height
```
