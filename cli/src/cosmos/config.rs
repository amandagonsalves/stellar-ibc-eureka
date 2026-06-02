use crate::config::{get, ChainId};

pub const COMPOSE_SERVICE: &str = "cosmos";

const DEFAULT_CHAIN_ID: &str = "cardano-entrypoint";
const DEFAULT_RPC_URL: &str = "http://127.0.0.1:26657";
const DEFAULT_REST_URL: &str = "http://127.0.0.1:1317";
const DEFAULT_GRPC_URL: &str = "http://127.0.0.1:9090";
const DEFAULT_KEY_NAME: &str = "relayer";

pub struct CosmosConfig {
    pub chain_id: ChainId,
    pub rpc_url: String,
    pub rest_url: String,
    pub grpc_url: String,
    pub key_name: String,
    pub relayer_mnemonic: String,
}

impl CosmosConfig {
    pub fn from_env() -> Self {
        Self {
            chain_id: ChainId::Cosmos(get("COSMOS_CHAIN_ID", DEFAULT_CHAIN_ID)),
            rpc_url: get("COSMOS_RPC_URL", DEFAULT_RPC_URL),
            rest_url: get("COSMOS_REST_URL", DEFAULT_REST_URL),
            grpc_url: get("COSMOS_GRPC_URL", DEFAULT_GRPC_URL),
            key_name: get("COSMOS_KEY_NAME", DEFAULT_KEY_NAME),
            relayer_mnemonic: get("COSMOS_RELAYER_MNEMONIC", ""),
        }
    }

    pub fn status_url(&self) -> String {
        format!("{}/status", self.rpc_url)
    }

    pub fn node_info_url(&self) -> String {
        format!("{}/cosmos/base/tendermint/v1beta1/node_info", self.rest_url)
    }
}
