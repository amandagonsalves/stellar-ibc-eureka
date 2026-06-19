pub mod config;
pub mod cosmos;
pub mod counterparty;
pub mod list;
pub mod stellar;

use std::path::Path;

use anyhow::{anyhow, bail, Result};

use crate::clients::config::ClientsConfig;
use crate::{logger, probe, run, shared};

#[derive(clap::Subcommand)]
pub enum ClientsCmd {
    #[command(about = "Create the Cosmos (Tendermint) client on Stellar")]
    Cosmos {
        #[arg(
            long,
            help = "Create a new client even if COSMOS_CLIENT_ID is already set"
        )]
        force: bool,
    },
    #[command(about = "Create the Stellar (08-wasm) client on Cosmos")]
    Stellar {
        #[arg(
            long,
            help = "Create a new client even if STELLAR_CLIENT_ID is already set"
        )]
        force: bool,
    },
    #[command(about = "Register a counterparty on the given side (stellar or cosmos)")]
    Counterparty {
        #[arg(value_enum, help = "Which side to register the counterparty on")]
        chain: shared::Chain,
    },
    #[command(
        about = "Create both clients and register both counterparties atomically (ids can't drift)"
    )]
    Bootstrap {
        #[arg(long, help = "Force-create fresh clients even if ids are already set")]
        force: bool,
    },
    #[command(about = "List clients created on the Stellar router")]
    List,
}

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
) -> Result<String> {
    if let Some(existing) = spec.existing {
        if !force {
            logger::warn(&format!(
                "{} already set ({existing}). Use --force to create another.",
                spec.result_env_var
            ));

            return Ok(existing.to_string());
        }
    }

    if !run::has("docker") {
        bail!("docker not found in PATH");
    }

    logger::step(&format!("probing gateway gRPC at {}", cfg.gateway_url));
    if !probe::tcp_ok(&cfg.gateway_url) {
        bail!(
            "gateway not reachable at {} — start it with: interstellar gateway start",
            cfg.gateway_url
        );
    }

    logger::step(&format!(
        "probing Cosmos RPC at {}",
        cfg.cosmos_status_url()
    ));
    if !probe::http_ok(http, &cfg.cosmos_status_url()).await {
        bail!(
            "Cosmos RPC not reachable at {} — start it with: interstellar up --cosmos",
            cfg.cosmos_rpc_url
        );
    }

    logger::step(&format!(
        "hermes create client --host-chain {} --reference-chain {}",
        spec.host_chain, spec.reference_chain
    ));
    let output = crate::hermes::container::exec(
        root,
        cfg.hermes_config_path.as_str(),
        &[
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

    shared::env_upsert(
        &root.join(".env"),
        &[(spec.result_env_var, client_id.as_str())],
    )?;
    logger::detail(&format!("{}={client_id}", spec.result_env_var));

    Ok(client_id)
}

pub async fn bootstrap(
    cfg: &ClientsConfig,
    root: &Path,
    http: &reqwest::Client,
    force: bool,
) -> Result<()> {
    logger::banner("clients bootstrap (create both clients + register both counterparties)");

    let cosmos_client = cosmos::run(cfg, root, http, force).await?;
    let stellar_client = stellar::run(cfg, root, http, force).await?;

    counterparty::register(cfg, root, "stellar", &cosmos_client, &stellar_client)?;
    counterparty::register(cfg, root, "cosmos", &cosmos_client, &stellar_client)?;

    logger::ok(&format!(
        "bootstrap complete: cosmos={cosmos_client} stellar={stellar_client} (counterparties paired)"
    ));

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
