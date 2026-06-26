use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::rpc::RpcClient;
use crate::{config::ApiConfig, services, AppState};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "stellar-api",
        description = "HTTP service that fronts the Stellar IBC stack.",
        version = "0.1.0",
    ),
    tags(
        (name = "Stellar", description = "Health + Stellar IBC reads (ledgers, events, balances, clients)."),
        (name = "Ledger", description = "Stellar ledger headers + metadata."),
        (name = "Tx", description = "Build unsigned and submit relayer-signed Soroban transactions."),
    ),
    paths(
        health,
        services::ledgers::get_latest_ledger,
        services::ledgers::get_ledger,
        services::events::get_events,
        services::balance::transfer_balance,
        services::clients::list_clients,
        services::clients::client_state,
        services::clients::consensus_state,
        services::tx::prepare_tx,
        services::tx::submit_signed_tx,
    ),
    components(schemas(
        services::balance::BalanceResponse,
        services::tx::PrepareRequest,
        services::tx::PrepareResponse,
        services::tx::SubmitSignedTxRequest,
        services::tx::SubmitSignedTxResponse,
    )),
)]
pub struct ApiDoc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ledger/latest", get(services::ledgers::get_latest_ledger))
        .route("/ledger/{sequence}", get(services::ledgers::get_ledger))
        .route("/events", get(services::events::get_events))
        .route(
            "/stellar/transfer/balance/{denom}/{address}",
            get(services::balance::transfer_balance),
        )
        .route("/stellar/clients", get(services::clients::list_clients))
        .route(
            "/stellar/clients/{client_id}/state",
            get(services::clients::client_state),
        )
        .route(
            "/stellar/clients/{client_id}/consensus/{height}",
            get(services::clients::consensus_state),
        )
        .route("/tx/submit", post(services::tx::submit_signed_tx))
        .route("/tx/prepare", post(services::tx::prepare_tx))
        .with_state(state)
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "Stellar",
    responses(
        (status = 200, description = "Liveness probe with the latest Stellar ledger (plain text)"),
    )
)]
async fn health(State(state): State<Arc<AppState>>) -> String {
    tracing::debug!("GET /health");

    match state.rpc.get_latest_ledger().await {
        Ok(sequence) => {
            format!("Stellar IBC API is healthy. Latest ledger: {sequence}")
        }
        Err(error) => {
            tracing::warn!(%error, "/health: latest_ledger probe failed");
            format!("Stellar IBC API is healthy. Latest ledger: unavailable ({error})")
        }
    }
}

pub async fn serve(addr: SocketAddr, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "[api] listening");

    axum::serve(listener, router(state)).await?;

    Ok(())
}

pub async fn run(cfg: ApiConfig) -> anyhow::Result<()> {
    tracing::info!(
        port = cfg.port,
        rpc_url = %cfg.rpc_url,
        "[api] starting"
    );

    let addr = cfg.addr();

    let rpc = RpcClient::new(cfg.rpc_url.as_str()).expect("could not create a new rpc client");

    let state = Arc::new(AppState::new(
        rpc,
        cfg.ibc_contract_id,
        cfg.transfer_contract_id,
        cfg.network_passphrase,
    ));

    serve(addr, state).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_documents_every_routed_endpoint() {
        let spec = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let paths = spec["paths"].as_object().expect("openapi has paths");

        for path in [
            "/health",
            "/ledger/latest",
            "/ledger/{sequence}",
            "/events",
            "/stellar/transfer/balance/{denom}/{address}",
            "/stellar/clients",
            "/stellar/clients/{client_id}/state",
            "/stellar/clients/{client_id}/consensus/{height}",
            "/tx/prepare",
            "/tx/submit",
        ] {
            assert!(
                paths.contains_key(path),
                "endpoint missing from OpenAPI: {path}"
            );
        }
    }
}
