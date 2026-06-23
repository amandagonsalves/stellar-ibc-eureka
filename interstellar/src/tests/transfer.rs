use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::tx::{self, TransferTx, TxCmd};
use crate::{logger, logs, probe};

pub async fn run(cfg: &Config, root: &Path, http: &reqwest::Client) -> Result<()> {
    let sender = cfg.accounts.stellar_sender_address.clone();
    if sender.is_empty() {
        bail!("STELLAR_SENDER_ADDRESS is unset — run `interstellar start` first");
    }

    let receiver = cfg.accounts.cosmos_receiver_address.clone();
    if receiver.is_empty() {
        bail!("COSMOS_RECEIVER_ADDRESS is unset — run `interstellar start` first");
    }

    let before = voucher_total(cfg, http, &receiver).await;

    tx::run(
        cfg,
        root,
        http,
        TxCmd::Transfer(TransferTx {
            from: sender,
            to: receiver.clone(),
            amount: 1000,
            denom: "stake".to_string(),
            timeout_secs: 600,
            no_mint: false,
        }),
    )
    .await?;

    let closed = logs::watch(root, "180s", 120).await?;
    if !closed {
        bail!("relay round trip did not close within the watch window");
    }

    let after = voucher_total(cfg, http, &receiver).await;
    if after <= before {
        bail!("cosmos receiver voucher did not increase (before={before}, after={after})");
    }

    logger::ok(&format!(
        "ics-20 round trip closed — voucher {before} → {after}"
    ));

    Ok(())
}

async fn voucher_total(cfg: &Config, http: &reqwest::Client, receiver: &str) -> u128 {
    let url = format!(
        "{}/cosmos/bank/v1beta1/balances/{receiver}",
        cfg.cosmos.rest_url
    );

    let Some(value) = probe::get_json(http, &url).await else {
        return 0;
    };

    value["balances"]
        .as_array()
        .map(|coins| {
            coins
                .iter()
                .filter(|coin| {
                    coin["denom"]
                        .as_str()
                        .is_some_and(|d| d.starts_with("ibc/"))
                })
                .filter_map(|coin| coin["amount"].as_str())
                .filter_map(|amount| amount.parse::<u128>().ok())
                .sum()
        })
        .unwrap_or(0)
}
