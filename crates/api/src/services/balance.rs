use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use stellar_ibc_core::conversion as cv;

use crate::state::AppState;

#[derive(Serialize)]
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

#[tracing::instrument(skip(_state))]
pub async fn balance(
    State(_state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(%address, "GET /balance/{address}");
    (
        StatusCode::OK,
        Json(BalanceResponse {
            balance: "0".to_string(),
        }),
    )
}

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
