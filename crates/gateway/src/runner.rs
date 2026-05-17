use std::sync::Arc;

use axum::{routing::get, Router};

use crate::{config::GatewayConfig, rpc::RpcClient, state::AppState};

pub async fn run(cfg: GatewayConfig) {
    let http_addr = cfg.http_addr();

    let rpc = RpcClient::new(&cfg.rpc_url.as_str()).expect("could not create a new rpc client");

    let app_state = Arc::new(AppState::new(Arc::new(rpc), cfg.signing_key.clone()));

    tokio::spawn(async move {
        let app = Router::new()
            .route("/health", get(|| async { "Server is up." }))
            .with_state(app_state);

        let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();

        tracing::info!("HTTP server listening on {}", http_addr);

        axum::serve(listener, app).await.unwrap();
    });

    let grpc_addr = cfg.grpc_addr();

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("gRPC reflection service failed to build");

    tracing::info!("gRPC server listening on {}", grpc_addr);

    tonic::transport::Server::builder()
        .add_service(reflection_service)
        .add_service(health_service)
        // .add_service(ClientServiceServer::new(client_handler))
        // .add_service(PacketServiceServer::new(packet_handler))
        // .add_service(QueryServiceServer::new(query_handler))
        // .add_service(CounterpartyServiceServer::new(counterparty_handler))
        .serve(grpc_addr)
        .await
        .expect("gRPC server failed");
}
