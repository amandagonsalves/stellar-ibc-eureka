use anyhow::Result;

use crate::config::Config;
use crate::{logger, probe, shared};

pub async fn run(cfg: &Config, http: &reqwest::Client) -> Result<()> {
    logger::banner("clients list");

    if !probe::http_ok(http, &cfg.api_health_url()).await {
        logger::warn("api unreachable — start it with `stellaribc up`");

        return Ok(());
    }

    match probe::get_json(http, &cfg.clients_url()).await {
        Some(value) => shared::print_clients(&value),
        None => logger::warn("could not read /stellar/clients"),
    }

    Ok(())
}
