use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use soroban_client::xdr::{ContractId, Hash, ScAddress, ScVal};
use stellar_ibc_core::conversion as cv;
use utoipa::IntoParams;

use crate::AppState;

const DEFAULT_CLIENT_TYPES: &[&str] = &["07-tendermint", "mock", "attestation", "08-wasm"];

#[derive(Deserialize, IntoParams)]
pub struct ListClientsQuery {
    pub client_type: Option<String>,
}

fn err<E: std::fmt::Display>(status: StatusCode, e: E) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": e.to_string() })))
}

fn decode_counter(entry_xdr: &[u8]) -> Option<u32> {
    cv::ledger_entry_contract_val(entry_xdr).and_then(|v| cv::scval_as_u32(&v))
}

fn contract_data_key(contract: [u8; 32], variant: &str, arg: &str) -> anyhow::Result<Vec<u8>> {
    let key_val = cv::scval_vec(vec![cv::scval_symbol(variant)?, cv::scval_string(arg)?])?;
    cv::persistent_contract_data_key(contract, key_val)
}

fn consensus_data_key(contract: [u8; 32], client_id: &str, height: u64) -> anyhow::Result<Vec<u8>> {
    let key_val = cv::scval_vec(vec![
        cv::scval_symbol("Consensus")?,
        cv::scval_string(client_id)?,
        cv::scval_u64(height),
    ])?;
    cv::persistent_contract_data_key(contract, key_val)
}

fn client_type_of(client_id: &str) -> &str {
    match client_id.rfind('-') {
        Some(i) => &client_id[..i],
        None => client_id,
    }
}

#[utoipa::path(
    get,
    path = "/stellar/clients/{client_id}/state",
    tag = "Stellar",
    params(
        ("client_id" = String, Path, description = "IBC client id, e.g. 07-tendermint-0"),
    ),
    responses(
        (status = 200, description = "Client state: { client_id, client_type, client_state_xdr }"),
        (status = 404, description = "Client or its state not found"),
        (status = 502, description = "ROUTER_CONTRACT_ADDRESS unset or Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state))]
pub async fn client_state(
    State(state): State<Arc<AppState>>,
    Path(client_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("GET /stellar/clients/{client_id}/state");

    if state.ibc_contract_id.is_empty() {
        return Err(err(
            StatusCode::BAD_GATEWAY,
            "ROUTER_CONTRACT_ADDRESS not configured",
        ));
    }

    let router = stellar_strkey::Contract::from_string(&state.ibc_contract_id)
        .map_err(|e| {
            err(
                StatusCode::BAD_GATEWAY,
                format!("invalid ROUTER_CONTRACT_ADDRESS: {e}"),
            )
        })?
        .0;

    let lc_key = contract_data_key(router, "ClientLcAddr", &client_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let lc_entry = state
        .rpc
        .get_ledger_entry(&lc_key)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("get lc address: {e}")))?
        .ok_or_else(|| {
            err(
                StatusCode::NOT_FOUND,
                format!("client {client_id} not found"),
            )
        })?;

    let lc_contract = match cv::ledger_entry_contract_val(&lc_entry) {
        Some(ScVal::Address(ScAddress::Contract(ContractId(Hash(id))))) => id,
        _ => {
            return Err(err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "ClientLcAddr entry is not a contract address",
            ))
        }
    };

    let cs_key = contract_data_key(lc_contract, "Client", &client_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let cs_entry = state
        .rpc
        .get_ledger_entry(&cs_key)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("get client state: {e}")))?
        .ok_or_else(|| {
            err(
                StatusCode::NOT_FOUND,
                format!("client state for {client_id} not found"),
            )
        })?;

    let cs_val = cv::ledger_entry_contract_val(&cs_entry).ok_or_else(|| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "client state entry is not contract data",
        )
    })?;
    let cs_xdr = cv::scval_to_xdr(&cs_val)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("re-encode: {e}")))?;

    let hex: String = cs_xdr.iter().map(|b| format!("{b:02x}")).collect();

    Ok(Json(json!({
        "client_id": client_id,
        "client_type": client_type_of(&client_id),
        "client_state_xdr": hex,
    })))
}

#[utoipa::path(
    get,
    path = "/stellar/clients/{client_id}/consensus/{height}",
    tag = "Stellar",
    params(
        ("client_id" = String, Path, description = "IBC client id, e.g. 07-tendermint-0"),
        ("height" = u64, Path, description = "Consensus height (revision_height)"),
    ),
    responses(
        (status = 200, description = "Consensus state: { client_id, client_type, height, consensus_state_xdr }"),
        (status = 404, description = "Consensus state not found at that height"),
        (status = 502, description = "ROUTER_CONTRACT_ADDRESS unset or Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state))]
pub async fn consensus_state(
    State(state): State<Arc<AppState>>,
    Path((client_id, height)): Path<(String, u64)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("GET /stellar/clients/{client_id}/consensus/{height}");

    if state.ibc_contract_id.is_empty() {
        return Err(err(
            StatusCode::BAD_GATEWAY,
            "ROUTER_CONTRACT_ADDRESS not configured",
        ));
    }

    let router = stellar_strkey::Contract::from_string(&state.ibc_contract_id)
        .map_err(|e| {
            err(
                StatusCode::BAD_GATEWAY,
                format!("invalid ROUTER_CONTRACT_ADDRESS: {e}"),
            )
        })?
        .0;

    let lc_key = contract_data_key(router, "ClientLcAddr", &client_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let lc_entry = state
        .rpc
        .get_ledger_entry(&lc_key)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("get lc address: {e}")))?
        .ok_or_else(|| {
            err(
                StatusCode::NOT_FOUND,
                format!("client {client_id} not found"),
            )
        })?;

    let lc_contract = match cv::ledger_entry_contract_val(&lc_entry) {
        Some(ScVal::Address(ScAddress::Contract(ContractId(Hash(id))))) => id,
        _ => {
            return Err(err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "ClientLcAddr entry is not a contract address",
            ))
        }
    };

    let cons_key = consensus_data_key(lc_contract, &client_id, height)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let cons_entry = state
        .rpc
        .get_ledger_entry(&cons_key)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("get consensus state: {e}")))?
        .ok_or_else(|| {
            err(
                StatusCode::NOT_FOUND,
                format!("consensus state for {client_id} at height {height} not found"),
            )
        })?;

    let cons_val = cv::ledger_entry_contract_val(&cons_entry).ok_or_else(|| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "consensus state entry is not contract data",
        )
    })?;
    let cons_xdr = cv::scval_to_xdr(&cons_val)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("re-encode: {e}")))?;

    let hex: String = cons_xdr.iter().map(|b| format!("{b:02x}")).collect();

    Ok(Json(json!({
        "client_id": client_id,
        "client_type": client_type_of(&client_id),
        "height": height,
        "consensus_state_xdr": hex,
    })))
}

#[utoipa::path(
    get,
    path = "/stellar/clients",
    tag = "Stellar",
    params(ListClientsQuery),
    responses(
        (status = 200, description = "Clients grouped by type: { clients: [{ client_type, count, client_ids }] }"),
        (status = 502, description = "ROUTER_CONTRACT_ADDRESS unset or Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state, params))]
pub async fn list_clients(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListClientsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("GET /stellar/clients");

    if state.ibc_contract_id.is_empty() {
        return Err(err(
            StatusCode::BAD_GATEWAY,
            "ROUTER_CONTRACT_ADDRESS not configured",
        ));
    }
    let router = stellar_strkey::Contract::from_string(&state.ibc_contract_id)
        .map_err(|e| {
            err(
                StatusCode::BAD_GATEWAY,
                format!("invalid ROUTER_CONTRACT_ADDRESS: {e}"),
            )
        })?
        .0;

    let types: Vec<String> = match &params.client_type {
        Some(t) => vec![t.clone()],
        None => DEFAULT_CLIENT_TYPES.iter().map(|s| s.to_string()).collect(),
    };

    let mut clients = Vec::new();
    for client_type in &types {
        let key = contract_data_key(router, "NextClientId", client_type)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        let entry = state.rpc.get_ledger_entry(&key).await.map_err(|e| {
            err(
                StatusCode::BAD_GATEWAY,
                format!("get_ledger_entry({client_type}): {e}"),
            )
        })?;
        let count = entry.as_deref().and_then(decode_counter).unwrap_or(0);
        if count == 0 {
            continue;
        }
        let client_ids: Vec<String> = (0..count).map(|n| format!("{client_type}-{n}")).collect();
        clients.push(json!({
            "client_type": client_type,
            "count": count,
            "client_ids": client_ids,
        }));
    }

    Ok(Json(json!({ "clients": clients })))
}
