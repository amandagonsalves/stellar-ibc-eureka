use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::state::AppState;

#[tracing::instrument(skip(state))]
pub async fn get_latest_ledger(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    tracing::info!("GET /ledger/latest");

    match state.rpc.get_latest_ledger().await {
        Ok(sequence) => {
            tracing::info!(sequence, "latest ledger sequence");

            let latest_ledger = get_ledger(State(state), Path(sequence))
                .await
                .into_response();

            latest_ledger
        }
        Err(error) => {
            tracing::error!(%error, "get_latest_ledger failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response()
        }
    }
}

#[tracing::instrument(skip(state))]
pub async fn get_ledger(
    State(state): State<Arc<AppState>>,
    Path(sequence): Path<u32>,
) -> impl IntoResponse {
    tracing::info!(sequence, "GET /ledger/{sequence}");

    match state.rpc.get_ledger(sequence).await {
        Ok(ledger) => {
            let body = json!({
                "sequence": ledger.sequence,
                "header_xdr": hex::encode(&ledger.header_xdr),
                "metadata_xdr": ledger.metadata_xdr.as_deref().map(hex::encode),
            });

            tracing::info!(
                sequence = ledger.sequence,
                header_bytes = ledger.header_xdr.len(),
                metadata_bytes = ledger.metadata_xdr.as_ref().map(|m| m.len()).unwrap_or(0),
                "ledger details"
            );

            (StatusCode::OK, Json(body)).into_response()
        }
        Err(error) => {
            tracing::error!(%error, sequence, "get_ledger failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response()
        }
    }
}
