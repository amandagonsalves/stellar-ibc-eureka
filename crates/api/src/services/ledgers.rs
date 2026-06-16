use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/ledger/latest",
    tag = "Ledger",
    responses(
        (status = 200, description = "Latest ledger: { sequence, header_xdr, metadata_xdr }"),
        (status = 502, description = "Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_latest_ledger(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    tracing::debug!("GET /ledger/latest");

    match state.rpc.get_latest_ledger().await {
        Ok(sequence) => {
            tracing::debug!(sequence, "latest ledger sequence");

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

#[utoipa::path(
    get,
    path = "/ledger/{sequence}",
    tag = "Ledger",
    params(
        ("sequence" = u32, Path, description = "Ledger sequence number"),
    ),
    responses(
        (status = 200, description = "Ledger: { sequence, header_xdr, metadata_xdr }"),
        (status = 502, description = "Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_ledger(
    State(state): State<Arc<AppState>>,
    Path(sequence): Path<u32>,
) -> impl IntoResponse {
    tracing::debug!(sequence, "GET /ledger/{sequence}");

    match state.rpc.get_ledger(sequence).await {
        Ok(ledger) => {
            let body = json!({
                "sequence": ledger.sequence,
                "header_xdr": hex::encode(&ledger.header_xdr),
                "metadata_xdr": ledger.metadata_xdr.as_deref().map(hex::encode),
            });

            tracing::debug!(
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
