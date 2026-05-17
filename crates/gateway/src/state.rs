use std::sync::Arc;

use crate::rpc::RpcClient;

#[derive(Clone)]
pub struct AppState {
    pub rpc: Arc<RpcClient>,
    pub signing_key: Arc<String>,
}

impl AppState {
    pub fn new(rpc: Arc<RpcClient>, signing_key: String) -> Self {
        Self {
            rpc,
            signing_key: Arc::new(signing_key),
        }
    }
}
