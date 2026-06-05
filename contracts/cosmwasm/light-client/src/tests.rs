use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
use cosmwasm_std::Binary;
use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use sha2::{Digest, Sha256};

use crate::entrypoint::{instantiate, query, sudo};
use crate::error::ContractError;
use crate::msg::{
    CheckForMisbehaviourMsg, CheckForMisbehaviourResult, ClientStatus, Height as MsgHeight,
    InstantiateMsg, LatestHeightResult, MerklePath, QueryMsg, StatusResult, SudoMsg,
    TimestampAtHeightResult, UpdateStateMsg, UpdateStateOnMisbehaviourMsg, UpdateStateResult,
    VerifyMembershipMsg, VerifyNonMembershipMsg,
};
use crate::types::{ClientState, ConsensusState, Height as WireHeight, ScpEnvelope, StellarHeader};

const CHAIN_ID: &str = "stellar-testnet";
const ROOT_INIT: [u8; 32] = [0x11; 32];
const ROOT_NEXT: [u8; 32] = [0x22; 32];
const LEDGER_HASH_INIT: [u8; 32] = [0xaa; 32];
const LEDGER_HASH_NEXT: [u8; 32] = [0xbb; 32];
const NETWORK_ID: [u8; 32] = [0x33; 32];
const VALIDATOR_SEED: [u8; 32] = [0x07; 32];
const STATEMENT_BYTES: &[u8] = b"sample-scp-statement-xdr";

fn trusted_signing_key() -> SigningKey {
    SigningKey::from_bytes(&VALIDATOR_SEED)
}

fn signed_envelope(key: &SigningKey, statement: &[u8]) -> ScpEnvelope {
    let mut preimage = Vec::with_capacity(32 + 4 + statement.len());
    preimage.extend_from_slice(&NETWORK_ID);
    preimage.extend_from_slice(&[0, 0, 0, 4]);
    preimage.extend_from_slice(statement);
    let digest: [u8; 32] = Sha256::digest(&preimage).into();
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

fn fresh_consensus_state(ts: u64, ledger_hash: [u8; 32], root: [u8; 32]) -> ConsensusState {
    ConsensusState {
        timestamp: ts,
        ledger_hash: ledger_hash.to_vec(),
        root: root.to_vec(),
    }
}

fn encode<T: Message>(m: &T) -> Binary {
    Binary::new(m.encode_to_vec())
}

fn header(
    trusted_height: u64,
    target_height: u64,
    ts: u64,
    previous_ledger_hash: [u8; 32],
    ledger_hash: [u8; 32],
    root: [u8; 32],
) -> StellarHeader {
    header_with_envelopes(
        trusted_height,
        target_height,
        ts,
        previous_ledger_hash,
        ledger_hash,
        root,
        vec![signed_envelope(&trusted_signing_key(), STATEMENT_BYTES)],
    )
}

fn header_with_envelopes(
    trusted_height: u64,
    target_height: u64,
    ts: u64,
    previous_ledger_hash: [u8; 32],
    ledger_hash: [u8; 32],
    root: [u8; 32],
    scp_envelopes: Vec<ScpEnvelope>,
) -> StellarHeader {
    StellarHeader {
        ledger_seq: target_height,
        ledger_header_xdr: vec![],
        ibc_state_root: root.to_vec(),
        scp_envelopes,
        trusted_height: Some(WireHeight {
            revision_number: 0,
            revision_height: trusted_height,
        }),
        timestamp: ts,
        ledger_hash: ledger_hash.to_vec(),
        previous_ledger_hash: previous_ledger_hash.to_vec(),
    }
}

fn do_instantiate(
    deps: &mut cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    >,
) {
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let cs = fresh_client_state(100);
    let cons = fresh_consensus_state(1_000_000, LEDGER_HASH_INIT, ROOT_INIT);
    let msg = InstantiateMsg {
        client_state: encode(&cs),
        consensus_state: encode(&cons),
        checksum: Binary::default(),
    };
    instantiate(deps.as_mut(), env, info, msg).expect("instantiate");
}

#[test]
fn instantiate_stores_state_and_consensus() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let latest: LatestHeightResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::LatestHeight {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    assert_eq!(latest.height.revision_height, 100);

    let status: StatusResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::Status {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    assert_eq!(status.status, ClientStatus::Active);

    let ts: TimestampAtHeightResult = serde_json::from_slice(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::TimestampAtHeight {
                height: MsgHeight {
                    revision_number: 0,
                    revision_height: 100,
                },
            },
        )
        .unwrap()
        .as_slice(),
    )
    .unwrap();
    assert_eq!(ts.timestamp, 1_000_000);
}

#[test]
fn instantiate_rejects_double_instantiation() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let cs = fresh_client_state(100);
    let cons = fresh_consensus_state(1_000_000, LEDGER_HASH_INIT, ROOT_INIT);
    let msg = InstantiateMsg {
        client_state: encode(&cs),
        consensus_state: encode(&cons),
        checksum: Binary::default(),
    };
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(err, ContractError::AlreadyInitialised));
}

#[test]
fn update_state_advances_height_when_chain_intact() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let hdr = header(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
    );
    let msg = SudoMsg::UpdateState(UpdateStateMsg {
        client_message: encode(&hdr),
    });
    let resp = sudo(deps.as_mut(), mock_env(), msg).expect("update_state");
    let result: UpdateStateResult = serde_json::from_slice(resp.data.unwrap().as_slice()).unwrap();
    assert_eq!(result.heights.len(), 1);
    assert_eq!(result.heights[0].revision_height, 105);

    let latest: LatestHeightResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::LatestHeight {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    assert_eq!(latest.height.revision_height, 105);
}

#[test]
fn update_state_rejects_non_advancing_height() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let hdr = header(
        100,
        100,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
    );
    let msg = SudoMsg::UpdateState(UpdateStateMsg {
        client_message: encode(&hdr),
    });
    let err = sudo(deps.as_mut(), mock_env(), msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::NonAdvancingHeight {
            trusted: 100,
            target: 100
        }
    ));
}

#[test]
fn check_for_misbehaviour_detects_conflicting_root_at_same_height() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let hdr1 = header(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
    );
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr1),
        }),
    )
    .unwrap();

    let conflicting_root = [0xCC; 32];
    let hdr2 = header(
        100,
        105,
        1_000_700,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        conflicting_root,
    );
    let resp = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::CheckForMisbehaviour(CheckForMisbehaviourMsg {
            client_message: encode(&hdr2),
        }),
    )
    .unwrap();
    let result: CheckForMisbehaviourResult =
        serde_json::from_slice(resp.data.unwrap().as_slice()).unwrap();
    assert!(result.found_misbehaviour);
}

#[test]
fn update_state_on_misbehaviour_freezes_client() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateStateOnMisbehaviour(UpdateStateOnMisbehaviourMsg {
            client_message: Binary::default(),
        }),
    )
    .unwrap();

    let status: StatusResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::Status {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    assert_eq!(status.status, ClientStatus::Frozen);
}

#[test]
fn update_state_rejects_when_frozen() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateStateOnMisbehaviour(UpdateStateOnMisbehaviourMsg {
            client_message: Binary::default(),
        }),
    )
    .unwrap();

    let hdr = header(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
    );
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::Frozen { .. }));
}

#[test]
fn verify_membership_rejects_when_consensus_state_missing() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 999,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath { key_path: vec![] },
            value: Binary::default(),
        }),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ContractError::ConsensusStateMissing { height: 999 }
    ));
}

#[test]
fn verify_membership_rejects_when_proof_bytes_are_empty() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 100,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath { key_path: vec![] },
            value: Binary::default(),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::MerkleVerificationFailed));
}

#[test]
fn verify_membership_accepts_valid_proof_against_matching_root() {
    use crate::merkle::{CommitmentProof, ExistenceProof, InnerOp, LeafOp, MerkleProof, Proof};
    use crate::smt::{fold_siblings, key_index, leaf_hash, sha256, HASH_SIZE, TREE_DEPTH};

    let key = b"10-stellar-0\x01\x00\x00\x00\x00\x00\x00\x00\x07";
    let value = b"committed-bytes";

    let siblings: Vec<[u8; HASH_SIZE]> = (0..TREE_DEPTH)
        .map(|i| [0x40u8.wrapping_add(i as u8); HASH_SIZE])
        .collect();
    let leaf = leaf_hash(sha256(key), sha256(value));
    let root = fold_siblings(key_index(key), leaf, &siblings);

    let idx = key_index(key);
    let mut path_ops = Vec::with_capacity(TREE_DEPTH);
    let mut sub_idx = idx;
    for sibling in &siblings {
        let is_left_child = sub_idx & 1 == 0;
        path_ops.push(if is_left_child {
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
    let proof_bytes = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Exist(ExistenceProof {
                key: key.to_vec(),
                value: sha256(value).to_vec(),
                leaf: Some(LeafOp::default()),
                path: path_ops,
            })),
        }],
    }
    .encode_to_vec();

    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let cs = fresh_client_state(100);
    let cons = ConsensusState {
        timestamp: 1_000_000,
        ledger_hash: LEDGER_HASH_INIT.to_vec(),
        root: root.to_vec(),
    };
    instantiate(
        deps.as_mut(),
        env,
        info,
        InstantiateMsg {
            client_state: encode(&cs),
            consensus_state: encode(&cons),
            checksum: Binary::default(),
        },
    )
    .unwrap();

    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 100,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::new(proof_bytes),
            merkle_path: MerklePath { key_path: vec![Binary::new(key.to_vec())] },
            value: Binary::new(value.to_vec()),
        }),
    )
    .expect("verify_membership accepts a valid proof");
}

#[test]
fn verify_non_membership_rejects_when_frozen() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateStateOnMisbehaviour(UpdateStateOnMisbehaviourMsg {
            client_message: Binary::default(),
        }),
    )
    .unwrap();

    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyNonMembership(VerifyNonMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 100,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath { key_path: vec![] },
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::Frozen { .. }));
}

#[test]
fn update_state_rejects_envelope_signed_by_untrusted_validator() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let stranger = SigningKey::from_bytes(&[0xAA; 32]);
    let hdr = header_with_envelopes(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
        vec![signed_envelope(&stranger, STATEMENT_BYTES)],
    );
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::QuorumNotMet { .. }));
}

#[test]
fn update_state_rejects_when_signature_was_made_for_a_different_statement() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let mut env = signed_envelope(&trusted_signing_key(), STATEMENT_BYTES);
    env.statement_xdr = b"tampered-statement".to_vec();

    let hdr = header_with_envelopes(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
        vec![env],
    );
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::QuorumNotMet { .. }));
}

#[test]
fn update_state_accepts_when_at_least_one_envelope_is_trusted() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let stranger = SigningKey::from_bytes(&[0xBB; 32]);
    let envelopes = vec![
        signed_envelope(&stranger, STATEMENT_BYTES),
        signed_envelope(&trusted_signing_key(), STATEMENT_BYTES),
    ];
    let hdr = header_with_envelopes(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
        envelopes,
    );
    let resp = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .expect("at least one trusted+valid envelope must satisfy quorum");
    let result: UpdateStateResult = serde_json::from_slice(resp.data.unwrap().as_slice()).unwrap();
    assert_eq!(result.heights[0].revision_height, 105);
}

#[test]
fn update_state_rejects_when_network_id_is_unconfigured() {
    let mut deps = mock_dependencies();
    let env_mock = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    let mut cs = fresh_client_state(100);
    cs.network_id = Vec::new();
    let cons = fresh_consensus_state(1_000_000, LEDGER_HASH_INIT, ROOT_INIT);
    instantiate(
        deps.as_mut(),
        env_mock,
        info,
        InstantiateMsg {
            client_state: encode(&cs),
            consensus_state: encode(&cons),
            checksum: Binary::default(),
        },
    )
    .unwrap();

    let hdr = header(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
    );
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::NetworkIdMissing));
}

#[test]
fn update_state_rejects_when_envelopes_carry_empty_statement_xdr() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let key = trusted_signing_key();
    let env_with_empty_statement = ScpEnvelope {
        node_id: key.verifying_key().to_bytes().to_vec(),
        statement_xdr: Vec::new(),
        signature: vec![0u8; 64],
    };
    let hdr = header_with_envelopes(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
        vec![env_with_empty_statement],
    );
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::QuorumNotMet { .. }));
}

#[test]
fn update_state_rejects_envelope_signed_against_different_network() {
    let mut deps = mock_dependencies();
    do_instantiate(&mut deps);

    let other_network = [0x55u8; 32];
    let key = trusted_signing_key();
    let mut preimage = Vec::with_capacity(32 + 4 + STATEMENT_BYTES.len());
    preimage.extend_from_slice(&other_network);
    preimage.extend_from_slice(&[0, 0, 0, 4]);
    preimage.extend_from_slice(STATEMENT_BYTES);
    let digest: [u8; 32] = Sha256::digest(&preimage).into();
    let signature = key.sign(&digest);
    let env = ScpEnvelope {
        node_id: key.verifying_key().to_bytes().to_vec(),
        statement_xdr: STATEMENT_BYTES.to_vec(),
        signature: signature.to_bytes().to_vec(),
    };

    let hdr = header_with_envelopes(
        100,
        105,
        1_000_500,
        LEDGER_HASH_INIT,
        LEDGER_HASH_NEXT,
        ROOT_NEXT,
        vec![env],
    );
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::QuorumNotMet { .. }));
}
