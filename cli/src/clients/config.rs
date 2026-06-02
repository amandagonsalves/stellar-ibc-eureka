use crate::config::{ClientId, Config};

pub struct ClientsConfig {
    pub stellar_chain_id: String,
    pub osmosis_chain_id: String,
    pub gateway_url: String,
    pub osmosis_rpc_url: String,
    pub api_url: String,
    pub hermes_config: String,
    pub hermes_config_path: String,
    pub cosmos_client: Option<ClientId>,
    pub stellar_client: Option<ClientId>,
}

impl From<&Config> for ClientsConfig {
    fn from(cfg: &Config) -> Self {
        Self {
            stellar_chain_id: cfg.stellar.chain_id.as_str().to_string(),
            osmosis_chain_id: cfg.osmosis.chain_id.as_str().to_string(),
            gateway_url: cfg.stellar.gateway_url.clone(),
            osmosis_rpc_url: cfg.osmosis.rpc_url.clone(),
            api_url: cfg.stellar.api_url.clone(),
            hermes_config: cfg.hermes.config.clone(),
            hermes_config_path: cfg.hermes.config_path.clone(),
            cosmos_client: cfg.deployment.cosmos_client(),
            stellar_client: cfg.deployment.stellar_client(),
        }
    }
}

impl ClientsConfig {
    pub fn clients_url(&self) -> String {
        format!("{}/stellar/clients", self.api_url)
    }

    pub fn osmosis_status_url(&self) -> String {
        format!("{}/status", self.osmosis_rpc_url)
    }

    pub fn api_health_url(&self) -> String {
        format!("{}/health", self.api_url)
    }
}
