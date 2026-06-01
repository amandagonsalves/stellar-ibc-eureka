use crate::config::{ClientId, Config};

pub struct ClientsConfig {
    pub stellar_chain_id: String,
    pub cosmos_chain_id: String,
    pub gateway_grpc_addr: String,
    pub cosmos_rpc_url: String,
    pub api_url: String,
    pub hermes_config: String,
    pub hermes_config_in_container: String,
    pub cosmos_client: Option<ClientId>,
    pub stellar_client: Option<ClientId>,
}

impl From<&Config> for ClientsConfig {
    fn from(cfg: &Config) -> Self {
        Self {
            stellar_chain_id: cfg.stellar.chain_id.as_str().to_string(),
            cosmos_chain_id: cfg.cosmos.chain_id.as_str().to_string(),
            gateway_grpc_addr: cfg.stellar.gateway_grpc_addr.clone(),
            cosmos_rpc_url: cfg.cosmos.rpc_url.clone(),
            api_url: cfg.stellar.api_url.clone(),
            hermes_config: cfg.hermes.config.clone(),
            hermes_config_in_container: cfg.hermes.config_in_container.clone(),
            cosmos_client: cfg.deployment.cosmos_client(),
            stellar_client: cfg.deployment.stellar_client(),
        }
    }
}

impl ClientsConfig {
    pub fn clients_url(&self) -> String {
        format!("{}/stellar/clients", self.api_url)
    }

    pub fn cosmos_status_url(&self) -> String {
        format!("{}/status", self.cosmos_rpc_url)
    }

    pub fn api_health_url(&self) -> String {
        format!("{}/health", self.api_url)
    }
}
