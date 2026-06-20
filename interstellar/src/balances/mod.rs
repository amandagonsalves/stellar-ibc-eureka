use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::shared::Chain;
use crate::{logger, probe, shared, tools};

#[derive(clap::Args)]
pub struct BalancesArgs {
    #[arg(
        long,
        help = "Address to read balances for (cosmos or stellar — chain inferred)"
    )]
    pub address: String,
    #[arg(
        long,
        default_value = "stake",
        help = "Token denom to read on the Stellar side"
    )]
    pub denom: String,
}

pub async fn run(
    cfg: &Config,
    root: &Path,
    http: &reqwest::Client,
    address: &str,
    denom: &str,
) -> Result<()> {
    match shared::chain_of(address) {
        Some(Chain::Cosmos) => {
            cosmos_balances(cfg, http, address).await;

            Ok(())
        }
        Some(Chain::Stellar) => {
            stellar_balances(cfg, root, address, denom);

            Ok(())
        }
        None => bail!("could not classify {address:?} as a cosmos or stellar address"),
    }
}

fn stellar_balances(cfg: &Config, root: &Path, address: &str, denom: &str) {
    logger::banner(&format!("balances — stellar {address} ({denom})"));

    let transfer = cfg.deployment.transfer_app.as_str();

    if transfer.is_empty() {
        logger::warn("TRANSFER_CONTRACT_ADDRESS unset — run `interstellar start` first");

        return;
    }

    logger::ok(&format!(
        "balance: {} {denom}",
        balance_of(cfg, root, address, denom)
    ));
}

async fn cosmos_balances(cfg: &Config, http: &reqwest::Client, address: &str) {
    logger::banner(&format!("balances — cosmos {address}"));

    let url = format!(
        "{}/cosmos/bank/v1beta1/balances/{address}",
        cfg.cosmos.rest_url
    );

    match probe::get_json(http, &url).await {
        Some(value) => match value["balances"].as_array() {
            Some(coins) if !coins.is_empty() => {
                for coin in coins {
                    let coin_denom = coin["denom"].as_str().unwrap_or("?");
                    let amount = coin["amount"].as_str().unwrap_or("?");
                    logger::detail(&format!("{amount} {coin_denom}"));
                }
            }
            _ => logger::detail("(no balances)"),
        },
        None => logger::warn(&format!("could not query {url}")),
    }
}

fn balance_of(cfg: &Config, root: &Path, who: &str, denom: &str) -> String {
    let source = if !cfg.accounts.stellar_sender_identity.is_empty() {
        cfg.accounts.stellar_sender_identity.as_str()
    } else {
        cfg.stellar.cli_identity.as_str()
    };

    let result = tools::stellar::capture_quiet(
        root,
        &[
            "contract",
            "invoke",
            "--id",
            cfg.deployment.transfer_app.as_str(),
            "--source",
            source,
            "--rpc-url",
            cfg.stellar.rpc_url.as_str(),
            "--network-passphrase",
            cfg.stellar.network_passphrase.as_str(),
            "--",
            "balance_of",
            "--who",
            who,
            "--denom",
            denom,
        ],
    );

    match result {
        Ok(out) => out.trim().trim_matches('"').to_string(),
        Err(_) => "(unavailable)".to_string(),
    }
}
