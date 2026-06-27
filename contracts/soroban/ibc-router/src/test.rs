#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{contract, contractimpl, vec, Bytes, BytesN, Env, String};
use stellar_mock_light_client::{MockLightClient, MockLightClientClient};
use types::{Packet, Payload};

#[contract]
pub struct EchoApp;

#[contractimpl]
impl EchoApp {
    pub fn on_recv_packet(_env: Env, cb: OnRecvPacketCallback) -> Bytes {
        cb.payload.value
    }
    pub fn on_acknowledgement_packet(_env: Env, _cb: OnAcknowledgementPacketCallback) {}
    pub fn on_timeout_packet(_env: Env, _cb: OnTimeoutPacketCallback) {}
}

fn setup() -> (Env, Address, IbcRouterClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let router_id = env.register(IbcRouter, (admin.clone(),));
    let router = IbcRouterClient::new(&env, &router_id);

    let lc_id = env.register(MockLightClient, ());

    router.register_client_type(&String::from_str(&env, "mock"), &lc_id);
    (env, router_id, router, lc_id)
}

#[test]
fn register_client_type_stores_lc_address() {
    let (env, _router_id, router, lc_id) = setup();
    let resolved = router.lc_address(&String::from_str(&env, "mock")).unwrap();
    assert_eq!(resolved, lc_id);
}

#[test]
fn create_client_initialises_lc_and_returns_unique_id() {
    let (env, _router_id, router, lc_id) = setup();

    let cs = Bytes::from_slice(&env, b"client-state");
    let cons = Bytes::from_slice(&env, b"consensus-state");
    let id1 = router.create_client(&String::from_str(&env, "mock"), &cs, &cons, &1);
    let id2 = router.create_client(&String::from_str(&env, "mock"), &cs, &cons, &1);

    assert_ne!(id1, id2);
    assert_eq!(id1, String::from_str(&env, "mock-0"));
    assert_eq!(id2, String::from_str(&env, "mock-1"));

    let lc = MockLightClientClient::new(&env, &lc_id);
    assert_eq!(lc.latest_height(&id1), 1);
    assert_eq!(lc.client_state(&id1), cs);
}

#[test]
fn register_counterparty_stores_mapping() {
    let (env, _router_id, router, _lc_id) = setup();

    let id = router.create_client(
        &String::from_str(&env, "mock"),
        &Bytes::from_slice(&env, b"cs"),
        &Bytes::from_slice(&env, b"cons"),
        &1,
    );

    let counterparty_id = String::from_str(&env, "07-tendermint-0");
    let prefix = vec![&env, Bytes::from_slice(&env, b"ibc")];
    router.register_counterparty(&id, &counterparty_id, &prefix);

    let cp = router.counterparty(&id).unwrap();
    assert_eq!(cp.client_id, counterparty_id);
    assert_eq!(cp.commitment_prefix.len(), 1);
}

#[test]
fn register_counterparty_rejects_duplicate() {
    let (env, _router_id, router, _lc_id) = setup();
    let id = router.create_client(
        &String::from_str(&env, "mock"),
        &Bytes::from_slice(&env, b"cs"),
        &Bytes::from_slice(&env, b"cons"),
        &1,
    );
    let cp_id = String::from_str(&env, "07-tendermint-0");
    let prefix = vec![&env, Bytes::from_slice(&env, b"ibc")];
    router.register_counterparty(&id, &cp_id, &prefix);

    let result = router.try_register_counterparty(&id, &cp_id, &prefix);
    assert_eq!(result, Err(Ok(Error::CounterpartyAlreadyRegistered.into())));
}

#[test]
fn register_counterparty_rejects_unknown_client() {
    let (env, _router_id, router, _lc_id) = setup();
    let result = router.try_register_counterparty(
        &String::from_str(&env, "mock-999"),
        &String::from_str(&env, "07-tendermint-0"),
        &vec![&env, Bytes::from_slice(&env, b"ibc")],
    );
    assert_eq!(result, Err(Ok(Error::ClientIdNotFound.into())));
}

#[test]
fn update_client_bumps_height_via_lc() {
    let (env, _router_id, router, lc_id) = setup();
    let id = router.create_client(
        &String::from_str(&env, "mock"),
        &Bytes::from_slice(&env, b"cs"),
        &Bytes::from_slice(&env, b"cons"),
        &5,
    );

    let new_h = router.update_client(&id, &Bytes::from_slice(&env, b"msg"));
    assert_eq!(new_h, 6);
    assert_eq!(
        MockLightClientClient::new(&env, &lc_id).latest_height(&id),
        6
    );
    assert!(!router.frozen(&id));
}

#[test]
fn packet_commitment_round_trips_and_can_be_deleted() {
    let (env, _router_id, router, _lc_id) = setup();
    let client_id = String::from_str(&env, "10-stellar-0");
    let hash = BytesN::from_array(&env, &[0xAB; 32]);

    assert!(router.packet_commitment(&client_id, &7).is_none());
    router.set_packet_commitment(&client_id, &7, &hash);
    assert_eq!(router.packet_commitment(&client_id, &7).unwrap(), hash);

    router.delete_packet_commitment(&client_id, &7);
    assert!(router.packet_commitment(&client_id, &7).is_none());
}

#[test]
fn packet_receipt_round_trips() {
    let (env, _router_id, router, _lc_id) = setup();
    let client_id = String::from_str(&env, "10-stellar-0");

    assert!(!router.has_packet_receipt(&client_id, &3));
    router.set_packet_receipt(&client_id, &3);
    assert!(router.has_packet_receipt(&client_id, &3));
    assert!(!router.has_packet_receipt(&client_id, &4));
}

#[test]
fn ack_commitment_round_trips() {
    let (env, _router_id, router, _lc_id) = setup();
    let client_id = String::from_str(&env, "10-stellar-0");
    let ack = BytesN::from_array(&env, &[0xCD; 32]);

    assert!(router.acknowledgement(&client_id, &11).is_none());
    router.set_ack_commitment(&client_id, &11, &ack);
    assert_eq!(router.acknowledgement(&client_id, &11).unwrap(), ack);
}

#[test]
fn register_port_stores_app_address() {
    let (env, _router_id, router, _lc_id) = setup();
    let app = Address::generate(&env);
    let port_id = String::from_str(&env, "transfer");

    assert!(router.port_app(&port_id).is_none());
    router.register_port(&port_id, &app);
    assert_eq!(router.port_app(&port_id).unwrap(), app);
}

#[test]
fn register_port_distinct_ports_isolated() {
    let (env, _router_id, router, _lc_id) = setup();
    let app_a = Address::generate(&env);
    let app_b = Address::generate(&env);
    let port_a = String::from_str(&env, "transfer");
    let port_b = String::from_str(&env, "echo");

    router.register_port(&port_a, &app_a);
    router.register_port(&port_b, &app_b);

    assert_eq!(router.port_app(&port_a).unwrap(), app_a);
    assert_eq!(router.port_app(&port_b).unwrap(), app_b);
}

#[test]
fn register_port_rejects_duplicate() {
    let (env, _router_id, router, _lc_id) = setup();
    let app_a = Address::generate(&env);
    let app_b = Address::generate(&env);
    let port_id = String::from_str(&env, "transfer");

    router.register_port(&port_id, &app_a);
    let result = router.try_register_port(&port_id, &app_b);
    assert_eq!(result, Err(Ok(Error::PortAlreadyRegistered.into())));
}

#[test]
fn provable_paths_are_keyed_by_distinct_discriminators() {
    let (env, _router_id, router, _lc_id) = setup();
    let client_id = String::from_str(&env, "10-stellar-0");

    let same_value = BytesN::from_array(&env, &[0xAA; 32]);
    router.set_packet_commitment(&client_id, &1, &same_value);
    router.set_ack_commitment(&client_id, &1, &same_value);
    router.set_packet_receipt(&client_id, &1);

    assert_eq!(
        router.packet_commitment(&client_id, &1).unwrap(),
        same_value
    );
    assert_eq!(router.acknowledgement(&client_id, &1).unwrap(), same_value);
    assert!(router.has_packet_receipt(&client_id, &1));

    router.delete_packet_commitment(&client_id, &1);
    assert!(router.packet_commitment(&client_id, &1).is_none());
    assert_eq!(router.acknowledgement(&client_id, &1).unwrap(), same_value);
    assert!(router.has_packet_receipt(&client_id, &1));
}

struct PacketFlow {
    env: Env,
    router: IbcRouterClient<'static>,
    client_id: String,
    counterparty_client_id: String,
    port_id: String,
    #[allow(dead_code)]
    app: Address,
}

fn setup_packet_flow() -> PacketFlow {
    let (env, _router_id, router, _lc_id) = setup();

    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let client_id = router.create_client(
        &String::from_str(&env, "mock"),
        &Bytes::from_slice(&env, b"cs"),
        &Bytes::from_slice(&env, b"cons"),
        &1,
    );
    let counterparty_client_id = String::from_str(&env, "07-tendermint-0");
    let prefix = vec![&env, Bytes::from_slice(&env, b"ibc")];
    router.register_counterparty(&client_id, &counterparty_client_id, &prefix);

    let app = env.register(EchoApp, ());
    let port_id = String::from_str(&env, "transfer");
    router.register_port(&port_id, &app);

    PacketFlow {
        env,
        router,
        client_id,
        counterparty_client_id,
        port_id,
        app,
    }
}

fn mk_payload(env: &Env, port: &String, value: &[u8]) -> Payload {
    Payload {
        source_port: port.clone(),
        dest_port: port.clone(),
        version: String::from_str(env, "v1"),
        encoding: String::from_str(env, "json"),
        value: Bytes::from_slice(env, value),
    }
}

#[test]
fn send_packet_mints_sequences_and_stores_commitment() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"hello");

    let seq1 = f
        .router
        .send_packet(&f.client_id, &2_000, &vec![&f.env, payload.clone()]);
    let seq2 = f
        .router
        .send_packet(&f.client_id, &2_000, &vec![&f.env, payload]);
    assert_eq!(seq1, 1);
    assert_eq!(seq2, 2);
    assert!(f.router.packet_commitment(&f.client_id, &seq1).is_some());
    assert!(f.router.packet_commitment(&f.client_id, &seq2).is_some());
}

#[test]
fn send_packet_rejects_expired_timeout() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"x");
    let result = f
        .router
        .try_send_packet(&f.client_id, &500, &vec![&f.env, payload]);
    assert_eq!(result, Err(Ok(Error::TimeoutAlreadyElapsed.into())));
}

#[test]
fn send_packet_rejects_no_counterparty() {
    let (env, _router_id, router, _lc_id) = setup();
    env.ledger().with_mut(|li| li.timestamp = 1_000);
    let client_id = router.create_client(
        &String::from_str(&env, "mock"),
        &Bytes::from_slice(&env, b"cs"),
        &Bytes::from_slice(&env, b"cons"),
        &1,
    );
    let app = env.register(EchoApp, ());
    let port_id = String::from_str(&env, "transfer");
    router.register_port(&port_id, &app);
    let payload = mk_payload(&env, &port_id, b"x");
    let result = router.try_send_packet(&client_id, &2_000, &vec![&env, payload]);
    assert_eq!(result, Err(Ok(Error::CounterpartyNotRegistered.into())));
}

#[test]
fn send_packet_rejects_empty_payloads() {
    let f = setup_packet_flow();
    let result = f
        .router
        .try_send_packet(&f.client_id, &2_000, &vec![&f.env]);
    assert_eq!(result, Err(Ok(Error::PayloadsEmpty.into())));
}

fn recv_packet_for(f: &PacketFlow, sequence: u64, value: &[u8]) -> Packet {
    let payload = mk_payload(&f.env, &f.port_id, value);
    Packet {
        sequence,
        source_client: f.counterparty_client_id.clone(),
        dest_client: f.client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![&f.env, payload],
    }
}

#[test]
fn recv_packet_stores_receipt_and_ack() {
    let f = setup_packet_flow();
    let packet = recv_packet_for(&f, 1, b"hello");

    f.router
        .recv_packet(&packet, &Bytes::from_slice(&f.env, b"proof"), &10);

    assert!(f.router.has_packet_receipt(&f.client_id, &1));
    assert!(f.router.acknowledgement(&f.client_id, &1).is_some());
}

#[test]
fn recv_packet_rejects_replay() {
    let f = setup_packet_flow();
    let packet = recv_packet_for(&f, 1, b"hello");
    let proof = Bytes::from_slice(&f.env, b"proof");

    f.router.recv_packet(&packet, &proof, &10);
    let result = f.router.try_recv_packet(&packet, &proof, &10);
    assert_eq!(result, Err(Ok(Error::ReceiptAlreadyExists.into())));
}

#[test]
fn recv_packet_rejects_expired_timeout() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"x");
    let stale_packet = Packet {
        sequence: 1,
        source_client: f.counterparty_client_id.clone(),
        dest_client: f.client_id.clone(),
        timeout_timestamp: 500,
        payloads: vec![&f.env, payload],
    };
    let result = f
        .router
        .try_recv_packet(&stale_packet, &Bytes::from_slice(&f.env, b"p"), &10);
    assert_eq!(result, Err(Ok(Error::TimeoutAlreadyElapsed.into())));
}

#[test]
fn recv_packet_rejects_wrong_counterparty() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"x");
    let mismatched_packet = Packet {
        sequence: 1,
        source_client: String::from_str(&f.env, "07-tendermint-99"),
        dest_client: f.client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![&f.env, payload],
    };
    let result =
        f.router
            .try_recv_packet(&mismatched_packet, &Bytes::from_slice(&f.env, b"p"), &10);
    assert_eq!(result, Err(Ok(Error::PacketCounterpartyMismatch.into())));
}

#[test]
fn write_acknowledgement_after_recv_rejects_duplicate() {
    let f = setup_packet_flow();
    let packet = recv_packet_for(&f, 1, b"hello");
    f.router
        .recv_packet(&packet, &Bytes::from_slice(&f.env, b"proof"), &10);

    let result = f.router.try_write_acknowledgement(
        &f.client_id,
        &1,
        &vec![&f.env, Bytes::from_slice(&f.env, b"more")],
    );
    assert_eq!(result, Err(Ok(Error::AckAlreadyExists.into())));
}

#[test]
fn write_acknowledgement_rejects_when_no_receipt() {
    let f = setup_packet_flow();
    let result = f.router.try_write_acknowledgement(
        &f.client_id,
        &1,
        &vec![&f.env, Bytes::from_slice(&f.env, b"ack")],
    );
    assert_eq!(result, Err(Ok(Error::NoReceiptForSequence.into())));
}

#[test]
fn acknowledge_packet_clears_commitment() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"hello");
    let seq = f
        .router
        .send_packet(&f.client_id, &2_000, &vec![&f.env, payload.clone()]);
    assert!(f.router.packet_commitment(&f.client_id, &seq).is_some());

    let packet = Packet {
        sequence: seq,
        source_client: f.client_id.clone(),
        dest_client: f.counterparty_client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![&f.env, payload],
    };
    let acks = vec![&f.env, Bytes::from_slice(&f.env, b"ack-bytes")];
    f.router
        .acknowledge_packet(&packet, &acks, &Bytes::from_slice(&f.env, b"proof"), &10);

    assert!(f.router.packet_commitment(&f.client_id, &seq).is_none());
}

#[test]
fn acknowledge_packet_rejects_when_no_commitment() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"x");
    let packet = Packet {
        sequence: 99,
        source_client: f.client_id.clone(),
        dest_client: f.counterparty_client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![&f.env, payload],
    };
    let result = f.router.try_acknowledge_packet(
        &packet,
        &vec![&f.env, Bytes::from_slice(&f.env, b"ack")],
        &Bytes::from_slice(&f.env, b"proof"),
        &10,
    );
    assert_eq!(result, Err(Ok(Error::NoCommitmentForSequence.into())));
}

#[test]
fn timeout_packet_clears_commitment_when_elapsed() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"hello");
    let seq = f
        .router
        .send_packet(&f.client_id, &2_000, &vec![&f.env, payload.clone()]);
    assert!(f.router.packet_commitment(&f.client_id, &seq).is_some());

    let packet = Packet {
        sequence: seq,
        source_client: f.client_id.clone(),
        dest_client: f.counterparty_client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![&f.env, payload],
    };
    let result = f
        .router
        .try_timeout_packet(&packet, &Bytes::from_slice(&f.env, b"proof"), &10);
    assert_eq!(result, Err(Ok(Error::TimeoutNotYetElapsed.into())));
    assert!(f.router.packet_commitment(&f.client_id, &seq).is_some());
}

#[test]
fn timeout_packet_rejects_when_no_commitment() {
    let f = setup_packet_flow();
    let payload = mk_payload(&f.env, &f.port_id, b"x");
    let packet = Packet {
        sequence: 99,
        source_client: f.client_id.clone(),
        dest_client: f.counterparty_client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![&f.env, payload],
    };
    let result = f
        .router
        .try_timeout_packet(&packet, &Bytes::from_slice(&f.env, b"proof"), &10);
    assert_eq!(result, Err(Ok(Error::NoCommitmentForSequence.into())));
}
