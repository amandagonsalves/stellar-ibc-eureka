use std::sync::Arc;

mod types;

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use types::{GetSignedTxResponse, GetUnsignedTxResponse, SignTxResponse, SubmitSignedTxResponse};

pub async fn submit_signed_tx(
    State(_state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(SubmitSignedTxResponse {
            account_id: address,
        }),
    )
}

pub async fn sign_tx(
    State(_state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(SignTxResponse {
            account_id: address,
        }),
    )
}

pub async fn get_unsigned_tx(
    State(_state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(GetUnsignedTxResponse {
            account_id: address,
        }),
    )
}

pub async fn get_signed_tx(
    State(_state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(GetSignedTxResponse {
            account_id: address,
        }),
    )
}
