use std::net::SocketAddr;

pub struct GatewayConfig {
    pub host: String,
    pub grpc_port: u16,
    pub api_url: String,
    pub ibc_contract_id: String,
}

fn env_or(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(value) => {
            let cleaned = value
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .trim()
                .to_string();

            if cleaned.is_empty() {
                default.to_string()
            } else {
                cleaned
            }
        }
        Err(_) => default.to_string(),
    }
}

impl GatewayConfig {
    pub fn from_env() -> Self {
        Self {
            host: env_or("STELLAR_GATEWAY_HOST", "0.0.0.0"),
            grpc_port: env_or("STELLAR_GATEWAY_GRPC_PORT", "50052")
                .parse()
                .unwrap_or(50052),
            api_url: env_or("STELLAR_API_URL", "http://127.0.0.1:8101"),
            ibc_contract_id: env_or("ROUTER_CONTRACT_ADDRESS", ""),
        }
    }

    pub fn grpc_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.grpc_port)
            .parse()
            .expect("invalid grpc address")
    }
}
