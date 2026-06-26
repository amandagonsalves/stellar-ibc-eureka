use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::logger;
use crate::tx::{self, TransferTx, TxCmd};

pub async fn run(cfg: &Config, root: &Path, http: &reqwest::Client) -> Result<()> {
    let sender = cfg.accounts.stellar_sender_address.clone();
    if sender.is_empty() {
        bail!("STELLAR_SENDER_ADDRESS is unset — run `interstellar start` first");
    }

    let receiver = cfg.accounts.cosmos_receiver_address.clone();
    if receiver.is_empty() {
        bail!("COSMOS_RECEIVER_ADDRESS is unset — run `interstellar start` first");
    }

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

    logger::ok("ics-20 round trip closed");

    Ok(())
}
