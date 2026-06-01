use std::env;
use std::path::Path;

pub struct Config {
    pub cosmos_chain_id: String,
    pub cosmos_rest_url: String,
    pub cosmos_rpc_url: String,
    pub api_url: String,
    pub gateway_grpc_addr: String,
    pub hermes_config: String,
    pub stellar_signing_key: String,
    pub stellar_rpc_url: String,
    pub network_passphrase: String,
    pub deployer_identity: String,
    pub transfer_port: String,
    pub mock_client_type: String,
    pub attestation_client_type: String,
    pub tendermint_client_type: String,
    pub ibc_contract_id: String,
    pub transfer_contract_id: String,
    pub deployer_address: String,
    pub stellar_client_id: String,
    pub cosmos_client_id: String,
    pub api_image: String,
    pub api_tag: String,
    pub api_registry: String,
    pub gateway_image: String,
    pub gateway_tag: String,
    pub gateway_registry: String,
    pub hermes_repo: String,
    pub hermes_image: String,
    pub hermes_tag: String,
    pub hermes_registry: String,
    pub hermes_config_in_container: String,
    pub osmosis_config_json: String,
    pub local_key_name: String,
    pub stellar_chain_id: String,
    pub stellar_key_name: String,
    pub docker_username: String,
    pub docker_token: String,
}

impl Config {
    pub fn load(root: &Path) -> Self {
        let _ = dotenvy::from_path(root.join(".env"));

        let get = |key: &str, default: &str| env::var(key).unwrap_or_else(|_| default.to_string());

        let api_port = get("STELLAR_API_PORT", "8101");
        let grpc_port = get("STELLAR_GATEWAY_GRPC_PORT", "50052");

        let default_hermes_repo = root
            .parent()
            .map(|p| p.join("hermes-relayer"))
            .unwrap_or_else(|| root.join("../hermes-relayer"))
            .display()
            .to_string();
        let hermes_repo = env::var("HERMES_REPO")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or(default_hermes_repo);

        Self {
            hermes_repo,
            cosmos_chain_id: get("COSMOS_CHAIN_ID", "localosmosis"),
            cosmos_rest_url: get("COSMOS_REST_URL", "http://127.0.0.1:1318"),
            cosmos_rpc_url: get("COSMOS_RPC_URL", "http://127.0.0.1:26658"),
            api_url: env::var("STELLAR_API_URL")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{api_port}")),
            gateway_grpc_addr: format!("127.0.0.1:{grpc_port}"),
            hermes_config: get(
                "HERMES_CONFIG",
                &root.join("ci/hermes-config.toml").display().to_string(),
            ),
            stellar_signing_key: get("STELLAR_SIGNING_KEY", ""),
            stellar_rpc_url: get("STELLAR_RPC_URL", "https://soroban-testnet.stellar.org"),
            network_passphrase: get("NETWORK_PASSPHRASE", "Test SDF Network ; September 2015"),
            deployer_identity: get("DEPLOYER_IDENTITY", "admin"),
            transfer_port: get("TRANSFER_PORT", "transfer"),
            mock_client_type: get("MOCK_CLIENT_TYPE", "mock"),
            attestation_client_type: get("ATTESTATION_CLIENT_TYPE", "attestation"),
            tendermint_client_type: get("TENDERMINT_CLIENT_TYPE", "07-tendermint"),
            ibc_contract_id: get("IBC_CONTRACT_ID", ""),
            transfer_contract_id: get("TRANSFER_CONTRACT_ID", ""),
            deployer_address: get("DEPLOYER_ADDRESS", ""),
            stellar_client_id: get("STELLAR_CLIENT_ID", ""),
            cosmos_client_id: get("COSMOS_CLIENT_ID", ""),
            api_image: get("API_IMAGE", "amandagonsalvesx/stellar-ibc-api"),
            api_tag: get("API_TAG", "latest"),
            api_registry: get("API_REGISTRY", ""),
            gateway_image: get("GATEWAY_IMAGE", "amandagonsalvesx/stellar-gateway"),
            gateway_tag: get("GATEWAY_TAG", "latest"),
            gateway_registry: get("GATEWAY_REGISTRY", ""),
            hermes_image: get("HERMES_IMAGE", "amandagonsalvesx/stellar-hermes-cardano"),
            hermes_tag: get("HERMES_TAG", "latest"),
            hermes_registry: get("HERMES_REGISTRY", ""),
            hermes_config_in_container: get(
                "HERMES_CONFIG_IN_CONTAINER",
                "/home/hermes/.hermes/config.toml",
            ),
            osmosis_config_json: env::var("OSMOSIS_CONFIG_JSON")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    root.join("crates/osmosis/assets/default-config.json")
                        .display()
                        .to_string()
                }),
            local_key_name: get("LOCAL_KEY_NAME", "localosmosis"),
            stellar_chain_id: get("STELLAR_CHAIN_ID", "stellar-testnet"),
            stellar_key_name: get("STELLAR_KEY_NAME", "stellar-relayer"),
            docker_username: get("DOCKER_USERNAME", ""),
            docker_token: get("DOCKER_TOKEN", ""),
        }
    }

    pub fn api_local_ref(&self) -> String {
        format!("{}:{}", self.api_image, self.api_tag)
    }

    pub fn api_remote_ref(&self) -> String {
        if self.api_registry.is_empty() {
            return self.api_local_ref();
        }

        format!("{}/{}:{}", self.api_registry, self.api_image, self.api_tag)
    }

    pub fn gateway_local_ref(&self) -> String {
        format!("{}:{}", self.gateway_image, self.gateway_tag)
    }

    pub fn gateway_remote_ref(&self) -> String {
        if self.gateway_registry.is_empty() {
            return self.gateway_local_ref();
        }

        format!("{}/{}:{}", self.gateway_registry, self.gateway_image, self.gateway_tag)
    }

    pub fn hermes_local_ref(&self) -> String {
        format!("{}:{}", self.hermes_image, self.hermes_tag)
    }

    pub fn hermes_remote_ref(&self) -> String {
        if self.hermes_registry.is_empty() {
            return self.hermes_local_ref();
        }

        format!("{}/{}:{}", self.hermes_registry, self.hermes_image, self.hermes_tag)
    }

    pub fn hermes_dockerfile(&self) -> String {
        format!("{}/ci/release/hermes.Dockerfile", self.hermes_repo)
    }

    pub fn cosmos_node_info_url(&self) -> String {
        format!(
            "{}/cosmos/base/tendermint/v1beta1/node_info",
            self.cosmos_rest_url
        )
    }

    pub fn api_health_url(&self) -> String {
        format!("{}/health", self.api_url)
    }

    pub fn clients_url(&self) -> String {
        format!("{}/stellar/clients", self.api_url)
    }
}
