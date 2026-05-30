use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use soroban_client::keypair::{Keypair, KeypairBehavior};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{config::ApiConfig, services, AppState};
use crate::{rpc::RpcClient, services::cosmos::client::CosmosClient};

/// OpenAPI document for the cosmos + hermes endpoints. Served as JSON at
/// `/api-docs/openapi.json` and rendered as Swagger UI at `/docs`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "stellar-api",
        description = "HTTP service that fronts the Stellar IBC stack, a configured Cosmos chain, and the hermes relayer config.",
        version = "0.1.0",
    ),
    tags(
        (name = "Cosmos read", description = "Read-only proxies to the configured Cosmos REST endpoint."),
        (name = "Cosmos write", description = "Signed Cosmos SDK transactions broadcast via the api's proposer/funder keys."),
        (name = "Hermes", description = "Mutations to the bound hermes config file."),
    ),
    paths(
        services::cosmos::node_info,
        services::cosmos::proposals,
        services::cosmos::proposal_by_id,
        services::cosmos::gov_deposit_params,
        services::cosmos::tx_by_hash,
        services::cosmos::ibc_wasm_checksums,
        services::cosmos::proposer_info,
        services::cosmos::funder_info,
        services::cosmos::submit_store_code,
        services::cosmos::submit_vote,
        services::cosmos::submit_bank_send,
        services::hermes::patch_wasm_checksum,
    ),
    components(schemas(
        services::cosmos::StoreCodeRequest,
        services::cosmos::VoteRequest,
        services::cosmos::BankSendRequest,
        services::hermes::PatchChecksumRequest,
    )),
)]
pub struct ApiDoc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ledger/latest", get(services::ledgers::get_latest_ledger))
        .route("/ledger/{sequence}", get(services::ledgers::get_ledger))
        .route("/events", get(services::events::get_events))
        .route("/account/{address}", get(services::account::account))
        .route("/balance/{address}", get(services::balance::balance))
        .route("/tx/submit", post(services::tx::submit_signed_tx))
        .route("/tx/prepare", post(services::tx::prepare_tx))
        .route("/cosmos/node-info", get(services::cosmos::node_info))
        .route("/cosmos/proposer", get(services::cosmos::proposer_info))
        .route("/cosmos/funder", get(services::cosmos::funder_info))
        .route(
            "/cosmos/bank/send",
            post(services::cosmos::submit_bank_send),
        )
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
        .route(
            "/hermes/wasm-checksum",
            post(services::hermes::patch_wasm_checksum),
        )
        .with_state(state)
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
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
    let keypair = Keypair::from_secret(&cfg.signing_key).unwrap();

    let rpc = RpcClient::new(cfg.rpc_url.as_str(), &keypair.public_key())
        .expect("could not create a new rpc client");
    let cosmos = CosmosClient::new(cfg.cosmos)?;
    if let Some(p) = cosmos.proposer_address() {
        tracing::info!(cosmos_proposer = %p, "cosmos signer derived");
    }
    tracing::info!(hermes_config_path = %cfg.hermes_config_path, "hermes config target");

    let state = Arc::new(AppState::new(
        rpc,
        cfg.signing_key,
        cosmos,
        cfg.hermes_config_path,
        cfg.ibc_contract_id,
        cfg.network_passphrase,
    ));

    serve(addr, state).await
}
