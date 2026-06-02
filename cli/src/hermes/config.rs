use std::path::Path;

use crate::config::{get, ImageRef};

pub struct HermesConfig {
    pub image: ImageRef,
    pub config: String,
    pub config_path: String,
}

impl HermesConfig {
    pub fn from_env(root: &Path) -> Self {
        Self {
            image: ImageRef {
                image: get("HERMES_IMAGE", "amandagonsalvesx/stellar-hermes-cardano"),
                tag: get("HERMES_TAG", "latest"),
                registry: get("HERMES_REGISTRY", ""),
            },
            config: root.join("hermes-config.toml").display().to_string(),
            config_path: get("HERMES_CONFIG_PATH", "/home/hermes/.hermes/config.toml"),
        }
    }
}
