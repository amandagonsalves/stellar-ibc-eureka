use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::contracts::config::ContractsConfig;
use crate::ops::config::OpsConfig;
use crate::{hermes, logger, osmosis, probe, run};

const WAIT_TIMEOUT_SECS: u64 = 300;

pub async fn run(
    cfg: &Config,
    root: &Path,
    http: &reqwest::Client,
    skip_images: bool,
    skip_contracts: bool,
    skip_wasm: bool,
    skip_keys: bool,
    force_redeploy: bool,
) -> Result<()> {
    logger::banner("start");

    if !run::has("docker") {
        bail!("docker not found in PATH — required to bring up the stack");
    }

    let ops = OpsConfig::from(cfg);

    if skip_images {
        logger::detail("skip image pull");
    } else {
        logger::step("Step 0: pulling images (osmosis, api, gateway, hermes)");
        run::compose(root, &["pull", "osmosis", "api", "gateway", "hermes"])?;
    }

    logger::step("Step 1: ensuring osmosis is up");
    osmosis::start(&cfg.osmosis, root, http, false).await?;

    logger::step("Step 2: ensuring api + gateway are up");
    if probe::http_ok(http, &ops.api_health_url()).await {
        logger::ok("api already reachable");
    } else {
        run::compose(root, &["up", "-d", "api", "gateway"])?;

        if !probe::wait_http(http, &ops.api_health_url(), WAIT_TIMEOUT_SECS).await {
            bail!("api not reachable within {WAIT_TIMEOUT_SECS}s (docker compose logs api gateway)");
        }

        logger::ok("api + gateway reachable");
    }

    if skip_contracts {
        logger::detail("skip contract deploy");
    } else {
        logger::step("Step 3: deploying Soroban contracts");
        crate::contracts::deploy_all::run(&ContractsConfig::from(cfg), root, force_redeploy, false, false)?;

        logger::step("recreating api + gateway to pick up ROUTER_CONTRACT_ADDRESS");
        run::compose(root, &["up", "-d", "--force-recreate", "api", "gateway"])?;
        let _ = probe::wait_http(http, &ops.api_health_url(), WAIT_TIMEOUT_SECS).await;
    }

    if skip_wasm {
        logger::detail("skip lc-wasm upload");
    } else {
        logger::step("Step 4: uploading light-client-wasm to Cosmos");
        crate::contracts::wasm::upload(&ContractsConfig::from(cfg), root, http).await?;
    }

    if skip_keys {
        logger::detail("skip hermes keys import");
    } else {
        logger::step("Step 5: importing hermes relayer keys");
        hermes::keys::import(cfg, root)?;
    }

    logger::ok("start complete");
    logger::hint("check: stellaribc status   then: stellaribc clients cosmos");

    Ok(())
}
