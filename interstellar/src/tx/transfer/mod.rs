use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};

use crate::config::Config;
use crate::cosmos::config::COMPOSE_SERVICE;
use crate::tx::contracts::{self, config::ContractsConfig};
use crate::{logger, shared, tools};

pub struct TransferParams {
    pub denom: String,
    pub amount: i128,
    pub receiver: String,
    pub sender: String,
    pub memo: String,
    pub timeout_secs: u64,
    pub mint: bool,
}

pub fn stellar_to_cosmos(cfg: &Config, root: &Path, args: &TransferParams) -> Result<()> {
    logger::banner("transfer stellar (Stellar → Cosmos ICS-20)");

    let transfer_app = cfg.deployment.transfer_app.as_str();
    let source_client = cfg.deployment.cosmos_client_id.as_str();

    let sender = if !args.sender.is_empty() {
        args.sender.as_str()
    } else if !cfg.accounts.stellar_sender_address.is_empty() {
        cfg.accounts.stellar_sender_address.as_str()
    } else {
        cfg.deployment.deployer_address.as_str()
    };

    let sender_identity = if !cfg.accounts.stellar_sender_identity.is_empty() {
        cfg.accounts.stellar_sender_identity.as_str()
    } else {
        cfg.stellar.cli_identity.as_str()
    };

    if transfer_app.is_empty() {
        bail!("TRANSFER_CONTRACT_ADDRESS is not set — run `interstellar start` first");
    }

    if source_client.is_empty() {
        bail!("COSMOS_CLIENT_ID is not set — run `interstellar clients cosmos` first");
    }

    if sender.is_empty() {
        bail!("no Stellar sender — run `interstellar start` to provision STELLAR_SENDER_ADDRESS (or set DEPLOYER_ADDRESS)");
    }

    let receiver = if !args.receiver.is_empty() {
        args.receiver.clone()
    } else if !cfg.cosmos.receiver_address.trim().is_empty() {
        let addr = cfg.cosmos.receiver_address.trim().to_string();
        logger::detail(&format!("using COSMOS_RECEIVER_ADDRESS → {addr}"));
        addr
    } else {
        cosmos_relayer_address(cfg, root)?
    };

    let cc = ContractsConfig::from(cfg);
    let amount = args.amount.to_string();
    let timeout = transfer_timeout(args.timeout_secs)?.to_string();
    let memo_json = format!("\"{}\"", args.memo);

    if args.mint {
        logger::step(&format!("mint {} {} to {sender}", args.amount, args.denom));

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
        "initiate_transfer {} {} → {receiver} (client {source_client}, timeout {timeout})",
        args.amount, args.denom
    ));

    contracts::invoke_as(
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
            &receiver,
            "--timeout_timestamp",
            &timeout,
            "--memo",
            &memo_json,
        ],
        sender_identity,
    )?;

    logger::ok("transfer initiated — hermes will relay recv → ack");
    logger::hint("watch the relay: docker compose logs -f hermes");

    Ok(())
}

pub fn cosmos_to_stellar(_cfg: &Config, _root: &Path, _args: &TransferParams) -> Result<()> {
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

fn cosmos_relayer_address(cfg: &Config, root: &Path) -> Result<String> {
    let key_name = cfg.cosmos.key_name.as_str();

    logger::detail(&format!(
        "no --receiver given, deriving the cosmos `{key_name}` address"
    ));

    let out = tools::docker::capture_all(
        root,
        &[
            "compose",
            "exec",
            "-T",
            COMPOSE_SERVICE,
            "simd",
            "keys",
            "show",
            key_name,
            "-a",
            "--keyring-backend",
            "test",
            "--home",
            "/root/.simapp",
        ],
    )?;

    let address = out
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("cosmos"))
        .map(str::to_string);

    match address {
        Some(addr) => Ok(addr),
        None => {
            bail!("could not derive the cosmos `{key_name}` address — pass --receiver explicitly")
        }
    }
}
