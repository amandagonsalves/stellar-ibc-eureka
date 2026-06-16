use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use stellar_ibc_core::conversion as cv;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct BalanceResponse {
    balance: String,
}

fn err<E: std::fmt::Display>(status: StatusCode, e: E) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": e.to_string() })))
}

fn balance_ledger_key(
    contract: [u8; 32],
    address_hex: &str,
    denom: &str,
) -> anyhow::Result<Vec<u8>> {
    let addr_xdr = hex::decode(address_hex)?;
    let addr_val = cv::scval_from_xdr(&addr_xdr)?;
    let key_val = cv::scval_vec(vec![
        cv::scval_symbol("Balance")?,
        addr_val,
        cv::scval_string(denom)?,
    ])?;
    cv::persistent_contract_data_key(contract, key_val)
}

fn decode_i128(entry_xdr: &[u8]) -> Option<i128> {
    cv::ledger_entry_contract_val(entry_xdr).and_then(|v| cv::scval_as_i128(&v))
}

#[utoipa::path(
    get,
    path = "/stellar/transfer/balance/{denom}/{address}",
    tag = "Stellar",
    params(
        ("denom" = String, Path, description = "Token denom"),
        ("address" = String, Path, description = "Hex-encoded sender address ScVal"),
    ),
    responses(
        (status = 200, description = "Escrowed transfer balance", body = BalanceResponse),
        (status = 400, description = "Malformed address or denom"),
        (status = 502, description = "TRANSFER_CONTRACT_ADDRESS unset or Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state))]
pub async fn transfer_balance(
    State(state): State<Arc<AppState>>,
    Path((denom, address)): Path<(String, String)>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<Value>)> {
    tracing::debug!(%denom, %address, "GET /stellar/transfer/balance");

    if state.transfer_contract_id.is_empty() {
        return Err(err(
            StatusCode::BAD_GATEWAY,
            "TRANSFER_CONTRACT_ADDRESS not configured",
        ));
    }

    let contract = stellar_strkey::Contract::from_string(state.transfer_contract_id.as_str())
        .map_err(|e| {
            err(
                StatusCode::BAD_GATEWAY,
                format!("transfer contract addr: {e}"),
            )
        })?
        .0;

    let key = balance_ledger_key(contract, &address, &denom)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    let entry = state
        .rpc
        .get_ledger_entry(&key)
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let balance = entry.and_then(|x| decode_i128(&x)).unwrap_or(0);

    Ok(Json(BalanceResponse {
        balance: balance.to_string(),
    }))
}
