use std::net::SocketAddr;

pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub rpc_url: String,
    pub ibc_contract_id: String,
    pub transfer_contract_id: String,
    pub network_passphrase: String,
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

impl ApiConfig {
    pub fn from_env() -> Self {
        Self {
            host: env_or("STELLAR_API_HOST", "0.0.0.0"),
            port: env_or("STELLAR_API_PORT", "8101").parse().unwrap_or(8101),
            rpc_url: env_or("STELLAR_RPC_URL", "https://soroban-testnet.stellar.org"),
            ibc_contract_id: env_or("ROUTER_CONTRACT_ADDRESS", ""),
            transfer_contract_id: env_or("TRANSFER_CONTRACT_ADDRESS", ""),
            network_passphrase: env_or("NETWORK_PASSPHRASE", "Test SDF Network ; September 2015"),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("invalid api address")
    }
}

#[cfg(test)]
mod tests {
    use super::env_or;

    #[test]
    fn strips_surrounding_quotes_and_whitespace() {
        std::env::set_var(
            "API_CFG_TEST_QUOTED",
            "  \"https://soroban-testnet.stellar.org\" ",
        );
        assert_eq!(
            env_or("API_CFG_TEST_QUOTED", "default"),
            "https://soroban-testnet.stellar.org"
        );
        std::env::remove_var("API_CFG_TEST_QUOTED");
    }

    #[test]
    fn falls_back_to_default_when_empty_or_unset() {
        std::env::set_var("API_CFG_TEST_EMPTY", "\"\"");
        assert_eq!(env_or("API_CFG_TEST_EMPTY", "default"), "default");
        std::env::remove_var("API_CFG_TEST_EMPTY");

        assert_eq!(env_or("API_CFG_TEST_UNSET_KEY", "default"), "default");
    }
}
