use ibc::core::{
    client::types::Height,
    host::types::{
        identifiers::{ClientId, Sequence},
        path::{ClientConsensusStatePath, ClientStatePath},
    },
};

pub fn client_state_key(client_id: &ClientId) -> Vec<u8> {
    ClientStatePath(client_id.clone()).to_string().into_bytes()
}

pub fn consensus_state_key(client_id: &ClientId, height: &Height) -> Vec<u8> {
    ClientConsensusStatePath {
        client_id: client_id.clone(),
        revision_number: height.revision_number(),
        revision_height: height.revision_height(),
    }
    .to_string()
    .into_bytes()
}

pub fn packet_commitment_key(client_id: &ClientId, seq: Sequence) -> Vec<u8> {
    format!("commitments/{client_id}/{seq}").into_bytes()
}

pub fn packet_receipt_key(client_id: &ClientId, seq: Sequence) -> Vec<u8> {
    format!("receipts/{client_id}/{seq}").into_bytes()
}

pub fn next_seq_recv_key(client_id: &ClientId) -> Vec<u8> {
    format!("nextSeqRecv/{client_id}").into_bytes()
}

pub fn client_counter_key() -> Vec<u8> {
    b"clientCounter".to_vec()
}
