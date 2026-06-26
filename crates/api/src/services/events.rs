use std::sync::Arc;

use crate::rpc::{EventCursor, EventsPage};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use utoipa::IntoParams;

use crate::state::AppState;

#[derive(Deserialize, Debug, Default, IntoParams)]
pub struct EventsQuery {
    pub contract_id: Option<String>,
    pub cursor: Option<String>,
    pub start_ledger: Option<u32>,
    pub limit: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/events",
    tag = "Stellar",
    params(EventsQuery),
    responses(
        (status = 200, description = "Event page: { latest_ledger, cursor, events }"),
        (status = 400, description = "Missing contract_id, or neither cursor nor start_ledger"),
        (status = 502, description = "Soroban RPC unreachable"),
    )
)]
#[tracing::instrument(skip(state))]
pub async fn get_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventsQuery>,
) -> impl IntoResponse {
    let contract_id = match params.contract_id.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => id.to_owned(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "missing 'contract_id' query parameter" })),
            )
                .into_response();
        }
    };

    let cursor = match (params.cursor.as_ref(), params.start_ledger) {
        (Some(c), _) if !c.is_empty() => EventCursor::Cursor(c.clone()),
        (_, Some(s)) if s > 0 => EventCursor::StartLedger(s),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "must provide either non-empty 'cursor' or 'start_ledger' > 0"
                })),
            )
                .into_response();
        }
    };

    let page: EventsPage = match state
        .rpc
        .get_events(&contract_id, cursor, params.limit)
        .await
    {
        Ok(page) => page,
        Err(error) => {
            tracing::error!(%error, %contract_id, "get_events failed");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    tracing::debug!(
        events = page.events.len(),
        latest_ledger = page.latest_ledger,
        contract_id = ?params.contract_id,
        "get events"
    );

    let events: Vec<Value> = page
        .events
        .iter()
        .map(|ev| {
            json!({
                "id":              ev.id,
                "ledger":          ev.ledger,
                "ledger_closed_at": ev.ledger_closed_at,
                "contract_id":     ev.contract_id,
                "tx_hash":         ev.tx_hash,
                "topics_xdr":      ev.topics_xdr.iter().map(hex::encode).collect::<Vec<_>>(),
                "value_xdr":       hex::encode(&ev.value_xdr),
            })
        })
        .collect();

    let body = json!({
        "latest_ledger": page.latest_ledger,
        "cursor":        page.cursor,
        "events":        events,
    });

    (StatusCode::OK, Json(body)).into_response()
}
