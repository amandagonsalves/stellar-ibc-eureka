use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use stellar_ibc_core::rpc::RpcClient;
use tokio::net::TcpListener;

use crate::{config::ApiConfig, services, AppState};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ledger/latest", get(services::ledgers::get_latest_ledger))
        .route("/ledger/{sequence}", get(services::ledgers::get_ledger))
        .route("/events", get(services::events::get_events))
        .route("/account/{address}", get(services::account::account))
        .route("/balance/{address}", get(services::balance::balance))
        .route("/tx", get(services::tx::get_unsigned_tx))
        .route("/tx/{tx_hash}", get(services::tx::get_signed_tx))
        .route("/tx/sign", post(services::tx::sign_tx))
        .route("/tx/submit", post(services::tx::submit_signed_tx))
        .with_state(state)
}

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

    tracing::info!(%addr, "stellar-api HTTP server listening");

    axum::serve(listener, router(state)).await?;

    Ok(())
}

pub async fn run(cfg: ApiConfig) -> anyhow::Result<()> {
    tracing::info!(
        host = %cfg.host,
        port = cfg.port,
        rpc_url = %cfg.rpc_url,
        signing_key_configured = !cfg.signing_key.is_empty(),
        "starting stellar-api"
    );

    let rpc = RpcClient::new(cfg.rpc_url.as_str()).expect("could not create a new rpc client");

    let state = Arc::new(AppState::new(rpc, cfg.signing_key.clone()));

    serve(cfg.addr(), state).await
}
