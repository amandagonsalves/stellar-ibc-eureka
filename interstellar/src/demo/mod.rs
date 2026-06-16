use std::path::Path;

use anyhow::Result;

use crate::clients::{self, config::ClientsConfig};
use crate::config::Config;
use crate::transfer::{self, TransferArgs};
use crate::{balances, hermes, logger, logs, ops};

pub struct DemoArgs {
    pub from_cosmos: bool,
    pub skip_start: bool,
    pub force_redeploy: bool,
    pub wait_secs: u64,
    pub transfer: TransferArgs,
}

pub async fn run(root: &Path, http: &reqwest::Client, args: DemoArgs) -> Result<()> {
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
