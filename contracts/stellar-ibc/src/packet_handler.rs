use soroban_sdk::{panic_with_error, vec, Bytes, Env, IntoVal, String, Symbol, Vec};

use crate::errors::Error;
use crate::ibc_store;
use crate::ics02_client;
use crate::ics24_host::{commit_v2_acknowledgement, commit_v2_packet, error_ack_hash};
use crate::port_router;
use crate::types::{
    DataKey, OnAcknowledgementPacketCallback, OnRecvPacketCallback, OnTimeoutPacketCallback,
    Packet, Payload,
};

pub(crate) fn send_packet(
    env: &Env,
    source_client_id: String,
    timeout_timestamp: u64,
    payloads: Vec<Payload>,
) -> u64 {
    if payloads.is_empty() {
        panic_with_error!(env, Error::PayloadsEmpty);
    }

    let cp = ics02_client::counterparty(env, source_client_id.clone())
        .unwrap_or_else(|| panic_with_error!(env, Error::CounterpartyNotRegistered));

    if timeout_timestamp <= env.ledger().timestamp() {
        panic_with_error!(env, Error::TimeoutAlreadyElapsed);
    }

    for payload in payloads.iter() {
        let app = port_router::port_app(env, payload.source_port.clone())
            .unwrap_or_else(|| panic_with_error!(env, Error::PortNotRegistered));
        app.require_auth();
    }

    let seq_key = DataKey::NextSeqSend(source_client_id.clone());
    let next: u64 = env
        .storage()
        .persistent()
        .get::<DataKey, u64>(&seq_key)
        .unwrap_or(0)
        + 1;
    env.storage().persistent().set(&seq_key, &next);

    let packet = Packet {
        sequence: next,
        source_client: source_client_id.clone(),
        dest_client: cp.client_id,
        timeout_timestamp,
        payloads,
    };

    let hash = commit_v2_packet(env, &packet);
    ibc_store::set_packet_commitment(env, &source_client_id, next, &hash);

    env.events().publish(
        (Symbol::new(env, "send_packet"), source_client_id, next),
        packet,
    );

    next
}

pub(crate) fn recv_packet(env: &Env, packet: Packet, proof: Bytes, proof_height: u64) {
    let cp = ics02_client::counterparty(env, packet.dest_client.clone())
        .unwrap_or_else(|| panic_with_error!(env, Error::CounterpartyNotRegistered));
    if cp.client_id != packet.source_client {
        panic_with_error!(env, Error::PacketCounterpartyMismatch);
    }

    if packet.timeout_timestamp <= env.ledger().timestamp() {
        panic_with_error!(env, Error::TimeoutAlreadyElapsed);
    }

    if ibc_store::has_packet_receipt(env, &packet.dest_client, packet.sequence) {
        panic_with_error!(env, Error::ReceiptAlreadyExists);
    }

    let expected_hash = commit_v2_packet(env, &packet);
    let cp_path = counterparty_packet_commitment_path(
        env,
        &cp.commitment_prefix,
        &packet.source_client,
        packet.sequence,
    );
    verify_membership(
        env,
        &packet.dest_client,
        proof_height,
        &proof,
        &cp_path,
        &expected_hash.into(),
    );

    ibc_store::set_packet_receipt(env, &packet.dest_client, packet.sequence);

    let acks = dispatch_recv_callbacks(env, &packet);

    let ack_hash = commit_v2_acknowledgement(env, &acks);
    ibc_store::set_ack_commitment(env, &packet.dest_client, packet.sequence, &ack_hash);

    env.events().publish(
        (
            Symbol::new(env, "recv_packet"),
            packet.dest_client.clone(),
            packet.sequence,
        ),
        packet.clone(),
    );
    env.events().publish(
        (
            Symbol::new(env, "write_ack"),
            packet.dest_client,
            packet.sequence,
        ),
        acks,
    );
}

pub(crate) fn write_acknowledgement(
    env: &Env,
    dest_client_id: String,
    sequence: u64,
    acknowledgements: Vec<Bytes>,
) {
    if !ibc_store::has_packet_receipt(env, &dest_client_id, sequence) {
        panic_with_error!(env, Error::NoReceiptForSequence);
    }
    if ibc_store::acknowledgement(env, &dest_client_id, sequence).is_some() {
        panic_with_error!(env, Error::AckAlreadyExists);
    }

    let ack_hash = commit_v2_acknowledgement(env, &acknowledgements);
    ibc_store::set_ack_commitment(env, &dest_client_id, sequence, &ack_hash);

    env.events().publish(
        (Symbol::new(env, "write_ack"), dest_client_id, sequence),
        acknowledgements,
    );
}

pub(crate) fn acknowledge_packet(
    env: &Env,
    packet: Packet,
    acknowledgements: Vec<Bytes>,
    proof: Bytes,
    proof_height: u64,
) {
    let stored = ibc_store::packet_commitment(env, &packet.source_client, packet.sequence)
        .unwrap_or_else(|| panic_with_error!(env, Error::NoCommitmentForSequence));
    let expected_hash = commit_v2_packet(env, &packet);
    if stored != expected_hash {
        panic_with_error!(env, Error::CommitmentMismatch);
    }

    let cp = ics02_client::counterparty(env, packet.source_client.clone())
        .unwrap_or_else(|| panic_with_error!(env, Error::CounterpartyNotRegistered));
    if cp.client_id != packet.dest_client {
        panic_with_error!(env, Error::PacketCounterpartyMismatch);
    }

    let ack_hash = commit_v2_acknowledgement(env, &acknowledgements);
    let cp_path = counterparty_ack_commitment_path(
        env,
        &cp.commitment_prefix,
        &packet.dest_client,
        packet.sequence,
    );
    verify_membership(
        env,
        &packet.source_client,
        proof_height,
        &proof,
        &cp_path,
        &ack_hash.into(),
    );

    dispatch_ack_callbacks(env, &packet, &acknowledgements);

    ibc_store::delete_packet_commitment(env, &packet.source_client, packet.sequence);

    env.events().publish(
        (
            Symbol::new(env, "ack_packet"),
            packet.source_client.clone(),
            packet.sequence,
        ),
        (packet, acknowledgements),
    );
}

pub(crate) fn timeout_packet(env: &Env, packet: Packet, proof: Bytes, proof_height: u64) {
    let stored = ibc_store::packet_commitment(env, &packet.source_client, packet.sequence)
        .unwrap_or_else(|| panic_with_error!(env, Error::NoCommitmentForSequence));
    let expected_hash = commit_v2_packet(env, &packet);
    if stored != expected_hash {
        panic_with_error!(env, Error::CommitmentMismatch);
    }

    let cp = ics02_client::counterparty(env, packet.source_client.clone())
        .unwrap_or_else(|| panic_with_error!(env, Error::CounterpartyNotRegistered));
    if cp.client_id != packet.dest_client {
        panic_with_error!(env, Error::PacketCounterpartyMismatch);
    }

    let lc_addr = ics02_client::client_lc_address(env, packet.source_client.clone())
        .unwrap_or_else(|| panic_with_error!(env, Error::ClientIdNotFound));

    let proof_ts: u64 = env.invoke_contract(
        &lc_addr,
        &Symbol::new(env, "get_timestamp_at_height"),
        vec![
            env,
            packet.source_client.clone().into_val(env),
            proof_height.into_val(env),
        ],
    );
    if proof_ts <= packet.timeout_timestamp {
        panic_with_error!(env, Error::TimeoutNotYetElapsed);
    }

    let cp_path = counterparty_packet_receipt_path(
        env,
        &cp.commitment_prefix,
        &packet.dest_client,
        packet.sequence,
    );
    verify_non_membership(env, &packet.source_client, proof_height, &proof, &cp_path);

    dispatch_timeout_callbacks(env, &packet);

    ibc_store::delete_packet_commitment(env, &packet.source_client, packet.sequence);

    env.events().publish(
        (
            Symbol::new(env, "timeout_packet"),
            packet.source_client.clone(),
            packet.sequence,
        ),
        packet,
    );
}

fn dispatch_recv_callbacks(env: &Env, packet: &Packet) -> Vec<Bytes> {
    let mut acks: Vec<Bytes> = Vec::new(env);
    for payload in packet.payloads.iter() {
        let app = port_router::port_app(env, payload.dest_port.clone())
            .unwrap_or_else(|| panic_with_error!(env, Error::PortNotRegistered));
        let cb = OnRecvPacketCallback {
            source_client: packet.source_client.clone(),
            dest_client: packet.dest_client.clone(),
            sequence: packet.sequence,
            payload,
        };
        let ack: Bytes =
            env.invoke_contract(&app, &Symbol::new(env, "on_recv_packet"), vec![env, cb.into_val(env)]);
        acks.push_back(ack);
    }
    if acks.is_empty() {
        let err_ack: Bytes = error_ack_hash(env).into();
        acks.push_back(err_ack);
    }
    acks
}

fn dispatch_ack_callbacks(env: &Env, packet: &Packet, acks: &Vec<Bytes>) {
    for (i, payload) in packet.payloads.iter().enumerate() {
        let ack = acks
            .get(i as u32)
            .unwrap_or_else(|| error_ack_hash(env).into());
        let app = port_router::port_app(env, payload.source_port.clone())
            .unwrap_or_else(|| panic_with_error!(env, Error::PortNotRegistered));
        let cb = OnAcknowledgementPacketCallback {
            source_client: packet.source_client.clone(),
            dest_client: packet.dest_client.clone(),
            sequence: packet.sequence,
            payload,
            acknowledgement: ack,
        };
        let _: () = env.invoke_contract(
            &app,
            &Symbol::new(env, "on_acknowledgement_packet"),
            vec![env, cb.into_val(env)],
        );
    }
}

fn dispatch_timeout_callbacks(env: &Env, packet: &Packet) {
    for payload in packet.payloads.iter() {
        let app = port_router::port_app(env, payload.source_port.clone())
            .unwrap_or_else(|| panic_with_error!(env, Error::PortNotRegistered));
        let cb = OnTimeoutPacketCallback {
            source_client: packet.source_client.clone(),
            dest_client: packet.dest_client.clone(),
            sequence: packet.sequence,
            payload,
        };
        let _: () = env.invoke_contract(
            &app,
            &Symbol::new(env, "on_timeout_packet"),
            vec![env, cb.into_val(env)],
        );
    }
}

fn verify_membership(
    env: &Env,
    client_id: &String,
    proof_height: u64,
    proof: &Bytes,
    path: &Bytes,
    value: &Bytes,
) {
    let lc_addr = ics02_client::client_lc_address(env, client_id.clone())
        .unwrap_or_else(|| panic_with_error!(env, Error::ClientIdNotFound));
    let ok: bool = env.invoke_contract(
        &lc_addr,
        &Symbol::new(env, "verify_membership"),
        vec![
            env,
            client_id.into_val(env),
            proof_height.into_val(env),
            proof.into_val(env),
            path.into_val(env),
            value.into_val(env),
        ],
    );
    if !ok {
        panic_with_error!(env, Error::MembershipVerificationFailed);
    }
}

fn verify_non_membership(
    env: &Env,
    client_id: &String,
    proof_height: u64,
    proof: &Bytes,
    path: &Bytes,
) {
    let lc_addr = ics02_client::client_lc_address(env, client_id.clone())
        .unwrap_or_else(|| panic_with_error!(env, Error::ClientIdNotFound));
    let ok: bool = env.invoke_contract(
        &lc_addr,
        &Symbol::new(env, "verify_non_membership"),
        vec![
            env,
            client_id.into_val(env),
            proof_height.into_val(env),
            proof.into_val(env),
            path.into_val(env),
        ],
    );
    if !ok {
        panic_with_error!(env, Error::NonMembershipVerificationFailed);
    }
}

fn counterparty_packet_commitment_path(
    env: &Env,
    prefix: &Vec<Bytes>,
    source_client_id: &String,
    sequence: u64,
) -> Bytes {
    counterparty_v2_path(env, prefix, source_client_id, 0x01, sequence)
}

fn counterparty_packet_receipt_path(
    env: &Env,
    prefix: &Vec<Bytes>,
    dest_client_id: &String,
    sequence: u64,
) -> Bytes {
    counterparty_v2_path(env, prefix, dest_client_id, 0x02, sequence)
}

fn counterparty_ack_commitment_path(
    env: &Env,
    prefix: &Vec<Bytes>,
    dest_client_id: &String,
    sequence: u64,
) -> Bytes {
    counterparty_v2_path(env, prefix, dest_client_id, 0x03, sequence)
}

fn counterparty_v2_path(
    env: &Env,
    prefix: &Vec<Bytes>,
    client_id: &String,
    discriminator: u8,
    sequence: u64,
) -> Bytes {
    let mut out = Bytes::new(env);
    for chunk in prefix.iter() {
        out.append(&chunk);
    }
    let id_len = client_id.len() as usize;
    let mut buf = [0u8; 128];
    client_id.copy_into_slice(&mut buf[..id_len]);
    buf[id_len] = discriminator;
    buf[id_len + 1..id_len + 9].copy_from_slice(&sequence.to_be_bytes());
    out.append(&Bytes::from_slice(env, &buf[..id_len + 9]));
    out
}

