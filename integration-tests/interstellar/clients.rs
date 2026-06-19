use std::path::Path;

use anyhow::{bail, Result};

use crate::clients::config::ClientsConfig;
use crate::config::Config;
use crate::logger;

pub async fn run(cfg: &Config, root: &Path, http: &reqwest::Client) -> Result<()> {
    let cc = ClientsConfig::from(cfg);

    let cosmos_client = crate::clients::cosmos::run(&cc, root, http, true).await?;
    let stellar_client = crate::clients::stellar::run(&cc, root, http, true).await?;

    expect_prefix(&cosmos_client, "07-tendermint")?;
    expect_prefix(&stellar_client, "08-wasm")?;

    logger::ok(&format!(
        "created cosmos={cosmos_client} stellar={stellar_client}"
    ));

    Ok(())
}

fn expect_prefix(id: &str, prefix: &str) -> Result<()> {
    if !id.starts_with(prefix) {
        bail!("expected a {prefix}-N client id, got {id:?}");
    }

    Ok(())
}
