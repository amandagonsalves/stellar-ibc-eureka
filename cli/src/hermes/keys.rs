use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

use crate::config::Config;
use crate::{logger, run};

pub fn import(cfg: &Config, root: &Path) -> Result<()> {
    logger::banner("hermes keys-import (relayer key = router admin key)");

    let mnemonic = read_relayer_mnemonic(&cfg.osmosis_config_json)?;

    if cfg.stellar_signing_key.is_empty() {
        bail!("STELLAR_SIGNING_KEY is empty in .env — it must be the funded contract admin/deployer secret so it can pay fees and satisfy admin.require_auth()");
    }

    logger::step("stopping hermes for a one-shot key import");
    let _ = run::compose(root, &["stop", "hermes"]);

    logger::step(&format!(
        "importing {} for {} (cosmos mnemonic)",
        cfg.local_key_name, cfg.cosmos_chain_id
    ));
    import_mnemonic(cfg, root, &cfg.cosmos_chain_id, &cfg.local_key_name, &mnemonic)?;

    logger::step(&format!(
        "importing {} for {} (from STELLAR_SIGNING_KEY)",
        cfg.stellar_key_name, cfg.stellar_chain_id
    ));
    import_secret(
        cfg,
        root,
        &cfg.stellar_chain_id,
        &cfg.stellar_key_name,
        &cfg.stellar_signing_key,
    )?;

    logger::step("starting hermes with keys in place");
    run::compose(root, &["up", "-d", "hermes"])?;

    logger::ok("keys imported into the hermes-keys volume (persists across restarts)");

    Ok(())
}

fn read_relayer_mnemonic(path: &str) -> Result<String> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parsing {path}"))?;

    json.get("keys")
        .and_then(|k| k.get("relayer"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{path} is missing .keys.relayer"))
}

fn import_mnemonic(
    cfg: &Config,
    root: &Path,
    chain: &str,
    key_name: &str,
    mnemonic: &str,
) -> Result<()> {
    let script = format!(
        "cat > /tmp/m.txt && hermes --config {cfg_path} keys add --chain {chain} --mnemonic-file /tmp/m.txt --key-name {key_name} --overwrite; rc=$?; rm -f /tmp/m.txt; exit $rc",
        cfg_path = cfg.hermes_config_in_container,
    );

    run::piped(
        root,
        "docker",
        &compose_run_args(&script),
        &format!("{mnemonic}\n"),
    )
}

fn import_secret(
    cfg: &Config,
    root: &Path,
    chain: &str,
    key_name: &str,
    secret: &str,
) -> Result<()> {
    let script = format!(
        "cat > /tmp/k.json && hermes --config {cfg_path} keys add --chain {chain} --key-file /tmp/k.json --key-name {key_name} --overwrite; rc=$?; rm -f /tmp/k.json; exit $rc",
        cfg_path = cfg.hermes_config_in_container,
    );

    run::piped(
        root,
        "docker",
        &compose_run_args(&script),
        &format!("{{\"secret_key\":\"{secret}\"}}\n"),
    )
}

fn compose_run_args(script: &str) -> [&str; 14] {
    [
        "compose",
        "--profile",
        "local",
        "--profile",
        "hermes",
        "run",
        "--rm",
        "--no-deps",
        "-T",
        "--entrypoint",
        "sh",
        "hermes",
        "-c",
        script,
    ]
}
