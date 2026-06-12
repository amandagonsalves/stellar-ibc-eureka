use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::{logger, probe, run};

pub async fn run(cfg: &Config, root: &Path, http: &reqwest::Client, denom: &str) -> Result<()> {
    logger::banner(&format!("balances ({denom})"));

    stellar_side(cfg, root, denom);
    cosmos_side(cfg, http).await;

    Ok(())
}

fn stellar_side(cfg: &Config, root: &Path, denom: &str) {
    logger::step("Stellar (ibc-transfer) — sender debited, escrow locks the sent denom");

    let transfer = cfg.deployment.transfer_app.as_str();

    if transfer.is_empty() {
        logger::warn("TRANSFER_CONTRACT_ADDRESS unset — run `eurekastellar start` first");

        return;
    }

    let sender = cfg.accounts.stellar_sender_address.as_str();

    if !sender.is_empty() {
        logger::ok(&format!("sender {sender}"));
        logger::detail(&format!(
            "balance: {} {denom}",
            balance_of(cfg, root, sender, denom)
        ));
    }

    logger::ok(&format!("escrow {transfer} (ibc-transfer)"));
    logger::detail(&format!(
        "balance: {} {denom}  (locked on-chain)",
        balance_of(cfg, root, transfer, denom)
    ));
}

async fn cosmos_side(cfg: &Config, http: &reqwest::Client) {
    logger::step("Cosmos (receiver bank balances) — the ibc/… voucher lands here");

    let receiver = cfg.accounts.cosmos_receiver_address.as_str();

    if receiver.is_empty() {
        logger::warn("COSMOS_RECEIVER_ADDRESS unset — run `eurekastellar start` first");

        return;
    }

    logger::ok(&format!("receiver {receiver}"));

    let url = format!(
        "{}/cosmos/bank/v1beta1/balances/{receiver}",
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
            _ => logger::detail("(no balances yet — no voucher for this route)"),
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

    let result = run::capture_quiet(
        root,
        "stellar",
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
