use std::path::Path;

use anyhow::{bail, Result};

use crate::clients::config::ClientsConfig;
use crate::config::Config;
use crate::{logger, probe};

pub async fn run(cfg: &Config, _root: &Path, http: &reqwest::Client) -> Result<()> {
    let cc = ClientsConfig::from(cfg);

    if !probe::http_ok(http, &cc.api_health_url()).await {
        bail!("api health check failed at {}", cc.api_health_url());
    }

    logger::ok("api /health ok");

    if probe::get_json(http, &cc.clients_url()).await.is_none() {
        bail!("could not read {}", cc.clients_url());
    }

    logger::ok("api /stellar/clients ok");

    if !probe::http_ok(http, &cc.cosmos_status_url()).await {
        bail!("cosmos status check failed at {}", cc.cosmos_status_url());
    }

    logger::ok("cosmos /status ok");

    Ok(())
}
