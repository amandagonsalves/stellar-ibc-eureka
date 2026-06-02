use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::{logger, run};

pub fn import(cfg: &Config, root: &Path) -> Result<()> {
    logger::banner("hermes keys-import (relayer key = router admin key)");

    if cfg.cosmos.relayer_mnemonic.is_empty() {
        bail!("COSMOS_RELAYER_MNEMONIC is empty in .env — set the cosmos relayer mnemonic (a faucet-funded account)");
    }

    if cfg.stellar.signing_key.is_empty() {
        bail!("STELLAR_SIGNING_KEY is empty in .env — it must be the funded contract admin/deployer secret so it can pay fees and satisfy admin.require_auth()");
    }

    logger::step("stopping hermes for a one-shot key import");
    let _ = run::compose(root, &["stop", "hermes"]);

    logger::step(&format!(
        "importing {} for {} (cosmos mnemonic)",
        cfg.cosmos.key_name,
        cfg.cosmos.chain_id.as_str()
    ));
    import_mnemonic(
        cfg,
        root,
        cfg.cosmos.chain_id.as_str(),
        &cfg.cosmos.key_name,
        &cfg.cosmos.relayer_mnemonic,
    )?;

    logger::step(&format!(
        "importing {} for {} (from STELLAR_SIGNING_KEY)",
        cfg.stellar.key_name,
        cfg.stellar.chain_id.as_str()
    ));
    import_secret(
        cfg,
        root,
        cfg.stellar.chain_id.as_str(),
        &cfg.stellar.key_name,
        &cfg.stellar.signing_key,
    )?;

    logger::step("starting hermes with keys in place");
    run::compose(root, &["up", "-d", "hermes"])?;

    logger::ok("keys imported into the hermes-keys volume (persists across restarts)");

    Ok(())
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
        cfg_path = cfg.hermes.config_path,
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
        cfg_path = cfg.hermes.config_path,
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
