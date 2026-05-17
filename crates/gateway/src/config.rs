use std::net::SocketAddr;

pub struct GatewayConfig {
    pub host: String,
    pub grpc_port: u16,
    pub http_port: u16,
    pub rpc_url: String,
    pub ibc_contract_id: String,
    pub transfer_contract_id: String,
    pub network_passphrase: String,
    pub signing_key: String,
}

impl GatewayConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("STELLAR_GATEWAY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            grpc_port: std::env::var("STELLAR_GATEWAY_GRPC_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50052),
            http_port: std::env::var("STELLAR_GATEWAY_HTTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8000),
            rpc_url: std::env::var("STELLAR_RPC_URL")
                .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string()),
            ibc_contract_id: std::env::var("IBC_CONTRACT_ID").unwrap_or_default(),
            transfer_contract_id: std::env::var("TRANSFER_CONTRACT_ID").unwrap_or_default(),
            network_passphrase: std::env::var("NETWORK_PASSPHRASE")
                .unwrap_or_else(|_| "Test SDF Network ; September 2015".to_string()),
            signing_key: std::env::var("STELLAR_SIGNING_KEY").unwrap_or_default(),
        }
    }

    pub fn grpc_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.grpc_port)
            .parse()
            .expect("invalid grpc address")
    }

    pub fn http_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.http_port)
            .parse()
            .expect("invalid http address")
    }
}
