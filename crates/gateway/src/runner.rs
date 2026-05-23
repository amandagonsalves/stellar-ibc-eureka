use std::sync::Arc;

use axum::{routing::get, Router};
use tokio::sync::Mutex;

use crate::{
    api::account::{account, balance},
    config::GatewayConfig,
    msg::MsgHandler,
    query::QueryHandler,
    state::AppState,
    state_tracker::StateTracker,
};
use stellar_ibc_core::rpc::RpcClient;

pub async fn run(cfg: GatewayConfig) {
    let http_addr = cfg.http_addr();

    let rpc = RpcClient::new(cfg.rpc_url.as_str()).expect("could not create a new rpc client");

    let ibc_contract_id = if cfg.ibc_contract_id.is_empty() {
        None
    } else {
        stellar_strkey::Contract::from_string(&cfg.ibc_contract_id)
            .ok()
            .map(|c| c.0)
    };

    let tracker = Arc::new(Mutex::new(StateTracker::new(rpc.clone(), ibc_contract_id)));

    let app_state = Arc::new(AppState::new(rpc.clone(), cfg.signing_key.clone()));

    tokio::spawn(async move {
        let app = Router::new()
            .route("/health", get(|| async { "Server is up." }))
            .route("/account/{address}", get(account))
            .route("/balance/{address}", get(balance))
            .with_state(app_state.clone());

        let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();

        tracing::info!("HTTP server listening on {}", http_addr);

        axum::serve(listener, app).await.unwrap();
    });

    let grpc_addr = cfg.grpc_addr();

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    const GATEWAY_FILE_DESCRIPTOR_SET: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/stellar_gateway_descriptor.bin"));

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(GATEWAY_FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("gRPC reflection service failed to build");

    tracing::info!("gRPC server listening on {}", grpc_addr);

    tonic::transport::Server::builder()
        .add_service(reflection_service)
        .add_service(health_service)
        .add_service(
            QueryHandler::new(
                rpc.clone(),
                tracker,
                Some(cfg.ibc_contract_id.clone()).filter(|s| !s.is_empty()),
            )
            .into_server(),
        )
        .add_service(
            MsgHandler::new(
                rpc.clone(),
                cfg.ibc_contract_id.clone(),
                cfg.signing_key.clone(),
                cfg.network_passphrase.clone(),
            )
            .into_server(),
        )
        .serve(grpc_addr)
        .await
        .expect("gRPC server failed");
}
