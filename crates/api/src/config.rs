use std::net::SocketAddr;

pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub rpc_url: String,
    pub ibc_contract_id: String,
    pub transfer_contract_id: String,
    pub network_passphrase: String,
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
            ibc_contract_id: std::env::var("ROUTER_CONTRACT_ADDRESS").unwrap_or_default(),
            transfer_contract_id: std::env::var("TRANSFER_CONTRACT_ADDRESS").unwrap_or_default(),
            network_passphrase: std::env::var("NETWORK_PASSPHRASE")
                .unwrap_or_else(|_| "Test SDF Network ; September 2015".to_string()),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("invalid api address")
    }
}
