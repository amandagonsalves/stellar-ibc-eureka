pub mod config;
pub mod cosmos;
pub mod counterparty;
pub mod list;
pub mod stellar;

use std::path::Path;

use anyhow::{anyhow, bail, Result};

use crate::clients::config::ClientsConfig;
use crate::{logger, probe, run, shared};

pub(crate) struct CreateSpec<'a> {
    pub host_chain: &'a str,
    pub reference_chain: &'a str,
    pub id_prefix: &'a str,
    pub result_env_var: &'a str,
    pub existing: Option<&'a str>,
}

pub(crate) async fn create(
    cfg: &ClientsConfig,
    root: &Path,
    http: &reqwest::Client,
    spec: &CreateSpec<'_>,
    force: bool,
) -> Result<()> {
    if let Some(existing) = spec.existing {
        if !force {
            logger::warn(&format!(
                "{} already set ({existing}). Use --force to create another.",
                spec.result_env_var
            ));

            return Ok(());
        }
    }

    if !run::has("docker") {
        bail!("docker not found in PATH");
    }

    logger::step(&format!("probing gateway gRPC at {}", cfg.gateway_url));
    if !probe::tcp_ok(&cfg.gateway_url) {
        bail!(
            "gateway not reachable at {} — start it with: stellaribc gateway start",
            cfg.gateway_url
        );
    }

    logger::step(&format!("probing Cosmos RPC at {}", cfg.osmosis_status_url()));
    if !probe::http_ok(http, &cfg.osmosis_status_url()).await {
        bail!(
            "Cosmos RPC not reachable at {} — start it with: stellaribc up --cosmos",
            cfg.osmosis_rpc_url
        );
    }

    logger::step(&format!(
        "hermes create client --host-chain {} --reference-chain {}",
        spec.host_chain, spec.reference_chain
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
            cfg.hermes_config_in_container.as_str(),
            "create",
            "client",
            "--host-chain",
            spec.host_chain,
            "--reference-chain",
            spec.reference_chain,
        ],
    )?;
    println!("{output}");

    let client_id = extract_client_id(&output, spec.id_prefix)
        .ok_or_else(|| anyhow!("no {}-N client id found in hermes output", spec.id_prefix))?;
    logger::ok(&format!("created: {client_id}"));

    shared::env_upsert(&root.join(".env"), &[(spec.result_env_var, client_id.as_str())])?;
    logger::detail(&format!("{}={client_id}", spec.result_env_var));

    Ok(())
}

fn extract_client_id(output: &str, prefix: &str) -> Option<String> {
    let needle = format!("{prefix}-");

    output
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .find(|token| {
            token
                .strip_prefix(&needle)
                .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
        })
        .map(str::to_string)
}
