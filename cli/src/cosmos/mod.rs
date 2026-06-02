pub mod config;

use std::path::Path;

use anyhow::{bail, Result};

use crate::cosmos::config::{CosmosConfig, COMPOSE_SERVICE};
use crate::{logger, probe, run};

const WAIT_TIMEOUT_SECS: u64 = 300;

pub async fn start(cfg: &CosmosConfig, root: &Path, http: &reqwest::Client) -> Result<()> {
    logger::banner(&format!("cosmos start ({})", cfg.chain_id.as_str()));

    if probe::http_ok(http, &cfg.status_url()).await {
        logger::ok("already running");

        return Ok(());
    }

    logger::step("docker compose up -d cosmos");
    run::compose(root, &["up", "-d", COMPOSE_SERVICE])?;

    if !probe::wait_http(http, &cfg.status_url(), WAIT_TIMEOUT_SECS).await {
        bail!("cosmos not healthy within {WAIT_TIMEOUT_SECS}s (docker compose logs cosmos cosmos-init)");
    }

    logger::ok("cosmos running");

    Ok(())
}

pub fn stop(_cfg: &CosmosConfig, root: &Path) -> Result<()> {
    logger::banner("cosmos stop");

    logger::step("docker compose stop cosmos");
    run::compose(root, &["stop", COMPOSE_SERVICE, "cosmos-init"])?;

    logger::ok("cosmos stopped");

    Ok(())
}

pub async fn status(cfg: &CosmosConfig, http: &reqwest::Client) -> Result<()> {
    logger::banner(&format!("cosmos status ({})", cfg.chain_id.as_str()));

    let up = probe::http_ok(http, &cfg.status_url()).await;
    logger::status_line(cfg.chain_id.as_str(), up, &cfg.rpc_url);

    logger::detail(&format!("rest      {}", cfg.rest_url));
    logger::detail(&format!("grpc      {}", cfg.grpc_url));

    Ok(())
}
