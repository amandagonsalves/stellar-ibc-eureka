use cosmwasm_std::{
    entry_point, to_json_binary, Api, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
};
use prost::Message;
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::merkle::{decode_membership_proof, decode_non_membership_proof};
use crate::msg::{
    CheckForMisbehaviourMsg, CheckForMisbehaviourResult, ClientStatus, Height as MsgHeight,
    InstantiateMsg, LatestHeightResult, QueryMsg, StatusResult, SudoMsg, TimestampAtHeightResult,
    UpdateStateMsg, UpdateStateOnMisbehaviourMsg, UpdateStateResult, VerifyMembershipMsg,
    VerifyNonMembershipMsg,
};
use crate::smt::{
    fold_siblings, key_index, leaf_hash, sha256, verify_non_membership_raw, HASH_SIZE, TREE_DEPTH,
};
use crate::store;
use crate::types::{ClientState, ConsensusState, Height as WireHeight, ScpEnvelope, StellarHeader};

const ENVELOPE_TYPE_SCPVALUE: [u8; 4] = [0, 0, 0, 4];

#[entry_point]
pub fn instantiate(
    deps: DepsMut<'_>,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if store::client_state(deps.storage).is_some() {
        return Err(ContractError::AlreadyInitialised);
    }

    let client_state = ClientState::decode(msg.client_state.as_slice())
        .map_err(|e| ContractError::InvalidWire(format!("client_state: {e}")))?;
    let consensus_state = ConsensusState::decode(msg.consensus_state.as_slice())
        .map_err(|e| ContractError::InvalidWire(format!("consensus_state: {e}")))?;

    let height = client_state
        .latest_height
        .as_ref()
        .ok_or_else(|| ContractError::InvalidWire("client_state.latest_height".into()))?
        .revision_height;

    store::set_checksum(deps.storage, msg.checksum.as_slice());
    store::set_client_state(deps.storage, &client_state);
    store::set_consensus_state(deps.storage, height, &consensus_state);

    Ok(Response::default())
}

#[entry_point]
pub fn sudo(deps: DepsMut<'_>, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let data = match msg {
        SudoMsg::UpdateState(m) => to_json(&update_state(deps, env, m)?)?,
        SudoMsg::UpdateStateOnMisbehaviour(m) => {
            update_state_on_misbehaviour(deps, env, m)?;
            Binary::default()
        }
        SudoMsg::CheckForMisbehaviour(m) => to_json(&check_for_misbehaviour(deps, env, m)?)?,
        SudoMsg::VerifyMembership(m) => {
            verify_membership(deps, env, m)?;
            Binary::default()
        }
        SudoMsg::VerifyNonMembership(m) => {
            verify_non_membership(deps, env, m)?;
            Binary::default()
        }
    };
    Ok(Response::default().set_data(data))
}

#[entry_point]
pub fn query(deps: Deps<'_>, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::ClientState {} => {
            let cs = require_client_state(deps)?;
            Ok(Binary::new(cs.encode_to_vec()))
        }
        QueryMsg::ConsensusState { height } => {
            let cons = require_consensus_state(deps, height.revision_height)?;
            Ok(Binary::new(cons.encode_to_vec()))
        }
        QueryMsg::LatestHeight {} => {
            let cs = require_client_state(deps)?;
            let h = cs.latest_height.unwrap_or_default();
            to_json(&LatestHeightResult {
                height: MsgHeight {
                    revision_number: h.revision_number,
                    revision_height: h.revision_height,
                },
            })
        }
        QueryMsg::Status {} => {
            let cs = require_client_state(deps)?;
            let status = if cs.frozen_height.is_some() {
                ClientStatus::Frozen
            } else {
                ClientStatus::Active
            };
            to_json(&StatusResult { status })
        }
        QueryMsg::TimestampAtHeight { height } => {
            let cons = require_consensus_state(deps, height.revision_height)?;
            to_json(&TimestampAtHeightResult {
                timestamp: cons.timestamp,
            })
        }
        QueryMsg::VerifyClientMessage { client_message } => {
            verify_client_message(deps, client_message.as_slice())?;
            to_json(&cosmwasm_std::Empty {})
        }
        QueryMsg::CheckForMisbehaviour { client_message: _ } => {
            to_json(&CheckForMisbehaviourResult {
                found_misbehaviour: false,
            })
        }
    }
}

fn verify_client_message(
    deps: Deps<'_>,
    client_message: &[u8],
) -> Result<StellarHeader, ContractError> {
    let cs = require_client_state(deps)?;
    if let Some(h) = cs.frozen_height.as_ref() {
        return Err(ContractError::Frozen {
            height: h.revision_height,
        });
    }

    let header = decode_header(client_message)?;
    let trusted_height = header
        .trusted_height
        .as_ref()
        .ok_or_else(|| ContractError::InvalidWire("header.trusted_height".into()))?
        .revision_height;
    if header.ledger_seq <= trusted_height {
        return Err(ContractError::NonAdvancingHeight {
            trusted: trusted_height,
            target: header.ledger_seq,
        });
    }

    let _trusted_consensus = require_consensus_state(deps, trusted_height)?;

    verify_scp_quorum(deps.api, &cs, &header.scp_envelopes)?;

    Ok(header)
}

fn update_state(
    deps: DepsMut<'_>,
    _env: Env,
    msg: UpdateStateMsg,
) -> Result<UpdateStateResult, ContractError> {
    let header = verify_client_message(deps.as_ref(), &msg.client_message)?;
    let mut cs = require_client_state_mut(deps.as_ref())?;

    let new_consensus = ConsensusState {
        timestamp: header.timestamp,
        ledger_hash: header.ledger_hash.clone(),
        root: header.ibc_state_root.clone(),
    };

    if let Some(existing) = store::consensus_state(deps.storage, header.ledger_seq) {
        if existing != new_consensus {
            return Err(ContractError::ConsensusStateConflict {
                height: header.ledger_seq,
            });
        }
    }

    store::set_consensus_state(deps.storage, header.ledger_seq, &new_consensus);

    if header.ledger_seq
        > cs.latest_height
            .as_ref()
            .map(|h| h.revision_height)
            .unwrap_or(0)
    {
        cs.latest_height = Some(WireHeight {
            revision_number: 0,
            revision_height: header.ledger_seq,
        });
        store::set_client_state(deps.storage, &cs);
    }

    Ok(UpdateStateResult {
        heights: vec![MsgHeight {
            revision_number: 0,
            revision_height: header.ledger_seq,
        }],
    })
}

fn update_state_on_misbehaviour(
    deps: DepsMut<'_>,
    _env: Env,
    _msg: UpdateStateOnMisbehaviourMsg,
) -> Result<(), ContractError> {
    let mut cs = require_client_state_mut(deps.as_ref())?;
    let latest = cs.latest_height.clone().unwrap_or_default();
    cs.frozen_height = Some(latest);
    store::set_client_state(deps.storage, &cs);
    Ok(())
}

fn check_for_misbehaviour(
    deps: DepsMut<'_>,
    _env: Env,
    msg: CheckForMisbehaviourMsg,
) -> Result<CheckForMisbehaviourResult, ContractError> {
    let cs = require_client_state_mut(deps.as_ref())?;
    if cs.frozen_height.is_some() {
        return Ok(CheckForMisbehaviourResult {
            found_misbehaviour: false,
        });
    }
    let header = decode_header(&msg.client_message)?;
    if let Some(existing) = store::consensus_state(deps.storage, header.ledger_seq) {
        let header_consensus = ConsensusState {
            timestamp: header.timestamp,
            ledger_hash: header.ledger_hash.clone(),
            root: header.ibc_state_root.clone(),
        };
        return Ok(CheckForMisbehaviourResult {
            found_misbehaviour: existing != header_consensus,
        });
    }
    Ok(CheckForMisbehaviourResult {
        found_misbehaviour: false,
    })
}

fn verify_membership(
    deps: DepsMut<'_>,
    _env: Env,
    msg: VerifyMembershipMsg,
) -> Result<(), ContractError> {
    let cs = require_client_state_mut(deps.as_ref())?;
    if let Some(h) = cs.frozen_height.as_ref() {
        return Err(ContractError::Frozen {
            height: h.revision_height,
        });
    }
    let consensus = require_consensus_state(deps.as_ref(), msg.height.revision_height)?;

    let root: [u8; HASH_SIZE] = consensus
        .root
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::MerkleVerificationFailed)?;

    let key = concat_path(&msg.merkle_path.key_path);
    let (proof_key, proof_value, siblings) = decode_membership_proof(msg.proof.as_slice())?;

    let key_match = proof_key == key;
    let value_match = proof_value.as_slice() == sha256(msg.value.as_slice()).as_slice();
    let computed_root = if siblings.len() == TREE_DEPTH && !msg.value.is_empty() {
        let leaf = leaf_hash(sha256(&key), sha256(msg.value.as_slice()));
        Some(fold_siblings(key_index(&key), leaf, &siblings))
    } else {
        None
    };
    let root_match = computed_root.map(|r| r == root).unwrap_or(false);

    if key_match && value_match && root_match {
        return Ok(());
    }

    Err(ContractError::MembershipMismatch {
        key_match,
        value_match,
        siblings: siblings.len(),
        height: msg.height.revision_height,
        req_key: hex_encode(&key),
        proof_key: hex_encode(&proof_key),
        value_len: msg.value.len(),
        proof_value_len: proof_value.len(),
        stored_root: hex_encode(&root),
        computed_root: computed_root.map(|r| hex_encode(&r)).unwrap_or_default(),
    })
}

fn verify_non_membership(
    deps: DepsMut<'_>,
    _env: Env,
    msg: VerifyNonMembershipMsg,
) -> Result<(), ContractError> {
    let cs = require_client_state_mut(deps.as_ref())?;
    if let Some(h) = cs.frozen_height.as_ref() {
        return Err(ContractError::Frozen {
            height: h.revision_height,
        });
    }
    let consensus = require_consensus_state(deps.as_ref(), msg.height.revision_height)?;

    let root: [u8; HASH_SIZE] = consensus
        .root
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::MerkleVerificationFailed)?;

    let key = concat_path(&msg.merkle_path.key_path);
    let (proof_key, siblings) = decode_non_membership_proof(msg.proof.as_slice())?;
    if proof_key != key {
        return Err(ContractError::MerkleVerificationFailed);
    }
    if !verify_non_membership_raw(&root, &key, &siblings) {
        return Err(ContractError::MerkleVerificationFailed);
    }
    Ok(())
}

fn concat_path(path: &[cosmwasm_std::Binary]) -> Vec<u8> {
    let total: usize = path.iter().map(|b| b.len()).sum();
    let mut out = Vec::with_capacity(total);
    for chunk in path {
        out.extend_from_slice(chunk.as_slice());
    }
    out
}

fn decode_header(bytes: &[u8]) -> Result<StellarHeader, ContractError> {
    StellarHeader::decode(bytes).map_err(|e| ContractError::InvalidWire(format!("header: {e}")))
}

fn require_client_state(deps: Deps<'_>) -> Result<ClientState, ContractError> {
    store::client_state(deps.storage).ok_or(ContractError::NotInitialised)
}

fn require_client_state_mut(deps: Deps<'_>) -> Result<ClientState, ContractError> {
    require_client_state(deps)
}

fn require_consensus_state(deps: Deps<'_>, height: u64) -> Result<ConsensusState, ContractError> {
    store::consensus_state(deps.storage, height)
        .ok_or(ContractError::ConsensusStateMissing { height })
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<Binary, ContractError> {
    to_json_binary(value).map_err(|e| ContractError::Std(StdError::generic_err(e.to_string())))
}

fn verify_scp_quorum(
    api: &dyn Api,
    client_state: &ClientState,
    envelopes: &[ScpEnvelope],
) -> Result<(), ContractError> {
    if client_state.network_id.is_empty() {
        return Err(ContractError::NetworkIdMissing);
    }
    if client_state.network_id.len() != 32 {
        return Err(ContractError::ScpSignatureError(format!(
            "network_id must be 32 bytes, got {}",
            client_state.network_id.len()
        )));
    }

    let mut matched = 0usize;
    let mut last_raw_ok = false;
    let mut last_hash_ok = false;
    let mut last_statement_len = 0usize;
    for env in envelopes {
        if env.node_id.len() != 32 || env.signature.len() != 64 {
            continue;
        }
        if !client_state
            .trusted_validators
            .iter()
            .any(|v| v.as_slice() == env.node_id.as_slice())
        {
            continue;
        }
        matched += 1;

        if env.statement_xdr.is_empty() {
            continue;
        }
        let mut preimage =
            Vec::with_capacity(32 + ENVELOPE_TYPE_SCPVALUE.len() + env.statement_xdr.len());
        preimage.extend_from_slice(&client_state.network_id);
        preimage.extend_from_slice(&ENVELOPE_TYPE_SCPVALUE);
        preimage.extend_from_slice(&env.statement_xdr);
        let digest: [u8; 32] = Sha256::digest(&preimage).into();

        let raw_ok = api
            .ed25519_verify(&preimage, env.signature.as_slice(), env.node_id.as_slice())
            .unwrap_or(false);
        if raw_ok {
            return Ok(());
        }
        let hash_ok = api
            .ed25519_verify(&digest, env.signature.as_slice(), env.node_id.as_slice())
            .unwrap_or(false);
        if hash_ok {
            return Ok(());
        }

        last_raw_ok = raw_ok;
        last_hash_ok = hash_ok;
        last_statement_len = env.statement_xdr.len();
    }

    Err(ContractError::QuorumNotMet {
        envelopes: envelopes.len(),
        matched,
        raw_ok: last_raw_ok,
        hash_ok: last_hash_ok,
        signer: envelopes
            .first()
            .map(|e| hex_encode(&e.node_id))
            .unwrap_or_default(),
        network_id: hex_encode(&client_state.network_id),
        statement_len: last_statement_len,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
