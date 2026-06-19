use std::path::Path;

use anyhow::Result;

use crate::clients::{self, config::ClientsConfig};
use crate::config::Config;
use crate::transfer::{self, TransferParams};
use crate::{balances, hermes, logger, logs, ops};

#[derive(clap::Args)]
pub struct DemoArgs {
    #[arg(
        long,
        help = "Run the ICS-20 transfer round-trip demo (the default and only scenario today)"
    )]
    pub transfer: bool,
    #[arg(
        value_enum,
        default_value = "stellar",
        help = "Source chain to transfer from"
    )]
    pub from: crate::shared::Chain,
    #[arg(long, default_value = "stake", help = "Token denom to transfer")]
    pub denom: String,
    #[arg(long, default_value_t = 1000, help = "Amount to transfer")]
    pub amount: i128,
    #[arg(
        long,
        default_value = "",
        help = "Receiver address on the destination chain (default: the relayer key on the destination)"
    )]
    pub receiver: String,
    #[arg(long, default_value = "", help = "Optional transfer memo")]
    pub memo: String,
    #[arg(
        long,
        default_value_t = 600,
        help = "Transfer timeout in seconds from now"
    )]
    pub timeout_secs: u64,
    #[arg(long, help = "Skip minting the amount to the sender first")]
    pub no_mint: bool,
    #[arg(
        long,
        help = "Skip the full `start` step (assume the stack is already up)"
    )]
    pub skip_start: bool,
    #[arg(
        long,
        help = "Force-redeploy contracts and re-bootstrap clients during start"
    )]
    pub force_redeploy: bool,
    #[arg(
        long,
        default_value_t = 120,
        help = "Max seconds to watch the relay round trip (exits early when it closes) before reading balances-after"
    )]
    pub wait_secs: u64,
}

pub struct DemoParams {
    pub from_cosmos: bool,
    pub skip_start: bool,
    pub force_redeploy: bool,
    pub wait_secs: u64,
    pub transfer: TransferParams,
}

pub async fn run(root: &Path, http: &reqwest::Client, args: DemoParams) -> Result<()> {
    logger::banner("demo — ICS-20 transfer round trip");
    logger::detail(
        "steps: start → client bootstrap → balances (before) → transfer → balances (after)",
    );

    let mut cfg = Config::load(root);

    if args.skip_start {
        logger::step("1/5 start — skipped (--skip-start; assuming the stack is already up)");
    } else {
        logger::step("1/5 start — stack up, deploy contracts, upload wasm, import keys");
        {
            let _tick = logger::ticker("start: bringing up the stack");
            ops::start::run(
                &cfg,
                root,
                http,
                false,
                false,
                false,
                false,
                false,
                args.force_redeploy,
            )
            .await?;
        }
        cfg = Config::load(root);
    }

    logger::step("2/5 client bootstrap — create both clients, register both counterparties");
    {
        let _tick = logger::ticker("bootstrap: creating clients + counterparties");
        clients::bootstrap(&ClientsConfig::from(&cfg), root, http, true).await?;
    }
    cfg = Config::load(root);

    logger::step("starting the hermes relayer (so the packet round trip relays)");
    {
        let _tick = logger::ticker("starting hermes relayer");
        hermes::container::start(&cfg.hermes, root, false)?;
    }

    logger::step("3/5 balances — before the transfer");
    {
        let _tick = logger::ticker("reading balances");
        balances::run(&cfg, root, http, &args.transfer.denom).await?;
    }

    logger::step("4/5 transfer — originate the ICS-20 packet");
    {
        let _tick = logger::ticker("submitting the transfer");
        if args.from_cosmos {
            transfer::cosmos_to_stellar(&cfg, root, &args.transfer)?;
        } else {
            transfer::stellar_to_cosmos(&cfg, root, &args.transfer)?;
        }
    }

    let since = format!("{}s", args.wait_secs + 30);
    {
        let _tick = logger::ticker("relaying recv → ack");
        logs::watch(root, &since, args.wait_secs).await?;
    }

    logger::step("5/5 balances — after the transfer");
    {
        let _tick = logger::ticker("reading balances");
        balances::run(&cfg, root, http, &args.transfer.denom).await?;
    }

    logger::ok("demo complete");
    logger::hint(
        "inspect the relay: docker compose logs hermes   ·   on-chain: interstellar status",
    );

    Ok(())
}
