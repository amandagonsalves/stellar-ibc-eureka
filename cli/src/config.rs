use std::env;
use std::path::Path;

use crate::gateway::config::GatewayConfig;
use crate::hermes::config::HermesConfig;
use crate::cosmos::config::CosmosConfig;
use crate::stellar::config::StellarConfig;

pub enum ChainId {
    Cosmos(String),
    Cardano(String),
    Stellar(String),
}

impl ChainId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Cosmos(id) | Self::Cardano(id) | Self::Stellar(id) => id,
        }
    }
}

pub enum ClientId {
    Cosmos(String),
    Cardano(String),
    Stellar(String),
}

impl ClientId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Cosmos(id) | Self::Cardano(id) | Self::Stellar(id) => id,
        }
    }
}

pub enum ClientTypes {
    Tendermint,
    Attestation,
    Mock,
}

impl ClientTypes {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Attestation => "attestation",
            Self::Tendermint => "07-tendermint",
            Self::Mock => "mock",
        }
    }

    pub fn attestation() -> &'static str {
        Self::Attestation.as_str()
    }

    pub fn mock() -> &'static str {
        Self::Mock.as_str()
    }

    pub fn tendermint() -> &'static str {
        Self::Tendermint.as_str()
    }
}

pub enum StellarAddresses {
    IbcRouter,
    Transfer,
    Deployer,
}

impl StellarAddresses {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::IbcRouter => "router",
            Self::Transfer => "transfer-app",
            Self::Deployer => "deployer",
        }
    }
}

pub struct ImageRef {
    pub image: String,
    pub tag: String,
    pub registry: String,
}

impl ImageRef {
    pub fn reference(&self) -> String {
        if self.registry.is_empty() {
            return format!("{}:{}", self.image, self.tag);
        }

        format!("{}/{}:{}", self.registry, self.image, self.tag)
    }
}

pub struct DeploymentConfig {
    pub ibc_router: String,
    pub transfer_app: String,
    pub deployer_address: String,
    pub transfer_port: String,
    pub mock_client_type: String,
    pub attestation_client_type: String,
    pub tendermint_client_type: String,
    pub cosmos_client_id: String,
    pub stellar_client_id: String,
}

impl DeploymentConfig {
    pub fn addresses(&self) -> Vec<(StellarAddresses, String)> {
        [
            (StellarAddresses::IbcRouter, &self.ibc_router),
            (StellarAddresses::Transfer, &self.transfer_app),
            (StellarAddresses::Deployer, &self.deployer_address),
        ]
        .into_iter()
        .filter_map(|(kind, value)| non_empty(value).map(|v| (kind, v)))
        .collect()
    }

    pub fn cosmos_client(&self) -> Option<ClientId> {
        non_empty(&self.cosmos_client_id).map(ClientId::Cosmos)
    }

    pub fn stellar_client(&self) -> Option<ClientId> {
        non_empty(&self.stellar_client_id).map(ClientId::Stellar)
    }
}

pub struct Config {
    pub cosmos: CosmosConfig,
    pub stellar: StellarConfig,
    pub hermes: HermesConfig,
    pub api: ImageRef,
    pub gateway: GatewayConfig,
    pub deployment: DeploymentConfig,
}

pub fn get(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

impl Config {
    pub fn load(root: &Path) -> Self {
        let _ = dotenvy::from_path(root.join(".env"));

        Self {
            cosmos: CosmosConfig::from_env(),
            stellar: StellarConfig::from_env(),
            hermes: HermesConfig::from_env(root),
            api: ImageRef {
                image: get("API_IMAGE", "amandagonsalvesx/stellar-ibc-api"),
                tag: get("API_TAG", "latest"),
                registry: get("API_REGISTRY", ""),
            },
            gateway: GatewayConfig::from_env(),
            deployment: DeploymentConfig {
                ibc_router: get("ROUTER_CONTRACT_ADDRESS", ""),
                transfer_app: get("TRANSFER_CONTRACT_ADDRESS", ""),
                deployer_address: get("DEPLOYER_ADDRESS", ""),
                transfer_port: get("TRANSFER_PORT", "transfer"),
                mock_client_type: get("MOCK_CLIENT_TYPE", ClientTypes::mock()),
                attestation_client_type: get("ATTESTATION_CLIENT_TYPE", ClientTypes::attestation()),
                tendermint_client_type: get("TENDERMINT_CLIENT_TYPE", ClientTypes::tendermint()),
                cosmos_client_id: get("COSMOS_CLIENT_ID", ""),
                stellar_client_id: get("STELLAR_CLIENT_ID", ""),
            },
        }
    }
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    Some(value.to_string())
}
