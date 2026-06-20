use std::path::Path;

use anyhow::{bail, Result};

use crate::tx::clients::config::ClientsConfig;
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
    let (label, chain, client, counterparty, commitment_prefix) = match side {
        "stellar" => (
            "clients counterparty stellar (register the Cosmos client as counterparty on Stellar)",
            cfg.stellar_chain_id.as_str(),
            cosmos_client,
            stellar_client,
            "ibc",
        ),
        _ => (
            "clients counterparty cosmos (register the Stellar client as counterparty on Cosmos)",
            cfg.cosmos_chain_id.as_str(),
            stellar_client,
            cosmos_client,
            "",
        ),
    };

    logger::banner(label);

    if client.is_empty() || counterparty.is_empty() {
        bail!("both COSMOS_CLIENT_ID and STELLAR_CLIENT_ID must be set — run `interstellar clients cosmos` and `interstellar clients stellar` first");
    }

    if !run::has("docker") {
        bail!("docker not found in PATH");
    }

    logger::step(&format!(
        "hermes create counterparty --chain {chain} --client {client} --counterparty-client {counterparty} --commitment-prefix '{commitment_prefix}'"
    ));

    let output = crate::hermes::container::exec(
        root,
        cfg.hermes_config_path.as_str(),
        &[
            "create",
            "counterparty",
            "--chain",
            chain,
            "--client",
            client,
            "--counterparty-client",
            counterparty,
            "--commitment-prefix",
            commitment_prefix,
        ],
    )?;
    println!("{output}");

    logger::ok("counterparty registered");

    Ok(())
}
