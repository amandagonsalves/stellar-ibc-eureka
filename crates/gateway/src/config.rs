use std::net::SocketAddr;

pub struct GatewayConfig {
    pub host: String,
    pub grpc_port: u16,
    pub api_url: String,
    pub ibc_contract_id: String,
}

impl GatewayConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("STELLAR_GATEWAY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            grpc_port: std::env::var("STELLAR_GATEWAY_GRPC_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50052),
            api_url: std::env::var("STELLAR_API_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8101".to_string()),
            ibc_contract_id: std::env::var("ROUTER_CONTRACT_ADDRESS").unwrap_or_default(),
        }
    }

    pub fn grpc_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.grpc_port)
            .parse()
            .expect("invalid grpc address")
    }
}
