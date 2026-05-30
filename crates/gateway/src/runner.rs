use std::sync::Arc;

use soroban_client::keypair::{Keypair, KeypairBehavior};
use tokio::sync::Mutex;

use crate::{
    config::GatewayConfig, msg::MsgHandler, query::QueryHandler, state_tracker::StateTracker,
};
use stellar_ibc_core::api_client::ApiClient;

pub async fn run(cfg: GatewayConfig) {
    tracing::info!(
        host = %cfg.host,
        grpc_port = cfg.grpc_port,
        api_url = %cfg.api_url,
        ibc_contract_id = %cfg.ibc_contract_id,
        "starting stellar-gateway"
    );

    let keypair =
        Keypair::from_secret(&cfg.signing_key).expect("failed to get keypair from secret");

    let api = ApiClient::new(&cfg.api_url, keypair.public_key());

    let ibc_contract_id = if cfg.ibc_contract_id.is_empty() {
        tracing::warn!("IBC_CONTRACT_ID is empty — state tracker will accept any contract");
        None
    } else {
        match stellar_strkey::Contract::from_string(&cfg.ibc_contract_id) {
            Ok(contract) => Some(contract.0),
            Err(error) => {
                tracing::warn!(%error, "IBC_CONTRACT_ID could not be parsed as a Stellar contract strkey");
                None
            }
        }
    };

    let tracker = Arc::new(Mutex::new(StateTracker::new(api.clone(), ibc_contract_id)));

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

    tracing::info!(%grpc_addr, "gRPC server listening");

    tonic::transport::Server::builder()
        .add_service(reflection_service)
        .add_service(health_service)
        .add_service(
            QueryHandler::new(
                api.clone(),
                tracker,
                Some(cfg.ibc_contract_id.clone()).filter(|s| !s.is_empty()),
            )
            .into_server(),
        )
        .add_service(MsgHandler::new(api.clone()).into_server())
        .serve(grpc_addr)
        .await
        .expect("gRPC server failed");
}
