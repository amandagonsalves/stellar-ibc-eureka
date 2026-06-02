pub mod config;

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::osmosis::config::{OsmosisConfig, COMPOSE_SERVICE};
use crate::{logger, probe, run};

const WAIT_TIMEOUT_SECS: u64 = 300;
const LOCAL_STATE_DIR: &str = ".osmosisd-local";

pub async fn start(
    cfg: &OsmosisConfig,
    root: &Path,
    http: &reqwest::Client,
    fresh: bool,
) -> Result<()> {
    logger::banner(&format!("osmosis start ({})", cfg.chain_id.as_str()));

    if !cfg.is_local() {
        logger::detail(&format!("testnet — external endpoints ({})", cfg.rpc_url));

        if probe::http_ok(http, &cfg.status_url()).await {
            logger::ok("testnet reachable");
        } else {
            logger::warn("testnet not reachable — check the endpoints / your connection");
        }

        return Ok(());
    }

    if fresh {
        logger::step("resetting local chain state");
        let _ = run::compose(root, &["down"]);
        reset_local_state();
    } else if probe::http_ok(http, &cfg.status_url()).await {
        logger::ok("already running");

        return Ok(());
    }

    logger::step("docker compose up -d osmosis");
    run::compose(root, &["up", "-d", COMPOSE_SERVICE])?;

    if !probe::wait_http(http, &cfg.status_url(), WAIT_TIMEOUT_SECS).await {
        bail!("osmosis not healthy within {WAIT_TIMEOUT_SECS}s (docker compose logs osmosis)");
    }

    logger::ok("osmosis running");

    Ok(())
}

pub fn stop(cfg: &OsmosisConfig, root: &Path) -> Result<()> {
    logger::banner("osmosis stop");

    if !cfg.is_local() {
        logger::detail("testnet — external, nothing to stop");

        return Ok(());
    }

    logger::step("docker compose stop osmosis");
    run::compose(root, &["stop", COMPOSE_SERVICE])?;

    logger::ok("osmosis stopped");

    Ok(())
}

pub async fn status(cfg: &OsmosisConfig, http: &reqwest::Client) -> Result<()> {
    logger::banner(&format!("osmosis status ({})", cfg.chain_id.as_str()));

    let up = probe::http_ok(http, &cfg.status_url()).await;
    logger::status_line(cfg.chain_id.as_str(), up, &cfg.rpc_url);

    let kind = if cfg.is_local() {
        "local (docker compose)"
    } else {
        "testnet (external)"
    };
    logger::detail(&format!("network   {kind}"));
    logger::detail(&format!("rest      {}", cfg.rest_url));
    logger::detail(&format!("grpc      {}", cfg.grpc_url));

    Ok(())
}

fn reset_local_state() {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return;
    };

    let dir = home.join(LOCAL_STATE_DIR);

    if !dir.exists() {
        return;
    }

    match std::fs::remove_dir_all(&dir) {
        Ok(()) => logger::detail(&format!("removed {}", dir.display())),
        Err(error) => logger::warn(&format!("could not remove {} ({error})", dir.display())),
    }
}
