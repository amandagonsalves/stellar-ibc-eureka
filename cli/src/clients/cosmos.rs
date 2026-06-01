use std::path::Path;

use anyhow::Result;

use crate::clients::CreateSpec;
use crate::config::Config;
use crate::logger;

pub async fn run(cfg: &Config, root: &Path, http: &reqwest::Client, force: bool) -> Result<()> {
    logger::banner("clients cosmos (F1.1 — Cosmos client on Stellar)");

    let spec = CreateSpec {
        host_chain: &cfg.stellar_chain_id,
        reference_chain: &cfg.cosmos_chain_id,
        id_prefix: "07-tendermint",
        result_env_var: "COSMOS_CLIENT_ID",
        existing: &cfg.cosmos_client_id,
    };

    super::create(cfg, root, http, &spec, force).await?;
    logger::hint("next: stellaribc clients stellar   (F1.2)");

    Ok(())
}
