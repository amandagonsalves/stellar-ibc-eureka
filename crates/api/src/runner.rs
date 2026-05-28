use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use stellar_ibc_core::rpc::RpcClient;
use tokio::net::TcpListener;

use crate::{config::ApiConfig, endpoints, AppState};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/account/{address}", get(endpoints::account::account))
        .route("/balance/{address}", get(endpoints::balance::balance))
        .route("/tx/xdr", get(endpoints::tx::get_unsigned_tx))
        .route("/tx/{tx_hash}", get(endpoints::tx::get_signed_tx))
        .route("/tx/sign", post(endpoints::tx::sign_tx))
        .route("/tx/submit", post(endpoints::tx::submit_signed_tx))
        .with_state(state)
}

async fn health() -> &'static str {
    "Stellar IBC API is healthy."
}

pub async fn serve(addr: SocketAddr, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;

    tracing::info!("HTTP server listening on {}", addr);

    axum::serve(listener, router(state)).await?;

    Ok(())
}

pub async fn run(cfg: ApiConfig) -> anyhow::Result<()> {
    let rpc = RpcClient::new(cfg.rpc_url.as_str()).expect("could not create a new rpc client");

    let state = Arc::new(AppState::new(rpc, cfg.signing_key.clone()));

    serve(cfg.addr(), state).await
}
