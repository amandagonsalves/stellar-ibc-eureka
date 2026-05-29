pub mod client;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

use self::client::CosmosClient;

fn err<E: std::fmt::Display>(status: StatusCode, e: E) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": e.to_string() })))
}

fn bad_gateway<E: std::fmt::Display>(e: E) -> (StatusCode, Json<Value>) {
    err(StatusCode::BAD_GATEWAY, e)
}

fn bad_request<E: std::fmt::Display>(e: E) -> (StatusCode, Json<Value>) {
    err(StatusCode::BAD_REQUEST, e)
}

pub async fn node_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .cosmos
        .node_info()
        .await
        .map(Json)
        .map_err(bad_gateway)
}

#[derive(Deserialize)]
pub struct ProposalsQuery {
    pub status: Option<String>,
}

fn proposal_status_str(status: &str) -> String {
    let lower = status.to_ascii_lowercase();
    match lower.as_str() {
        "voting" | "voting_period" => "PROPOSAL_STATUS_VOTING_PERIOD".to_string(),
        "deposit" | "deposit_period" => "PROPOSAL_STATUS_DEPOSIT_PERIOD".to_string(),
        "passed" => "PROPOSAL_STATUS_PASSED".to_string(),
        "rejected" => "PROPOSAL_STATUS_REJECTED".to_string(),
        "failed" => "PROPOSAL_STATUS_FAILED".to_string(),
        _ if status.starts_with("PROPOSAL_STATUS_") => status.to_string(),
        _ => "PROPOSAL_STATUS_UNSPECIFIED".to_string(),
    }
}

pub async fn proposals(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ProposalsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let status = q
        .status
        .as_deref()
        .map(proposal_status_str)
        .unwrap_or_else(|| "PROPOSAL_STATUS_VOTING_PERIOD".to_string());
    state
        .cosmos
        .proposals_by_status(&status)
        .await
        .map(Json)
        .map_err(bad_gateway)
}

pub async fn proposal_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state.cosmos.proposal(id).await.map(Json).map_err(bad_gateway)
}

pub async fn gov_deposit_params(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .cosmos
        .gov_deposit_params()
        .await
        .map(Json)
        .map_err(bad_gateway)
}

pub async fn tx_by_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .cosmos
        .tx_by_hash(&hash)
        .await
        .map(Json)
        .map_err(|e| {
            if e.to_string().contains("not found") {
                err(StatusCode::NOT_FOUND, e)
            } else {
                bad_gateway(e)
            }
        })
}

pub async fn ibc_wasm_checksums(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .cosmos
        .ibc_wasm_checksums()
        .await
        .map(Json)
        .map_err(bad_gateway)
}

#[derive(Deserialize)]
pub struct StoreCodeRequest {
    pub wasm_base64: String,
    pub title: String,
    pub summary: String,
    pub deposit_amount: u128,
    pub gas_limit: u64,
    pub fee_amount: u128,
    #[serde(default)]
    pub wait_for_landing: bool,
    #[serde(default = "default_wait_secs")]
    pub wait_timeout_secs: u64,
}

fn default_wait_secs() -> u64 {
    30
}

pub async fn submit_store_code(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StoreCodeRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let wasm = BASE64
        .decode(req.wasm_base64.as_bytes())
        .map_err(|e| bad_request(format!("wasm_base64 not valid base64: {e}")))?;

    let result = state
        .cosmos
        .submit_store_code_proposal(
            wasm,
            req.title,
            req.summary,
            req.deposit_amount,
            req.gas_limit,
            req.fee_amount,
        )
        .await
        .map_err(bad_gateway)?;

    if result.code != 0 {
        return Err(err(
            StatusCode::BAD_GATEWAY,
            format!("broadcast rejected (code {}): {}", result.code, result.raw_log),
        ));
    }

    if !req.wait_for_landing {
        return Ok(Json(json!({
            "tx_hash": result.tx_hash,
            "code": result.code,
            "raw_log": result.raw_log,
        })));
    }

    let landed = state
        .cosmos
        .wait_for_tx(&result.tx_hash, Duration::from_secs(req.wait_timeout_secs))
        .await
        .map_err(bad_gateway)?;
    let proposal_id = CosmosClient::extract_proposal_id(&landed);

    Ok(Json(json!({
        "tx_hash": result.tx_hash,
        "code": result.code,
        "raw_log": result.raw_log,
        "tx_response": landed,
        "proposal_id": proposal_id,
    })))
}

#[derive(Deserialize)]
pub struct VoteRequest {
    pub proposal_id: u64,
    #[serde(default = "default_vote_option")]
    pub option: i32,
    pub gas_limit: u64,
    pub fee_amount: u128,
}

fn default_vote_option() -> i32 {
    1
}

pub async fn submit_vote(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VoteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = state
        .cosmos
        .submit_vote(req.proposal_id, req.option, req.gas_limit, req.fee_amount)
        .await
        .map_err(bad_gateway)?;

    if result.code != 0 {
        return Err(err(
            StatusCode::BAD_GATEWAY,
            format!("vote rejected (code {}): {}", result.code, result.raw_log),
        ));
    }

    Ok(Json(json!({
        "tx_hash": result.tx_hash,
        "code": result.code,
        "raw_log": result.raw_log,
    })))
}

pub async fn proposer_info(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Json(json!({
        "address": state.cosmos.proposer_address(),
    }))
}
