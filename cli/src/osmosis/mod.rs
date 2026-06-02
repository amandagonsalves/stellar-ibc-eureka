pub mod config;

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::osmosis::config::{OsmosisConfig, COMPOSE_SERVICE};
use crate::{logger, probe, run, shared};

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

pub fn keygen(root: &Path, force: bool) -> Result<()> {
    logger::banner("osmosis keygen (validator + relayer → .env mnemonics + signer keys)");

    if !run::has("docker") {
        bail!("docker not found in PATH — required to generate keys via the osmosis image");
    }

    let image = format!(
        "osmolabs/osmosis:{}-alpine",
        crate::config::get("OSMOSIS_VERSION", "31.0.3")
    );

    let mut updates: Vec<(&str, String)> = Vec::new();

    for (name, mnemonic_var, key_var) in [
        ("validator", "COSMOS_VALIDATOR_MNEMONIC", "COSMOS_FUNDER_PRIVATE_KEY"),
        ("relayer", "COSMOS_RELAYER_MNEMONIC", "COSMOS_PROPOSER_PRIVATE_KEY"),
    ] {
        if !crate::config::get(mnemonic_var, "").is_empty() && !force {
            logger::detail(&format!("{mnemonic_var} already set — skip (--force to regenerate)"));

            continue;
        }

        logger::step(&format!("generating {name} key"));
        let (address, mnemonic, key_hex) = generate_key(root, &image, name)?;
        logger::ok(&format!("{name} = {address}"));
        updates.push((mnemonic_var, mnemonic));
        updates.push((key_var, key_hex));
    }

    if updates.is_empty() {
        logger::detail("nothing to write — all mnemonics already set");

        return Ok(());
    }

    let refs: Vec<(&str, &str)> = updates.iter().map(|(k, v)| (*k, v.as_str())).collect();
    shared::env_upsert(&root.join(".env"), &refs)?;

    logger::ok("wrote mnemonics + signer keys to .env");
    logger::hint("rebuild genesis to fund the new accounts: stellaribc osmosis start --fresh");

    Ok(())
}

fn generate_key(root: &Path, image: &str, name: &str) -> Result<(String, String, String)> {
    let script = format!(
        "osmosisd keys add {name} --keyring-backend test --output json; \
         printf 'y\\n' | osmosisd keys export {name} --unarmored-hex --unsafe --keyring-backend test"
    );

    let out = run::capture_all(
        root,
        "docker",
        &["run", "--rm", "--entrypoint", "sh", image, "-c", &script],
    )?;

    let json_line = out
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .context("osmosisd keys add produced no JSON output")?;

    let json: serde_json::Value =
        serde_json::from_str(json_line.trim()).context("parsing osmosisd keys add output")?;

    let mnemonic = json["mnemonic"]
        .as_str()
        .context("no mnemonic in osmosisd keys add output")?
        .to_string();

    let address = json["address"].as_str().unwrap_or_default().to_string();

    let key_hex = out
        .lines()
        .map(str::trim)
        .find(|l| l.len() == 64 && l.bytes().all(|b| b.is_ascii_hexdigit()))
        .context("no unarmored hex key in osmosisd keys export output")?
        .to_string();

    Ok((address, mnemonic, key_hex))
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
