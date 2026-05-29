use std::net::SocketAddr;

pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub rpc_url: String,
    pub signing_key: String,
    pub cosmos: CosmosConfig,
}

pub struct CosmosConfig {
    pub chain_id: String,
    pub rest_url: String,
    pub rpc_url: String,
    pub account_prefix: String,
    pub gas_denom: String,
    pub proposer_private_key_hex: String,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("STELLAR_API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("STELLAR_API_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8101),
            rpc_url: std::env::var("STELLAR_RPC_URL")
                .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string()),
            signing_key: std::env::var("STELLAR_SIGNING_KEY").unwrap_or_default(),
            cosmos: CosmosConfig::from_env(),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("invalid api address")
    }
}

impl CosmosConfig {
    pub fn from_env() -> Self {
        Self {
            chain_id: std::env::var("COSMOS_CHAIN_ID")
                .unwrap_or_else(|_| "localosmosis".to_string()),
            rest_url: std::env::var("COSMOS_REST_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:1318".to_string()),
            rpc_url: std::env::var("COSMOS_RPC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:26658".to_string()),
            account_prefix: std::env::var("COSMOS_ACCOUNT_PREFIX")
                .unwrap_or_else(|_| "osmo".to_string()),
            gas_denom: std::env::var("COSMOS_GAS_DENOM")
                .unwrap_or_else(|_| "uosmo".to_string()),
            proposer_private_key_hex: std::env::var("COSMOS_PROPOSER_PRIVATE_KEY")
                .unwrap_or_default(),
        }
    }
}
