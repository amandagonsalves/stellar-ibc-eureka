use std::sync::Arc;

use crate::{rpc::RpcClient, services::cosmos::client::CosmosClient};

#[derive(Clone)]
pub struct AppState {
    pub rpc: Arc<RpcClient>,
    pub signing_key: Arc<String>,
    pub cosmos: Arc<CosmosClient>,
    pub hermes_config_path: Arc<String>,
    pub ibc_contract_id: Arc<String>,
    pub network_passphrase: Arc<String>,
}

impl AppState {
    pub fn new(
        rpc: RpcClient,
        signing_key: String,
        cosmos: CosmosClient,
        hermes_config_path: String,
        ibc_contract_id: String,
        network_passphrase: String,
    ) -> Self {
        Self {
            rpc: Arc::new(rpc),
            signing_key: Arc::new(signing_key),
            cosmos: Arc::new(cosmos),
            hermes_config_path: Arc::new(hermes_config_path),
            ibc_contract_id: Arc::new(ibc_contract_id),
            network_passphrase: Arc::new(network_passphrase),
        }
    }
}
