use std::sync::Arc;

use stellar_ibc_core::rpc::RpcClient;

#[derive(Clone)]
pub struct AppState {
    pub rpc: Arc<RpcClient>,
    pub signing_key: Arc<String>,
}

impl AppState {
    pub fn new(rpc: RpcClient, signing_key: String) -> Self {
        Self {
            rpc: Arc::new(rpc),
            signing_key: Arc::new(signing_key),
        }
    }
}
