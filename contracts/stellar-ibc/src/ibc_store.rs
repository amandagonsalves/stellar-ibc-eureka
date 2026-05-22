use soroban_sdk::{Bytes, BytesN, Env, String};

use crate::ics24_host::{
    ack_commitment_path, packet_commitment_path, packet_receipt_path, PROVABLE_TTL_EXTEND_TO,
    PROVABLE_TTL_THRESHOLD, RECEIPT_SENTINEL,
};

pub(crate) fn set_packet_commitment(
    env: &Env,
    source_client_id: &String,
    sequence: u64,
    commitment: &BytesN<32>,
) {
    let key = packet_commitment_path(env, source_client_id, sequence);
    let storage = env.storage().persistent();
    storage.set(&key, commitment);
    storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
}

pub(crate) fn packet_commitment(
    env: &Env,
    source_client_id: &String,
    sequence: u64,
) -> Option<BytesN<32>> {
    let key = packet_commitment_path(env, source_client_id, sequence);
    env.storage().persistent().get(&key)
}

pub(crate) fn delete_packet_commitment(env: &Env, source_client_id: &String, sequence: u64) {
    let key = packet_commitment_path(env, source_client_id, sequence);
    env.storage().persistent().remove(&key);
}

pub(crate) fn set_packet_receipt(env: &Env, dest_client_id: &String, sequence: u64) {
    let key = packet_receipt_path(env, dest_client_id, sequence);
    let sentinel = Bytes::from_slice(env, &[RECEIPT_SENTINEL]);
    let storage = env.storage().persistent();
    storage.set(&key, &sentinel);
    storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
}

pub(crate) fn has_packet_receipt(env: &Env, dest_client_id: &String, sequence: u64) -> bool {
    let key = packet_receipt_path(env, dest_client_id, sequence);
    env.storage().persistent().has(&key)
}

pub(crate) fn set_ack_commitment(
    env: &Env,
    dest_client_id: &String,
    sequence: u64,
    ack_hash: &BytesN<32>,
) {
    let key = ack_commitment_path(env, dest_client_id, sequence);
    let storage = env.storage().persistent();
    storage.set(&key, ack_hash);
    storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
}

pub(crate) fn acknowledgement(
    env: &Env,
    dest_client_id: &String,
    sequence: u64,
) -> Option<BytesN<32>> {
    let key = ack_commitment_path(env, dest_client_id, sequence);
    env.storage().persistent().get(&key)
}
