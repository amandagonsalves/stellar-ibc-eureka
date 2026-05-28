use std::net::SocketAddr;

pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub rpc_url: String,
    pub signing_key: String,
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
        }
    }

    pub fn addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("invalid api address")
    }
}
