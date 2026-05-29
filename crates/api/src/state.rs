use std::sync::Arc;

use stellar_ibc_core::rpc::RpcClient;

use crate::services::cosmos::client::CosmosClient;

#[derive(Clone)]
pub struct AppState {
    pub rpc: Arc<RpcClient>,
    pub signing_key: Arc<String>,
    pub cosmos: Arc<CosmosClient>,
}

impl AppState {
    pub fn new(rpc: RpcClient, signing_key: String, cosmos: CosmosClient) -> Self {
        Self {
            rpc: Arc::new(rpc),
            signing_key: Arc::new(signing_key),
            cosmos: Arc::new(cosmos),
        }
    }
}
