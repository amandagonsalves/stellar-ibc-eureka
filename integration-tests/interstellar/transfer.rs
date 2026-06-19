use std::path::Path;

use anyhow::{bail, Result};

use crate::clients::config::ClientsConfig;
use crate::config::Config;
use crate::transfer::TransferParams;
use crate::{clients, logger, logs, probe, transfer};

pub async fn run(cfg: &Config, root: &Path, http: &reqwest::Client) -> Result<()> {
    let cc = ClientsConfig::from(cfg);

    clients::bootstrap(&cc, root, http, false).await?;

    let fresh = Config::load(root);

    let receiver = fresh.accounts.cosmos_receiver_address.clone();
    if receiver.is_empty() {
        bail!("COSMOS_RECEIVER_ADDRESS is unset — run `interstellar start` first");
    }

    let before = voucher_total(&fresh, http, &receiver).await;

    let params = TransferParams {
        denom: "stake".to_string(),
        amount: 1000,
        receiver: receiver.clone(),
        memo: String::new(),
        timeout_secs: 600,
        mint: true,
    };

    transfer::stellar_to_cosmos(&fresh, root, &params)?;

    let closed = logs::watch(root, "180s", 120).await?;
    if !closed {
        bail!("relay round trip did not close within the watch window");
    }

    let after = voucher_total(&fresh, http, &receiver).await;
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
