#![cfg(test)]

use super::*;
use soroban_sdk::{xdr::ToXdr, Bytes, BytesN, Env, String};

fn setup() -> (Env, TendermintLightClientClient<'static>, String) {
    let env = Env::default();
    let contract_id = env.register(TendermintLightClient, ());
    let client = TendermintLightClientClient::new(&env, &contract_id);

    let cs = TendermintClientState {
        chain_id: String::from_str(&env, "cosmoshub-4"),
        trust_level: TrustThreshold {
            numerator: 2,
            denominator: 3,
        },
        trusting_period_secs: 1_209_600,
        unbonding_period_secs: 1_814_400,
        max_clock_drift_secs: 30,
        latest_height: Height {
            revision_number: 0,
            revision_height: 100,
        },
        is_frozen: false,
        frozen_height: Height {
            revision_number: 0,
            revision_height: 0,
        },
        proof_specs: Bytes::new(&env),
    };
    let cons = TendermintConsensusState {
        timestamp_secs: 1_000_000,
        next_validators_hash: BytesN::from_array(&env, &[0x11; 32]),
        root: BytesN::from_array(&env, &[0x22; 32]),
    };

    let client_id = String::from_str(&env, "07-tendermint-0");
    client.initialise(&client_id, &cs.to_xdr(&env), &cons.to_xdr(&env), &100);
    (env, client, client_id)
}

fn header(env: &Env, target_h: u64, ts: u64, root: [u8; 32]) -> Bytes {
    TendermintHeader {
        trusted_height: Height {
            revision_number: 0,
            revision_height: 100,
        },
        target_height: Height {
            revision_number: 0,
            revision_height: target_h,
        },
        timestamp_secs: ts,
        next_validators_hash: BytesN::from_array(env, &[0x33; 32]),
        app_hash: BytesN::from_array(env, &root),
        signed_header_bytes: Bytes::from_slice(env, b"signed-header-placeholder"),
        validator_set_bytes: Bytes::from_slice(env, b"validator-set-placeholder"),
    }
    .to_xdr(env)
}

#[test]
fn initialise_stores_state_and_consensus() {
    let (_env, client, id) = setup();
    assert_eq!(client.latest_height(&id), 100);
    assert_eq!(client.get_timestamp_at_height(&id, &100), 1_000_000);
    assert!(!client.frozen(&id));
}

#[test]
fn update_state_advances_height_and_records_consensus_state() {
    let (env, client, id) = setup();
    let new_h = client.update_state(&id, &header(&env, 105, 1_000_500, [0x44; 32]));
    assert_eq!(new_h, 105);
    assert_eq!(client.latest_height(&id), 105);
    assert_eq!(client.get_timestamp_at_height(&id, &105), 1_000_500);
}

#[test]
fn update_state_rejects_non_advancing_height() {
    let (env, client, id) = setup();
    let result = client.try_update_state(&id, &header(&env, 100, 1_000_500, [0x44; 32]));
    assert_eq!(result, Err(Ok(Error::HeaderHeightNotAfterTrusted.into())));
}

#[test]
fn check_for_misbehaviour_detects_conflicting_root() {
    let (env, client, id) = setup();
    client.update_state(&id, &header(&env, 105, 1_000_500, [0x44; 32]));

    let conflicting = header(&env, 105, 1_000_600, [0xCC; 32]);
    assert!(client.check_for_misbehaviour(&id, &conflicting));
}

#[test]
fn check_for_misbehaviour_idempotent_for_same_root() {
    let (env, client, id) = setup();
    client.update_state(&id, &header(&env, 105, 1_000_500, [0x44; 32]));

    let same = header(&env, 105, 9_999_999, [0x44; 32]);
    assert!(!client.check_for_misbehaviour(&id, &same));
}

#[test]
fn update_state_on_misbehaviour_freezes_client() {
    let (env, client, id) = setup();
    assert!(!client.frozen(&id));
    client.update_state_on_misbehaviour(&id, &Bytes::new(&env));
    assert!(client.frozen(&id));
}

#[test]
fn update_state_rejected_once_frozen() {
    let (env, client, id) = setup();
    client.update_state_on_misbehaviour(&id, &Bytes::new(&env));
    let result = client.try_update_state(&id, &header(&env, 105, 1_000_500, [0x44; 32]));
    assert_eq!(result, Err(Ok(Error::ClientFrozen.into())));
}

#[test]
fn verify_membership_accepts_when_consensus_exists() {
    // Stub verifier — anything passes against a valid (height, client).
    // Production must walk an ICS-23 proof; this test only locks the
    // dispatch path through the router today.
    let (env, client, id) = setup();
    assert!(client.verify_membership(
        &id,
        &100,
        &Bytes::from_slice(&env, b"proof"),
        &Bytes::from_slice(&env, b"path"),
        &Bytes::from_slice(&env, b"value"),
    ));
}

#[test]
fn verify_membership_rejects_unknown_height() {
    let (env, client, id) = setup();
    assert!(!client.verify_membership(
        &id,
        &999,
        &Bytes::from_slice(&env, b"proof"),
        &Bytes::from_slice(&env, b"path"),
        &Bytes::from_slice(&env, b"value"),
    ));
}

#[test]
fn verify_non_membership_rejects_when_frozen() {
    let (env, client, id) = setup();
    client.update_state_on_misbehaviour(&id, &Bytes::new(&env));
    assert!(!client.verify_non_membership(
        &id,
        &100,
        &Bytes::from_slice(&env, b"proof"),
        &Bytes::from_slice(&env, b"path"),
    ));
}

#[test]
fn client_state_round_trips_through_xdr() {
    let (env, client, id) = setup();
    let bytes = client.client_state(&id);
    let decoded = TendermintClientState::from_xdr(&env, &bytes).expect("decode");
    assert_eq!(decoded.chain_id, String::from_str(&env, "cosmoshub-4"));
    assert_eq!(decoded.latest_height.revision_height, 100);
    assert!(!decoded.is_frozen);
}

#[test]
fn consensus_state_round_trips_through_xdr() {
    let (env, client, id) = setup();
    let bytes = client.consensus_state(&id, &100);
    let decoded = TendermintConsensusState::from_xdr(&env, &bytes).expect("decode");
    assert_eq!(decoded.timestamp_secs, 1_000_000);
    assert_eq!(decoded.root, BytesN::from_array(&env, &[0x22; 32]));
}
