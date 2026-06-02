use crate::config::{Config, StellarAddresses};

pub struct OpsConfig {
    pub cosmos_chain_id: String,
    pub cosmos_rest_url: String,
    pub cosmos_rpc_url: String,
    pub api_url: String,
    pub gateway_url: String,
    pub hermes_config: String,
    pub stellar_signing_key: String,
    pub ibc_router: String,
    pub transfer_app: String,
    pub stellar_client_id: String,
    pub addresses: Vec<(StellarAddresses, String)>,
    pub images: Vec<(&'static str, String)>,
}

impl From<&Config> for OpsConfig {
    fn from(cfg: &Config) -> Self {
        let addresses = cfg.deployment.addresses();

        let images = vec![
            ("api", cfg.api.reference()),
            ("gateway", cfg.gateway.image.reference()),
            ("hermes", cfg.hermes.image.reference()),
        ];

        Self {
            cosmos_chain_id: cfg.cosmos.chain_id.as_str().to_string(),
            cosmos_rest_url: cfg.cosmos.rest_url.clone(),
            cosmos_rpc_url: cfg.cosmos.rpc_url.clone(),
            api_url: cfg.stellar.api_url.clone(),
            gateway_url: cfg.stellar.gateway_url.clone(),
            hermes_config: cfg.hermes.config.clone(),
            stellar_signing_key: cfg.stellar.signing_key.clone(),
            ibc_router: cfg.deployment.ibc_router.clone(),
            transfer_app: cfg.deployment.transfer_app.clone(),
            stellar_client_id: cfg.deployment.stellar_client_id.clone(),
            addresses,
            images,
        }
    }
}

impl OpsConfig {
    pub fn api_health_url(&self) -> String {
        format!("{}/health", self.api_url)
    }
}
