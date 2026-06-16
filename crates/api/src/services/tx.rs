use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use soroban_client::xdr::{Limits, WriteXdr};
use utoipa::ToSchema;

use crate::AppState;

const BASE_FEE: u32 = 1_000;

#[derive(Deserialize, ToSchema)]
pub struct PrepareRequest {
    /// Stellar address (G…) used as the tx source account; falls back to the
    /// api's configured account when empty.
    #[serde(default)]
    pub signer: String,
    /// Router contract method to invoke.
    pub method: String,
    /// Hex-encoded ScVal XDR arguments, in order.
    #[serde(default)]
    pub args_xdr: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PrepareResponse {
    /// Hex-encoded unsigned transaction envelope XDR.
    pub tx_xdr: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SubmitSignedTxRequest {
    /// Hex-encoded signed transaction envelope XDR.
    pub tx_xdr: String,
}

#[derive(Serialize, ToSchema)]
pub struct SubmitSignedTxResponse {
    /// Submitted transaction hash.
    pub hash: String,
    /// Hex-encoded ScVal XDR return value (empty when none).
    pub return_value_xdr: String,
}

fn err<E: std::fmt::Display>(status: StatusCode, e: E) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": e.to_string() })))
}

#[utoipa::path(
    post,
    path = "/tx/prepare",
    tag = "Tx",
    request_body = PrepareRequest,
    responses(
        (status = 200, description = "Unsigned tx built for the relayer to sign", body = PrepareResponse),
        (status = 400, description = "Malformed args_xdr hex"),
        (status = 502, description = "ROUTER_CONTRACT_ADDRESS unset or Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state, req), fields(method = %req.method))]
pub async fn prepare_tx(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PrepareRequest>,
) -> Result<Json<PrepareResponse>, (StatusCode, Json<Value>)> {
    tracing::debug!("POST /tx/prepare");

    if state.ibc_contract_id.is_empty() {
        return Err(err(
            StatusCode::BAD_GATEWAY,
            "ROUTER_CONTRACT_ADDRESS not configured",
        ));
    }

    let mut args_xdr = Vec::with_capacity(req.args_xdr.len());
    for (i, a) in req.args_xdr.iter().enumerate() {
        let bytes = hex::decode(a)
            .map_err(|e| err(StatusCode::BAD_REQUEST, format!("args_xdr[{i}] hex: {e}")))?;
        args_xdr.push(bytes);
    }

    let tx_xdr = state
        .rpc
        .build_unsigned_tx(
            &req.signer,
            &state.ibc_contract_id,
            &req.method,
            &args_xdr,
            &state.network_passphrase,
            BASE_FEE,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "build_unsigned_tx failed");
            err(StatusCode::BAD_GATEWAY, e)
        })?;

    Ok(Json(PrepareResponse {
        tx_xdr: hex::encode(tx_xdr),
    }))
}

#[utoipa::path(
    post,
    path = "/tx/submit",
    tag = "Tx",
    request_body = SubmitSignedTxRequest,
    responses(
        (status = 200, description = "Tx submitted; returns hash + return value", body = SubmitSignedTxResponse),
        (status = 400, description = "Malformed tx_xdr hex"),
        (status = 502, description = "Soroban RPC submission failed"),
    )
)]
#[tracing::instrument(skip(state, req), fields(tx_bytes = req.tx_xdr.len()))]
pub async fn submit_signed_tx(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitSignedTxRequest>,
) -> Result<Json<SubmitSignedTxResponse>, (StatusCode, Json<Value>)> {
    tracing::debug!("POST /tx/submit");

    let tx_xdr = hex::decode(&req.tx_xdr)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("tx_xdr hex: {e}")))?;

    let submitted = state.rpc.submit_and_wait(&tx_xdr).await.map_err(|e| {
        tracing::error!(error = %e, "submit_and_wait failed");
        err(StatusCode::BAD_GATEWAY, e)
    })?;

    let return_value_xdr = match submitted.return_value {
        Some(value) => value.to_xdr(Limits::none()).map(hex::encode).map_err(|e| {
            err(
                StatusCode::BAD_GATEWAY,
                format!("return_value XDR encode: {e}"),
            )
        })?,
        None => String::new(),
    };

    tracing::info!(hash = %submitted.hash, "[api] tx submitted to soroban");

    Ok(Json(SubmitSignedTxResponse {
        hash: submitted.hash,
        return_value_xdr,
    }))
}
