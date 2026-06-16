//! Configuration loaded from environment variables.

use std::net::SocketAddr;

/// Top-level api configuration.
///
/// Loaded via [`ApiConfig::from_env`]. Every field has a sensible default so
/// the service starts even with no env set (useful for local development).
pub struct ApiConfig {
    /// Bind host. Env: `STELLAR_API_HOST`. Default `0.0.0.0`.
    pub host: String,
    /// Bind port. Env: `STELLAR_API_PORT`. Default `8101`.
    pub port: u16,
    /// Stellar RPC URL. Env: `STELLAR_RPC_URL`.
    /// Default `https://soroban-testnet.stellar.org`.
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
