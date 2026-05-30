use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use soroban_client::xdr::{
    ContractDataDurability, ContractId, Hash, LedgerEntryData, LedgerKey, LedgerKeyContractData,
    Limits, ReadXdr, ScAddress, ScString, ScSymbol, ScVal, ScVec, StringM, VecM, WriteXdr,
};

use crate::AppState;

const DEFAULT_CLIENT_TYPES: &[&str] = &["07-tendermint", "mock", "attestation", "08-wasm"];

#[derive(Deserialize)]
pub struct ListClientsQuery {
    pub client_type: Option<String>,
}

fn err<E: std::fmt::Display>(status: StatusCode, e: E) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": e.to_string() })))
}

fn next_client_id_ledger_key(router: [u8; 32], client_type: &str) -> anyhow::Result<Vec<u8>> {
    let variant: StringM<32> = "NextClientId".try_into()?;
    let type_str: StringM = client_type.try_into()?;
    let key_val = ScVal::Vec(Some(ScVec(VecM::try_from(vec![
        ScVal::Symbol(ScSymbol(variant)),
        ScVal::String(ScString(type_str)),
    ])?)));
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract: ScAddress::Contract(ContractId(Hash(router))),
        key: key_val,
        durability: ContractDataDurability::Persistent,
    });
    Ok(key.to_xdr(Limits::none())?)
}

fn decode_counter(entry_xdr: &[u8]) -> Option<u32> {
    match LedgerEntryData::from_xdr(entry_xdr, Limits::none()).ok()? {
        LedgerEntryData::ContractData(d) => match d.val {
            ScVal::U32(n) => Some(n),
            _ => None,
        },
        _ => None,
    }
}

#[tracing::instrument(skip(state, params))]
pub async fn list_clients(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListClientsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::info!("GET /stellar/clients");

    if state.ibc_contract_id.is_empty() {
        return Err(err(StatusCode::BAD_GATEWAY, "IBC_CONTRACT_ID not configured"));
    }
    let router = stellar_strkey::Contract::from_string(&state.ibc_contract_id)
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("invalid IBC_CONTRACT_ID: {e}")))?
        .0;

    let types: Vec<String> = match &params.client_type {
        Some(t) => vec![t.clone()],
        None => DEFAULT_CLIENT_TYPES.iter().map(|s| s.to_string()).collect(),
    };

    let mut clients = Vec::new();
    for client_type in &types {
        let key = next_client_id_ledger_key(router, client_type)
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
