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
    /// Stellar signing key (secret seed). Env: `STELLAR_SIGNING_KEY`.
    /// Empty when not configured; tx-signing endpoints will fail without it.
    pub signing_key: String,
    /// Cosmos-side settings. See [`CosmosConfig`].
    pub cosmos: CosmosConfig,
    /// Path to the hermes config file the api may patch. Env:
    /// `HERMES_CONFIG_PATH`. Default `/etc/hermes/config.toml`.
    pub hermes_config_path: String,
    pub ibc_contract_id: String,
    pub network_passphrase: String,
}

/// Cosmos chain configuration consumed by [`super::services::cosmos`].
pub struct CosmosConfig {
    /// Chain id. Env: `COSMOS_CHAIN_ID`. Default `localosmosis`.
    pub chain_id: String,
    /// REST endpoint URL. Env: `COSMOS_REST_URL`.
    /// Default `http://127.0.0.1:1318`.
    pub rest_url: String,
    /// Tendermint RPC URL. Env: `COSMOS_RPC_URL`.
    /// Default `http://127.0.0.1:26658`.
    pub rpc_url: String,
    /// Bech32 prefix. Env: `COSMOS_ACCOUNT_PREFIX`. Default `osmo`.
    pub account_prefix: String,
    /// Gas/fee denom. Env: `COSMOS_GAS_DENOM`. Default `uosmo`.
    pub gas_denom: String,
    /// Hex-encoded proposer secp256k1 private key.
    /// Env: `COSMOS_PROPOSER_PRIVATE_KEY`. Empty when unconfigured.
    pub proposer_private_key_hex: String,
    /// Hex-encoded funder secp256k1 private key (typically the genesis
    /// validator on localnets). Env: `COSMOS_FUNDER_PRIVATE_KEY`. Empty when
    /// unconfigured.
    pub funder_private_key_hex: String,
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
            hermes_config_path: std::env::var("HERMES_CONFIG_PATH")
                .unwrap_or_else(|_| "/etc/hermes/config.toml".to_string()),
            ibc_contract_id: std::env::var("IBC_CONTRACT_ID").unwrap_or_default(),
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
            funder_private_key_hex: std::env::var("COSMOS_FUNDER_PRIVATE_KEY")
                .unwrap_or_default(),
        }
    }
}
