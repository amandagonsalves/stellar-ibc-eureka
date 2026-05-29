use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use stellar_ibc_core::rpc::RpcClient;
use tokio::net::TcpListener;

use crate::services::cosmos::client::CosmosClient;
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
        .route("/cosmos/node-info", get(services::cosmos::node_info))
        .route("/cosmos/proposer", get(services::cosmos::proposer_info))
        .route("/cosmos/gov/proposals", get(services::cosmos::proposals))
        .route(
            "/cosmos/gov/proposals/{id}",
            get(services::cosmos::proposal_by_id),
        )
        .route(
            "/cosmos/gov/params/deposit",
            get(services::cosmos::gov_deposit_params),
        )
        .route("/cosmos/tx/{hash}", get(services::cosmos::tx_by_hash))
        .route(
            "/cosmos/ibc-wasm/checksums",
            get(services::cosmos::ibc_wasm_checksums),
        )
        .route(
            "/cosmos/ibc-wasm/store-code",
            post(services::cosmos::submit_store_code),
        )
        .route("/cosmos/gov/vote", post(services::cosmos::submit_vote))
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
        cosmos_chain_id = %cfg.cosmos.chain_id,
        cosmos_rest_url = %cfg.cosmos.rest_url,
        cosmos_signer_configured = !cfg.cosmos.proposer_private_key_hex.is_empty(),
        "starting stellar-api"
    );

    let addr = cfg.addr();
    let rpc = RpcClient::new(cfg.rpc_url.as_str()).expect("could not create a new rpc client");
    let cosmos = CosmosClient::new(cfg.cosmos)?;
    if let Some(p) = cosmos.proposer_address() {
        tracing::info!(cosmos_proposer = %p, "cosmos signer derived");
    }

    let state = Arc::new(AppState::new(rpc, cfg.signing_key, cosmos));

    serve(addr, state).await
}
