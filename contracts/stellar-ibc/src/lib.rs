#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Vec};

mod errors;
mod ibc_store;
mod ics02_client;
mod ics24_host;
mod identifiers;
mod packet_handler;
mod port_router;
mod types;

pub use errors::Error;
pub use types::{
    OnAcknowledgementPacketCallback, OnRecvPacketCallback, OnTimeoutPacketCallback,
};

pub use types::{Counterparty, DataKey, Height, Packet, Payload};

#[contract]
pub struct IbcRouter;

#[contractimpl]
impl IbcRouter {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn register_client_type(env: &Env, client_type: String, lc_address: Address) {
        ics02_client::register_client_type(env, client_type, lc_address)
    }

    pub fn lc_address(env: &Env, client_type: String) -> Option<Address> {
        ics02_client::lc_address(env, client_type)
    }

    pub fn create_client(
        env: &Env,
        client_type: String,
        client_state: Bytes,
        consensus_state: Bytes,
        height: u64,
    ) -> String {
        ics02_client::create_client(env, client_type, client_state, consensus_state, height)
    }

    pub fn register_counterparty(
        env: &Env,
        client_id: String,
        counterparty_client_id: String,
        counterparty_commitment_prefix: Vec<Bytes>,
    ) {
        ics02_client::register_counterparty(
            env,
            client_id,
            counterparty_client_id,
            counterparty_commitment_prefix,
        )
    }

    pub fn counterparty(env: &Env, client_id: String) -> Option<Counterparty> {
        ics02_client::counterparty(env, client_id)
    }

    pub fn update_client(env: &Env, client_id: String, client_message: Bytes) -> u64 {
        ics02_client::update_client(env, client_id, client_message)
    }

    pub fn client_lc_address(env: &Env, client_id: String) -> Option<Address> {
        ics02_client::client_lc_address(env, client_id)
    }

    pub fn frozen(env: &Env, client_id: String) -> bool {
        ics02_client::frozen(env, client_id)
    }

    pub fn register_port(env: &Env, port_id: String, app_address: Address) {
        port_router::register_port(env, port_id, app_address)
    }

    pub fn port_app(env: &Env, port_id: String) -> Option<Address> {
        port_router::port_app(env, port_id)
    }

    pub fn set_packet_commitment(
        env: &Env,
        source_client_id: String,
        sequence: u64,
        commitment: BytesN<32>,
    ) {
        ibc_store::set_packet_commitment(env, &source_client_id, sequence, &commitment)
    }

    pub fn packet_commitment(
        env: &Env,
        source_client_id: String,
        sequence: u64,
    ) -> Option<BytesN<32>> {
        ibc_store::packet_commitment(env, &source_client_id, sequence)
    }

    pub fn delete_packet_commitment(env: &Env, source_client_id: String, sequence: u64) {
        ibc_store::delete_packet_commitment(env, &source_client_id, sequence)
    }

    pub fn set_packet_receipt(env: &Env, dest_client_id: String, sequence: u64) {
        ibc_store::set_packet_receipt(env, &dest_client_id, sequence)
    }

    pub fn has_packet_receipt(env: &Env, dest_client_id: String, sequence: u64) -> bool {
        ibc_store::has_packet_receipt(env, &dest_client_id, sequence)
    }

    pub fn set_ack_commitment(
        env: &Env,
        dest_client_id: String,
        sequence: u64,
        ack_hash: BytesN<32>,
    ) {
        ibc_store::set_ack_commitment(env, &dest_client_id, sequence, &ack_hash)
    }

    pub fn acknowledgement(env: &Env, dest_client_id: String, sequence: u64) -> Option<BytesN<32>> {
        ibc_store::acknowledgement(env, &dest_client_id, sequence)
    }

    pub fn send_packet(
        env: &Env,
        source_client_id: String,
        timeout_timestamp: u64,
        payloads: Vec<Payload>,
    ) -> u64 {
        packet_handler::send_packet(env, source_client_id, timeout_timestamp, payloads)
    }

    pub fn recv_packet(env: &Env, packet: Packet, proof: Bytes, proof_height: u64) {
        packet_handler::recv_packet(env, packet, proof, proof_height)
    }

    pub fn write_acknowledgement(
        env: &Env,
        dest_client_id: String,
        sequence: u64,
        acknowledgements: Vec<Bytes>,
    ) {
        packet_handler::write_acknowledgement(env, dest_client_id, sequence, acknowledgements)
    }

    pub fn acknowledge_packet(
        env: &Env,
        packet: Packet,
        acknowledgements: Vec<Bytes>,
        proof: Bytes,
        proof_height: u64,
    ) {
        packet_handler::acknowledge_packet(env, packet, acknowledgements, proof, proof_height)
    }

    pub fn timeout_packet(env: &Env, packet: Packet, proof: Bytes, proof_height: u64) {
        packet_handler::timeout_packet(env, packet, proof, proof_height)
    }
}

mod test;
