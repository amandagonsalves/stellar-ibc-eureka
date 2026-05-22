#![cfg(test)]

use super::*;
use mock_light_client::{MockLightClient, MockLightClientClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Bytes, BytesN, Env, String};

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
#[should_panic(expected = "counterparty already registered")]
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
    router.register_counterparty(&id, &cp_id, &prefix);
}

#[test]
#[should_panic(expected = "client_id not found")]
fn register_counterparty_rejects_unknown_client() {
    let (env, _router_id, router, _lc_id) = setup();
    router.register_counterparty(
        &String::from_str(&env, "mock-999"),
        &String::from_str(&env, "07-tendermint-0"),
        &vec![&env, Bytes::from_slice(&env, b"ibc")],
    );
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
#[should_panic(expected = "port already registered")]
fn register_port_rejects_duplicate() {
    let (env, _router_id, router, _lc_id) = setup();
    let app_a = Address::generate(&env);
    let app_b = Address::generate(&env);
    let port_id = String::from_str(&env, "transfer");

    router.register_port(&port_id, &app_a);
    router.register_port(&port_id, &app_b);
}

#[test]
fn provable_paths_are_keyed_by_distinct_discriminators() {
    let (env, _router_id, router, _lc_id) = setup();
    let client_id = String::from_str(&env, "10-stellar-0");

    let same_value = BytesN::from_array(&env, &[0xAA; 32]);
    router.set_packet_commitment(&client_id, &1, &same_value);
    router.set_ack_commitment(&client_id, &1, &same_value);
    router.set_packet_receipt(&client_id, &1);

    assert_eq!(router.packet_commitment(&client_id, &1).unwrap(), same_value);
    assert_eq!(router.acknowledgement(&client_id, &1).unwrap(), same_value);
    assert!(router.has_packet_receipt(&client_id, &1));

    router.delete_packet_commitment(&client_id, &1);
    assert!(router.packet_commitment(&client_id, &1).is_none());
    assert_eq!(router.acknowledgement(&client_id, &1).unwrap(), same_value);
    assert!(router.has_packet_receipt(&client_id, &1));
}
