use anyhow::Result;

use crate::clients::config::ClientsConfig;
use crate::{logger, probe, shared};

pub async fn run(cfg: &ClientsConfig, http: &reqwest::Client) -> Result<()> {
    logger::banner("clients list");

    if !probe::http_ok(http, &cfg.api_health_url()).await {
        logger::warn("api unreachable — start it with `eurekastellar up`");

        return Ok(());
    }

    match probe::get_json(http, &cfg.clients_url()).await {
        Some(value) => shared::print_clients(&value),
        None => logger::warn("could not read /stellar/clients"),
    }

    Ok(())
}
