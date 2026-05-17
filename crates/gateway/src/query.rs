use std::sync::Arc;

use crate::{
    proto::{stellar_gateway_query_server::StellarGatewayQueryServer, LatestHeightResponse},
    rpc::RpcClient,
};
use ibc::core::client::ClientQueryService;

#[derive(Clone)]
pub struct QueryHandler {
    pub rpc: Arc<RpcClient>,
}

impl QueryHandler {
    pub fn new(rpc: Arc<RpcClient>) -> Self {
        Self { rpc }
    }

    pub fn into_server(self) -> StellarGatewayQueryServer<Self> {
        ClientQueryService::new()
    }

    pub async fn latest_height(self) -> LatestHeightResponse {
        let latest_sequence = self
            .rpc
            .latest_ledger_sequence()
            .await
            .expect("failed to get latest ledger sequence");

        LatestHeightResponse {
            revision_height: latest_sequence.into(),
            revision_number: 0,
        }
    }
}
