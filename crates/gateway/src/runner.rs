use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    config::GatewayConfig, msg::MsgHandler, query::QueryHandler, state_tracker::StateTracker,
};
use stellar_ibc_core::rpc::RpcClient;

pub async fn run(cfg: GatewayConfig) {
    let rpc = RpcClient::new(cfg.rpc_url.as_str()).expect("could not create a new rpc client");

    let ibc_contract_id = if cfg.ibc_contract_id.is_empty() {
        None
    } else {
        stellar_strkey::Contract::from_string(&cfg.ibc_contract_id)
            .ok()
            .map(|c| c.0)
    };

    let tracker = Arc::new(Mutex::new(StateTracker::new(rpc.clone(), ibc_contract_id)));

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
