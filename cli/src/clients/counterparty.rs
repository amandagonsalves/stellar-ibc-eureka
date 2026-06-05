use std::path::Path;

use anyhow::{bail, Result};

use crate::clients::config::ClientsConfig;
use crate::{logger, run};

pub fn run(cfg: &ClientsConfig, root: &Path, side: &str) -> Result<()> {
    let cosmos_client = cfg.cosmos_client.as_ref().map(|c| c.as_str()).unwrap_or("");
    let stellar_client = cfg
        .stellar_client
        .as_ref()
        .map(|c| c.as_str())
        .unwrap_or("");

    register(cfg, root, side, cosmos_client, stellar_client)
}

pub fn register(
    cfg: &ClientsConfig,
    root: &Path,
    side: &str,
    cosmos_client: &str,
    stellar_client: &str,
) -> Result<()> {
    let (label, chain, client, counterparty) = match side {
        "stellar" => (
            "clients counterparty stellar (register the Cosmos client as counterparty on Stellar)",
            cfg.stellar_chain_id.as_str(),
            cosmos_client,
            stellar_client,
        ),
        _ => (
            "clients counterparty cosmos (register the Stellar client as counterparty on Cosmos)",
            cfg.cosmos_chain_id.as_str(),
            stellar_client,
            cosmos_client,
        ),
    };

    logger::banner(label);

    if client.is_empty() || counterparty.is_empty() {
        bail!("both COSMOS_CLIENT_ID and STELLAR_CLIENT_ID must be set — run `stellaribc clients cosmos` and `stellaribc clients stellar` first");
    }

    if !run::has("docker") {
        bail!("docker not found in PATH");
    }

    logger::step(&format!(
        "hermes create counterparty --chain {chain} --client {client} --counterparty-client {counterparty}"
    ));

    let output = run::capture_all(
        root,
        "docker",
        &[
            "compose",
            "run",
            "--rm",
            "hermes",
            "--config",
            cfg.hermes_config_path.as_str(),
            "create",
            "counterparty",
            "--chain",
            chain,
            "--client",
            client,
            "--counterparty-client",
            counterparty,
        ],
    )?;
    println!("{output}");

    logger::ok("counterparty registered");

    Ok(())
}
