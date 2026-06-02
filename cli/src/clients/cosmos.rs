use std::path::Path;

use anyhow::Result;

use crate::clients::config::ClientsConfig;
use crate::clients::CreateSpec;
use crate::logger;

pub async fn run(
    cfg: &ClientsConfig,
    root: &Path,
    http: &reqwest::Client,
    force: bool,
) -> Result<()> {
    logger::banner("clients cosmos (Cosmos client on Stellar)");

    let spec = CreateSpec {
        host_chain: &cfg.stellar_chain_id,
        reference_chain: &cfg.cosmos_chain_id,
        id_prefix: "07-tendermint",
        result_env_var: "COSMOS_CLIENT_ID",
        existing: cfg.cosmos_client.as_ref().map(|c| c.as_str()),
    };

    super::create(cfg, root, http, &spec, force).await?;
    logger::hint("next: stellaribc clients stellar");

    Ok(())
}
