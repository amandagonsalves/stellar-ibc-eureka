use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};

use crate::config::Config;
use crate::contracts::{self, config::ContractsConfig};
use crate::{logger, shared};

pub struct TransferArgs {
    pub denom: String,
    pub amount: i128,
    pub receiver: String,
    pub memo: String,
    pub timeout_secs: u64,
    pub mint: bool,
}

pub fn stellar_to_cosmos(cfg: &Config, root: &Path, args: &TransferArgs) -> Result<()> {
    logger::banner("transfer stellar (Stellar → Cosmos ICS-20)");

    let transfer_app = cfg.deployment.transfer_app.as_str();
    let source_client = cfg.deployment.cosmos_client_id.as_str();
    let sender = cfg.deployment.deployer_address.as_str();

    if transfer_app.is_empty() {
        bail!("TRANSFER_CONTRACT_ADDRESS is not set — run `stellaribc start` first");
    }

    if source_client.is_empty() {
        bail!("COSMOS_CLIENT_ID is not set — run `stellaribc clients cosmos` first");
    }

    if sender.is_empty() {
        bail!("DEPLOYER_ADDRESS is not set");
    }

    let cc = ContractsConfig::from(cfg);
    let amount = args.amount.to_string();
    let timeout = transfer_timeout(args.timeout_secs)?.to_string();

    if args.mint {
        logger::step(&format!(
            "mint {} {} to {sender}",
            args.amount, args.denom
        ));

        contracts::invoke(
            &cc,
            root,
            transfer_app,
            &[
                "mint",
                "--to",
                sender,
                "--denom",
                &args.denom,
                "--amount",
                &amount,
            ],
        )?;
    }

    logger::step(&format!(
        "initiate_transfer {} {} → {} (client {source_client}, timeout {timeout})",
        args.amount, args.denom, args.receiver
    ));

    contracts::invoke(
        &cc,
        root,
        transfer_app,
        &[
            "initiate_transfer",
            "--sender",
            sender,
            "--source_client_id",
            source_client,
            "--denom",
            &args.denom,
            "--amount",
            &amount,
            "--receiver",
            &args.receiver,
            "--timeout_timestamp",
            &timeout,
            "--memo",
            &args.memo,
        ],
    )?;

    logger::ok("transfer initiated — hermes will relay recv → ack");
    logger::hint("watch the relay: docker compose logs -f hermes");

    Ok(())
}

pub fn cosmos_to_stellar(_cfg: &Config, _root: &Path, _args: &TransferArgs) -> Result<()> {
    shared::pending(
        "transfer cosmos (Cosmos → Stellar ICS-20)",
        "M4: needs a MsgTransfer over the v2 client on simd-1 plus the Cosmos→Stellar recv path (CT3); implement after the Stellar→Cosmos relay (M3) lands.",
    );

    Ok(())
}

fn transfer_timeout(secs: u64) -> Result<u64> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    Ok(now + secs)
}
