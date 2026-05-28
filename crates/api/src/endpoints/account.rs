use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
struct AccountResponse {
    account_id: String,
}

pub async fn account(
    State(_state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(AccountResponse {
            account_id: address,
        }),
    )
}
