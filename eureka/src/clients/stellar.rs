use std::path::Path;

use anyhow::{bail, Result};

use crate::clients::config::ClientsConfig;
use crate::clients::CreateSpec;
use crate::logger;

pub async fn run(
    cfg: &ClientsConfig,
    root: &Path,
    http: &reqwest::Client,
    force: bool,
) -> Result<String> {
    logger::banner("clients stellar (Stellar client on Cosmos, 08-wasm)");

    if !force {
        if let Some(existing) = &cfg.stellar_client {
            logger::warn(&format!(
                "STELLAR_CLIENT_ID already set ({}). Use --force to create another.",
                existing.as_str()
            ));

            return Ok(existing.as_str().to_string());
        }
    }

    if !wasm_checksum_present(&cfg.hermes_config) {
        bail!(
            "wasm_checksum_hex is empty in {} — upload the light client first: eurekastellar contracts upload-wasm",
            cfg.hermes_config
        );
    }

    let spec = CreateSpec {
        host_chain: &cfg.cosmos_chain_id,
        reference_chain: &cfg.stellar_chain_id,
        id_prefix: "08-wasm",
        result_env_var: "STELLAR_CLIENT_ID",
        existing: cfg.stellar_client.as_ref().map(|c| c.as_str()),
    };

    let client_id = super::create(cfg, root, http, &spec, force).await?;
    logger::hint("next: eurekastellar clients counterparty stellar / cosmos");

    Ok(client_id)
}

fn wasm_checksum_present(hermes_config: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(hermes_config) else {
        return false;
    };

    for line in text.lines() {
        if line.trim_start().starts_with("wasm_checksum_hex") {
            if let Some((_, value)) = line.split_once('=') {
                let value = value
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"' || c == ' ');

                return !value.is_empty();
            }
        }
    }

    false
}
