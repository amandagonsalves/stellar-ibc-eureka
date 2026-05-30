use cosmwasm_std::Storage;
use prost::Message;

use crate::types::{Any, ClientState, ConsensusState, WasmClientState, WasmConsensusState};

const CLIENT_STATE_KEY: &[u8] = b"clientState";
const CHECKSUM_KEY: &[u8] = b"wasmChecksum";
const STELLAR_REVISION_NUMBER: u64 = 0;
const WASM_CLIENT_STATE_TYPE_URL: &str = "/ibc.lightclients.wasm.v1.ClientState";
const WASM_CONSENSUS_STATE_TYPE_URL: &str = "/ibc.lightclients.wasm.v1.ConsensusState";

fn consensus_state_key(height: u64) -> alloc::vec::Vec<u8> {
    alloc::format!("consensusStates/{STELLAR_REVISION_NUMBER}-{height}").into_bytes()
}

pub fn set_checksum(storage: &mut dyn Storage, checksum: &[u8]) {
    if !checksum.is_empty() {
        storage.set(CHECKSUM_KEY, checksum);
    }
}

fn checksum(storage: &dyn Storage) -> alloc::vec::Vec<u8> {
    storage.get(CHECKSUM_KEY).unwrap_or_default()
}

pub fn save_client_state(storage: &mut dyn Storage, client_state: &ClientState) {
    let wasm = WasmClientState {
        data: client_state.encode_to_vec(),
        checksum: checksum(storage),
        latest_height: client_state.latest_height.clone(),
    };
    let any = Any {
        type_url: WASM_CLIENT_STATE_TYPE_URL.into(),
        value: wasm.encode_to_vec(),
    };
    storage.set(CLIENT_STATE_KEY, &any.encode_to_vec());
}

pub fn load_client_state(storage: &dyn Storage) -> Option<ClientState> {
    let raw = storage.get(CLIENT_STATE_KEY)?;
    let any = Any::decode(raw.as_slice()).ok()?;
    let wasm = WasmClientState::decode(any.value.as_slice()).ok()?;
    ClientState::decode(wasm.data.as_slice()).ok()
}

pub fn save_consensus_state(
    storage: &mut dyn Storage,
    height: u64,
    consensus_state: &ConsensusState,
) {
    let wasm = WasmConsensusState {
        data: consensus_state.encode_to_vec(),
    };
    let any = Any {
        type_url: WASM_CONSENSUS_STATE_TYPE_URL.into(),
        value: wasm.encode_to_vec(),
    };
    storage.set(&consensus_state_key(height), &any.encode_to_vec());
}

pub fn load_consensus_state(storage: &dyn Storage, height: u64) -> Option<ConsensusState> {
    let raw = storage.get(&consensus_state_key(height))?;
    let any = Any::decode(raw.as_slice()).ok()?;
    let wasm = WasmConsensusState::decode(any.value.as_slice()).ok()?;
    ConsensusState::decode(wasm.data.as_slice()).ok()
}

extern crate alloc;
