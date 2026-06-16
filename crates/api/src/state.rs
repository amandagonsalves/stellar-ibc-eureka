use std::sync::Arc;

use crate::rpc::RpcClient;

#[derive(Clone)]
pub struct AppState {
    pub rpc: Arc<RpcClient>,
    pub ibc_contract_id: Arc<String>,
    pub transfer_contract_id: Arc<String>,
    pub network_passphrase: Arc<String>,
}

impl AppState {
    pub fn new(
        rpc: RpcClient,
        ibc_contract_id: String,
        transfer_contract_id: String,
        network_passphrase: String,
    ) -> Self {
        Self {
            rpc: Arc::new(rpc),
            ibc_contract_id: Arc::new(ibc_contract_id),
            transfer_contract_id: Arc::new(transfer_contract_id),
            network_passphrase: Arc::new(network_passphrase),
        }
    }
}
