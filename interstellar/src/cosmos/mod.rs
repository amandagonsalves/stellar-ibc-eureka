pub mod config;
pub mod tx;

use std::path::Path;

use anyhow::{bail, Result};

use crate::cosmos::config::{CosmosConfig, COMPOSE_SERVICE};
use crate::{logger, probe, run};

const WAIT_TIMEOUT_SECS: u64 = 300;

#[derive(clap::Subcommand)]
pub enum CosmosCmd {
    #[command(about = "Start the local Cosmos chain (cardano-entrypoint, ibc-go v10 + 08-wasm)")]
    Start,
    #[command(about = "Stop the local Cosmos chain")]
    Stop,
    #[command(about = "Show the Cosmos chain endpoints and health")]
    Status,
    #[command(
        about = "Check the public cosmos-testnet (Cosmos Hub `provider`) — health + node/app version"
    )]
    Testnet {
        #[arg(
            long,
            value_name = "ADDRESS",
            help = "Query this cosmos address's balances on cosmos-testnet (and show the faucet) instead of the health check"
        )]
        balance: Option<String>,
    },
}

pub async fn start(cfg: &CosmosConfig, root: &Path, http: &reqwest::Client) -> Result<()> {
    logger::banner(&format!("cosmos start ({})", cfg.name));

    if !cfg.is_local() {
        bail!("`{}` is a public testnet — it is not started locally; use `cosmos testnet` to check it", cfg.name);
    }

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

pub async fn check(cfg: &CosmosConfig, http: &reqwest::Client) -> Result<()> {
    logger::banner(&format!(
        "cosmos {} (chain_id {})",
        cfg.name,
        cfg.chain_id.as_str()
    ));

    match probe::get_json(http, &cfg.status_url()).await {
        Some(status) => {
            logger::status_line(cfg.name.as_str(), true, &cfg.rpc_url);

            let result = &status["result"];
            let network = result["node_info"]["network"].as_str().unwrap_or("?");
            let height = result["sync_info"]["latest_block_height"]
                .as_str()
                .unwrap_or("?");
            let catching_up = result["sync_info"]["catching_up"]
                .as_bool()
                .unwrap_or(false);
            let version = result["node_info"]["version"].as_str().unwrap_or("?");

            logger::detail(&format!("network   {network}"));
            logger::detail(&format!("height    {height}"));
            logger::detail(&format!(
                "synced    {}",
                if catching_up {
                    "no (catching up)"
                } else {
                    "yes"
                }
            ));
            logger::detail(&format!("cometbft  {version}"));

            if network != cfg.chain_id.as_str() {
                logger::warn(&format!(
                    "reported network '{network}' != configured chain_id '{}'",
                    cfg.chain_id.as_str()
                ));
            }
        }
        None => {
            logger::status_line(cfg.name.as_str(), false, &cfg.rpc_url);
            logger::warn("RPC /status unreachable (chain down, not started, or rate-limiting)");
        }
    }

    if let Some(info) = probe::get_json(http, &cfg.node_info_url()).await {
        let version = info["application_version"]["version"]
            .as_str()
            .unwrap_or("?");
        let app = info["application_version"]["app_name"]
            .as_str()
            .unwrap_or("?");
        logger::detail(&format!("app       {app} {version}"));
    }

    logger::detail(&format!("rest      {}", cfg.rest_url));
    logger::detail(&format!("grpc      {}", cfg.grpc_url));
    if let Some(faucet) = &cfg.faucet_url {
        logger::detail(&format!("faucet    {faucet}"));
        logger::hint("override any endpoint via COSMOS_TESTNET_{RPC,REST,GRPC,FAUCET}_URL in .env");
    }

    Ok(())
}

pub async fn balance(cfg: &CosmosConfig, http: &reqwest::Client, address: &str) -> Result<()> {
    logger::banner(&format!("cosmos {} balance ({address})", cfg.name));

    let url = format!("{}/cosmos/bank/v1beta1/balances/{address}", cfg.rest_url);
    match probe::get_json(http, &url).await {
        Some(value) => match value["balances"].as_array() {
            Some(balances) if !balances.is_empty() => {
                for coin in balances {
                    let denom = coin["denom"].as_str().unwrap_or("?");
                    let amount = coin["amount"].as_str().unwrap_or("?");
                    logger::detail(&format!("{amount} {denom}"));
                }
            }
            Some(_) => logger::warn("account has no balances — fund it via the faucet"),
            None => logger::warn(&format!("unexpected balances response from {url}")),
        },
        None => logger::warn(&format!("could not query balances at {url}")),
    }

    if let Some(faucet) = &cfg.faucet_url {
        logger::hint(&format!("faucet: {faucet}"));
    }

    Ok(())
}
