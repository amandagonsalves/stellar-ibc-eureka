use cosmwasm_std::{Binary, DepsMut, Empty, Env, MessageInfo, Response};
use cw_multi_test::{App, AppResponse, Contract, ContractWrapper, Executor, WasmSudo};
use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use sha2::{Digest, Sha256};

use light_client_wasm::entrypoint::{instantiate, query, sudo};
use light_client_wasm::error::ContractError;
use light_client_wasm::merkle::{
    CommitmentProof, ExistenceProof, InnerOp, LeafOp, MerkleProof, NonExistenceProof, Proof,
};
use light_client_wasm::msg::{
    ClientStatus, Height as MsgHeight, InstantiateMsg, LatestHeightResult, MerklePath, QueryMsg,
    StatusResult, SudoMsg, TimestampAtHeightResult, UpdateStateMsg, UpdateStateOnMisbehaviourMsg,
    UpdateStateResult, VerifyMembershipMsg, VerifyNonMembershipMsg,
};
use light_client_wasm::smt::{fold_siblings, key_index, leaf_hash, sha256, HASH_SIZE, TREE_DEPTH};
use light_client_wasm::types::{
    ClientState, ConsensusState, Height as WireHeight, ScpEnvelope, StellarHeader,
};

const CHAIN_ID: &str = "stellar-testnet";
const ROOT_INIT: [u8; 32] = [0x11; 32];
const LEDGER_HASH_INIT: [u8; 32] = [0xaa; 32];
const LEDGER_HASH_NEXT: [u8; 32] = [0xbb; 32];
const NETWORK_ID: [u8; 32] = [0x33; 32];
const VALIDATOR_SEED: [u8; 32] = [0x07; 32];
const STATEMENT_BYTES: &[u8] = b"sample-scp-statement-xdr";
const STELLAR_CLIENT_ID: &str = "10-stellar-0";

const PACKET_COMMITMENT_DISCRIMINATOR: u8 = 0x01;
const PACKET_RECEIPT_DISCRIMINATOR: u8 = 0x02;
const ACK_COMMITMENT_DISCRIMINATOR: u8 = 0x03;
const COMMITMENT_VERSION_PREFIX: u8 = 0x02;

fn execute_stub(
    _deps: DepsMut<'_>,
    _env: Env,
    _info: MessageInfo,
    _msg: Empty,
) -> Result<Response, ContractError> {
    Err(ContractError::NotInitialised)
}

fn light_client() -> Box<dyn Contract<Empty>> {
    let wrapper = ContractWrapper::new(execute_stub, instantiate, query).with_sudo(sudo);

    Box::new(wrapper)
}

fn trusted_signing_key() -> SigningKey {
    SigningKey::from_bytes(&VALIDATOR_SEED)
}

fn sha256v(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn signed_envelope(key: &SigningKey, statement: &[u8]) -> ScpEnvelope {
    let mut preimage = Vec::with_capacity(32 + 4 + statement.len());
    preimage.extend_from_slice(&NETWORK_ID);
    preimage.extend_from_slice(&[0, 0, 0, 4]);
    preimage.extend_from_slice(statement);
    let digest = sha256v(&preimage);
    let signature = key.sign(&digest);

    ScpEnvelope {
        node_id: key.verifying_key().to_bytes().to_vec(),
        statement_xdr: statement.to_vec(),
        signature: signature.to_bytes().to_vec(),
    }
}

fn fresh_client_state(latest_height: u64) -> ClientState {
    let pubkey = trusted_signing_key().verifying_key().to_bytes().to_vec();

    ClientState {
        chain_id: CHAIN_ID.to_string(),
        latest_height: Some(WireHeight {
            revision_number: 0,
            revision_height: latest_height,
        }),
        frozen_height: None,
        trusted_validators: vec![pubkey],
        proof_specs: vec![],
        network_id: NETWORK_ID.to_vec(),
    }
}

fn signed_header(
    trusted_height: u64,
    target_height: u64,
    ts: u64,
    root: [u8; 32],
) -> StellarHeader {
    StellarHeader {
        ledger_seq: target_height,
        ledger_header_xdr: vec![],
        ibc_state_root: root.to_vec(),
        scp_envelopes: vec![signed_envelope(&trusted_signing_key(), STATEMENT_BYTES)],
        trusted_height: Some(WireHeight {
            revision_number: 0,
            revision_height: trusted_height,
        }),
        timestamp: ts,
        ledger_hash: LEDGER_HASH_NEXT.to_vec(),
        previous_ledger_hash: LEDGER_HASH_INIT.to_vec(),
    }
}

fn encode<T: Message>(m: &T) -> Binary {
    Binary::new(m.encode_to_vec())
}

fn store_and_instantiate(app: &mut App) -> cosmwasm_std::Addr {
    let code_id = app.store_code(light_client());
    let admin = app.api().addr_make("admin");
    let cs = fresh_client_state(100);
    let cons = ConsensusState {
        timestamp: 1_000_000,
        ledger_hash: LEDGER_HASH_INIT.to_vec(),
        root: ROOT_INIT.to_vec(),
    };
    let msg = InstantiateMsg {
        client_state: encode(&cs),
        consensus_state: encode(&cons),
        checksum: Binary::default(),
    };

    app.instantiate_contract(code_id, admin, &msg, &[], "stellar-light-client", None)
        .expect("instantiate")
}

fn wasm_sudo(app: &mut App, addr: &cosmwasm_std::Addr, msg: &SudoMsg) -> AppResponse {
    app.sudo(WasmSudo::new(addr, msg).unwrap().into())
        .expect("sudo")
}

fn try_wasm_sudo(
    app: &mut App,
    addr: &cosmwasm_std::Addr,
    msg: &SudoMsg,
) -> Result<AppResponse, String> {
    app.sudo(WasmSudo::new(addr, msg).unwrap().into())
        .map_err(|e| format!("{e:#}"))
}

fn update_to(app: &mut App, addr: &cosmwasm_std::Addr, trusted: u64, target: u64, root: [u8; 32]) {
    let hdr = signed_header(trusted, target, 1_000_000 + target, root);
    let resp = wasm_sudo(
        app,
        addr,
        &SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    );
    let result: UpdateStateResult = serde_json::from_slice(resp.data.unwrap().as_slice()).unwrap();
    assert_eq!(result.heights[0].revision_height, target);
}

fn membership_proof(key: &[u8], value: &[u8], sibling_byte: u8) -> ([u8; 32], Binary) {
    let siblings: Vec<[u8; HASH_SIZE]> = (0..TREE_DEPTH)
        .map(|i| [sibling_byte.wrapping_add(i as u8); HASH_SIZE])
        .collect();
    let leaf = leaf_hash(sha256(key), sha256(value));
    let root = fold_siblings(key_index(key), leaf, &siblings);
    let path = inner_ops(key, &siblings);
    let bytes = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Exist(ExistenceProof {
                key: key.to_vec(),
                value: sha256(value).to_vec(),
                leaf: Some(LeafOp::default()),
                path,
            })),
        }],
    }
    .encode_to_vec();

    (root, Binary::new(bytes))
}

fn non_membership_proof(key: &[u8], sibling_byte: u8) -> ([u8; 32], Binary) {
    let siblings: Vec<[u8; HASH_SIZE]> = (0..TREE_DEPTH)
        .map(|i| [sibling_byte.wrapping_add(i as u8); HASH_SIZE])
        .collect();
    let root = fold_siblings(key_index(key), [0u8; HASH_SIZE], &siblings);
    let path = inner_ops(key, &siblings);
    let bytes = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Nonexist(NonExistenceProof {
                key: key.to_vec(),
                left: Some(ExistenceProof {
                    key: key.to_vec(),
                    value: Vec::new(),
                    leaf: Some(LeafOp::default()),
                    path,
                }),
                right: None,
            })),
        }],
    }
    .encode_to_vec();

    (root, Binary::new(bytes))
}

fn inner_ops(key: &[u8], siblings: &[[u8; HASH_SIZE]]) -> Vec<InnerOp> {
    let mut path = Vec::with_capacity(TREE_DEPTH);
    let mut sub_idx = key_index(key);
    for sibling in siblings {
        let is_left_child = sub_idx & 1 == 0;
        path.push(if is_left_child {
            InnerOp {
                hash: 1,
                prefix: vec![0x01],
                suffix: sibling.to_vec(),
            }
        } else {
            let mut prefix = Vec::with_capacity(1 + HASH_SIZE);
            prefix.push(0x01);
            prefix.extend_from_slice(sibling);
            InnerOp {
                hash: 1,
                prefix,
                suffix: Vec::new(),
            }
        });
        sub_idx >>= 1;
    }

    path
}

fn v2_path_key(client_id: &str, discriminator: u8, sequence: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(client_id.len() + 9);
    key.extend_from_slice(client_id.as_bytes());
    key.push(discriminator);
    key.extend_from_slice(&sequence.to_be_bytes());

    key
}

fn commit_payload(
    source_port: &str,
    dest_port: &str,
    version: &str,
    encoding: &str,
    value: &[u8],
) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&sha256v(source_port.as_bytes()));
    buf.extend_from_slice(&sha256v(dest_port.as_bytes()));
    buf.extend_from_slice(&sha256v(version.as_bytes()));
    buf.extend_from_slice(&sha256v(encoding.as_bytes()));
    buf.extend_from_slice(&sha256v(value));

    sha256v(&buf)
}

fn commit_v2_packet(dest_client: &str, timeout: u64, payload_hashes: &[[u8; 32]]) -> [u8; 32] {
    let mut concat = Vec::new();
    for h in payload_hashes {
        concat.extend_from_slice(h);
    }
    let mut preimage = Vec::new();
    preimage.push(COMMITMENT_VERSION_PREFIX);
    preimage.extend_from_slice(&sha256v(dest_client.as_bytes()));
    preimage.extend_from_slice(&sha256v(&timeout.to_be_bytes()));
    preimage.extend_from_slice(&sha256v(&concat));

    sha256v(&preimage)
}

fn commit_v2_acknowledgement(acks: &[&[u8]]) -> [u8; 32] {
    let mut concat = Vec::new();
    for ack in acks {
        concat.extend_from_slice(&sha256v(ack));
    }
    let mut preimage = Vec::new();
    preimage.push(COMMITMENT_VERSION_PREFIX);
    preimage.extend_from_slice(&concat);

    sha256v(&preimage)
}

fn ics20_transfer_value() -> Vec<u8> {
    serde_json::json!({
        "token": { "denom": "native", "amount": "1000000" },
        "sender": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF5",
        "receiver": "cosmos1xyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzabcd",
        "memo": "ics20 stellar transfer"
    })
    .to_string()
    .into_bytes()
}

#[test]
fn app_instantiate_exposes_height_status_and_timestamp() {
    let mut app = App::default();
    let addr = store_and_instantiate(&mut app);

    let latest: LatestHeightResult = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::LatestHeight {})
        .unwrap();
    assert_eq!(latest.height.revision_height, 100);

    let status: StatusResult = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::Status {})
        .unwrap();
    assert_eq!(status.status, ClientStatus::Active);

    let ts: TimestampAtHeightResult = app
        .wrap()
        .query_wasm_smart(
            addr,
            &QueryMsg::TimestampAtHeight {
                height: MsgHeight {
                    revision_number: 0,
                    revision_height: 100,
                },
            },
        )
        .unwrap();
    assert_eq!(ts.timestamp, 1_000_000);
}

#[test]
fn app_update_state_advances_latest_height() {
    let mut app = App::default();
    let addr = store_and_instantiate(&mut app);

    update_to(&mut app, &addr, 100, 105, [0x22; 32]);

    let latest: LatestHeightResult = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::LatestHeight {})
        .unwrap();
    assert_eq!(latest.height.revision_height, 105);
}

#[test]
fn ics20_recv_verifies_packet_commitment_membership() {
    let mut app = App::default();
    let addr = store_and_instantiate(&mut app);

    let sequence = 7u64;
    let timeout = 1_900_000_000u64;
    let dest_client = "07-tendermint-0";
    let value = ics20_transfer_value();
    let payload_hash = commit_payload(
        "transfer",
        "transfer",
        "ics20-1",
        "application/json",
        &value,
    );
    let commitment = commit_v2_packet(dest_client, timeout, &[payload_hash]);

    let key = v2_path_key(STELLAR_CLIENT_ID, PACKET_COMMITMENT_DISCRIMINATOR, sequence);
    let (root, proof) = membership_proof(&key, &commitment, 0x40);

    update_to(&mut app, &addr, 100, 105, root);

    wasm_sudo(
        &mut app,
        &addr,
        &SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 105,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof,
            merkle_path: MerklePath {
                key_path: vec![Binary::new(key)],
            },
            value: Binary::new(commitment.to_vec()),
        }),
    );
}

#[test]
fn ics20_recv_rejects_tampered_packet_commitment() {
    let mut app = App::default();
    let addr = store_and_instantiate(&mut app);

    let sequence = 7u64;
    let value = ics20_transfer_value();
    let payload_hash = commit_payload(
        "transfer",
        "transfer",
        "ics20-1",
        "application/json",
        &value,
    );
    let commitment = commit_v2_packet("07-tendermint-0", 1_900_000_000, &[payload_hash]);

    let key = v2_path_key(STELLAR_CLIENT_ID, PACKET_COMMITMENT_DISCRIMINATOR, sequence);
    let (root, proof) = membership_proof(&key, &commitment, 0x40);

    update_to(&mut app, &addr, 100, 105, root);

    let mut tampered = commitment;
    tampered[0] ^= 0xff;

    let err = try_wasm_sudo(
        &mut app,
        &addr,
        &SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 105,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof,
            merkle_path: MerklePath {
                key_path: vec![Binary::new(key)],
            },
            value: Binary::new(tampered.to_vec()),
        }),
    )
    .unwrap_err();
    assert!(err.contains("membership mismatch"), "{err}");
}

#[test]
fn ics20_ack_verifies_acknowledgement_commitment_membership() {
    let mut app = App::default();
    let addr = store_and_instantiate(&mut app);

    let sequence = 7u64;
    let success_ack = b"\x01";
    let commitment = commit_v2_acknowledgement(&[success_ack]);

    let key = v2_path_key(STELLAR_CLIENT_ID, ACK_COMMITMENT_DISCRIMINATOR, sequence);
    let (root, proof) = membership_proof(&key, &commitment, 0x60);

    update_to(&mut app, &addr, 100, 106, root);

    wasm_sudo(
        &mut app,
        &addr,
        &SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 106,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof,
            merkle_path: MerklePath {
                key_path: vec![Binary::new(key)],
            },
            value: Binary::new(commitment.to_vec()),
        }),
    );
}

#[test]
fn ics20_timeout_verifies_receipt_non_membership() {
    let mut app = App::default();
    let addr = store_and_instantiate(&mut app);

    let sequence = 7u64;
    let key = v2_path_key(STELLAR_CLIENT_ID, PACKET_RECEIPT_DISCRIMINATOR, sequence);
    let (root, proof) = non_membership_proof(&key, 0x80);

    update_to(&mut app, &addr, 100, 107, root);

    wasm_sudo(
        &mut app,
        &addr,
        &SudoMsg::VerifyNonMembership(VerifyNonMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 107,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof,
            merkle_path: MerklePath {
                key_path: vec![Binary::new(key)],
            },
        }),
    );
}

#[test]
fn frozen_client_rejects_transfer_membership_proofs() {
    let mut app = App::default();
    let addr = store_and_instantiate(&mut app);

    wasm_sudo(
        &mut app,
        &addr,
        &SudoMsg::UpdateStateOnMisbehaviour(UpdateStateOnMisbehaviourMsg {
            client_message: Binary::default(),
        }),
    );

    let status: StatusResult = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::Status {})
        .unwrap();
    assert_eq!(status.status, ClientStatus::Frozen);

    let key = v2_path_key(STELLAR_CLIENT_ID, PACKET_COMMITMENT_DISCRIMINATOR, 1);
    let err = try_wasm_sudo(
        &mut app,
        &addr,
        &SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 100,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath {
                key_path: vec![Binary::new(key)],
            },
            value: Binary::default(),
        }),
    )
    .unwrap_err();
    assert!(err.contains("frozen"), "{err}");
}

#[test]
fn two_clients_in_one_app_are_isolated() {
    let mut app = App::default();
    let first = store_and_instantiate(&mut app);
    let second = store_and_instantiate(&mut app);

    update_to(&mut app, &first, 100, 120, [0x22; 32]);

    let first_height: LatestHeightResult = app
        .wrap()
        .query_wasm_smart(first, &QueryMsg::LatestHeight {})
        .unwrap();
    assert_eq!(first_height.height.revision_height, 120);

    let second_height: LatestHeightResult = app
        .wrap()
        .query_wasm_smart(second, &QueryMsg::LatestHeight {})
        .unwrap();
    assert_eq!(second_height.height.revision_height, 100);
}
