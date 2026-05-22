use soroban_sdk::{Bytes, BytesN, Env, String};

pub(crate) const PACKET_COMMITMENT_DISCRIMINATOR: u8 = 0x01;
pub(crate) const PACKET_RECEIPT_DISCRIMINATOR: u8 = 0x02;
pub(crate) const ACK_COMMITMENT_DISCRIMINATOR: u8 = 0x03;

pub(crate) const RECEIPT_SENTINEL: u8 = 0x01;

pub(crate) const PROVABLE_TTL_THRESHOLD: u32 = 17_280;
pub(crate) const PROVABLE_TTL_EXTEND_TO: u32 = 86_400;

pub(crate) fn set_packet_commitment(
    env: &Env,
    source_client_id: &String,
    sequence: u64,
    commitment: &BytesN<32>,
) {
    let key = packet_commitment_key(env, source_client_id, sequence);
    let storage = env.storage().persistent();
    storage.set(&key, commitment);
    storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
}

pub(crate) fn packet_commitment(
    env: &Env,
    source_client_id: &String,
    sequence: u64,
) -> Option<BytesN<32>> {
    let key = packet_commitment_key(env, source_client_id, sequence);
    env.storage().persistent().get(&key)
}

pub(crate) fn delete_packet_commitment(env: &Env, source_client_id: &String, sequence: u64) {
    let key = packet_commitment_key(env, source_client_id, sequence);
    env.storage().persistent().remove(&key);
}

pub(crate) fn set_packet_receipt(env: &Env, dest_client_id: &String, sequence: u64) {
    let key = packet_receipt_key(env, dest_client_id, sequence);
    let sentinel = Bytes::from_slice(env, &[RECEIPT_SENTINEL]);
    let storage = env.storage().persistent();
    storage.set(&key, &sentinel);
    storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
}

pub(crate) fn has_packet_receipt(env: &Env, dest_client_id: &String, sequence: u64) -> bool {
    let key = packet_receipt_key(env, dest_client_id, sequence);
    env.storage().persistent().has(&key)
}

pub(crate) fn set_ack_commitment(
    env: &Env,
    dest_client_id: &String,
    sequence: u64,
    ack_hash: &BytesN<32>,
) {
    let key = ack_commitment_key(env, dest_client_id, sequence);
    let storage = env.storage().persistent();
    storage.set(&key, ack_hash);
    storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
}

pub(crate) fn acknowledgement(
    env: &Env,
    dest_client_id: &String,
    sequence: u64,
) -> Option<BytesN<32>> {
    let key = ack_commitment_key(env, dest_client_id, sequence);
    env.storage().persistent().get(&key)
}

fn packet_commitment_key(env: &Env, source_client_id: &String, sequence: u64) -> Bytes {
    v2_path_key(env, source_client_id, PACKET_COMMITMENT_DISCRIMINATOR, sequence)
}

fn packet_receipt_key(env: &Env, dest_client_id: &String, sequence: u64) -> Bytes {
    v2_path_key(env, dest_client_id, PACKET_RECEIPT_DISCRIMINATOR, sequence)
}

fn ack_commitment_key(env: &Env, dest_client_id: &String, sequence: u64) -> Bytes {
    v2_path_key(env, dest_client_id, ACK_COMMITMENT_DISCRIMINATOR, sequence)
}

fn v2_path_key(env: &Env, client_id: &String, discriminator: u8, sequence: u64) -> Bytes {
    let id_len = client_id.len() as usize;
    let mut buf = [0u8; 128];
    client_id.copy_into_slice(&mut buf[..id_len]);
    buf[id_len] = discriminator;
    let seq_bytes = sequence.to_be_bytes();
    buf[id_len + 1..id_len + 9].copy_from_slice(&seq_bytes);
    Bytes::from_slice(env, &buf[..id_len + 9])
}
