mod accounts;
mod check;
mod config;
mod install;
mod logger;
mod probe;
mod repo;
mod run;
mod service;
mod services;
mod shared;
mod stack;
mod start;
mod tests;
mod tools;
mod tx;

use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};

use config::Config;
use services::hermes::{self, HermesCmd};
use services::ServicesCmd;
use tests::TestArgs;
use tx::TxCmd;

use crate::shared::{DownArgs, StartArgs, UpArgs};

#[derive(Parser)]
#[command(
    name = "interstellar",
    version,
    about = "Orchestrator for the Stellar<->Cosmos IBC v2 bridge",
    long_about = "A caribic-style orchestrator for the Stellar<->Cosmos bridge: ops \
(install/check/status/up/down/start), tx (clients/contracts/transfer writes), query + balances (reads), and the cosmos/hermes/gateway/api service groups. Drives docker, the stellar CLI, and the api directly — no shell scripts.",
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Install the interstellar binary to the cargo bin dir")]
    Install,
    #[command(about = "Check prerequisites, configuration, and service health")]
    Check,
    #[command(about = "Bring the stack up via docker compose (cosmos + api + gateway)")]
    Up(UpArgs),
    #[command(about = "Stop the stack via docker compose")]
    Down(DownArgs),
    #[command(
        about = "Full start: pull images, start chains, deploy contracts, upload wasm, import keys"
    )]
    Start(StartArgs),
    #[command(about = "Write operations: clients (create, counterparty), contracts, transfer")]
    Tx {
        #[command(subcommand)]
        cmd: TxCmd,
    },
    #[command(
        about = "Service lifecycle + images: pull/up/restart/down/build/push (api, gateway, hermes, cosmos)"
    )]
    Services {
        #[command(subcommand)]
        cmd: ServicesCmd,
    },
    #[command(about = "Relayer (hermes): import the relayer keys")]
    Hermes {
        #[command(subcommand)]
        cmd: HermesCmd,
    },
    Test(TestArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    logger::init();
    let cli = Cli::parse();
    let root = repo::find_root()?;
    let root = root.as_path();
    let cfg = Config::load(root);
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?;

    match cli.command {
        Command::Install => install::run(root)?,
        Command::Check => check::run(root, &cfg, &http).await?,
        Command::Up(args) => stack::up(root, args.cosmos, args.stellar)?,
        Command::Down(args) => stack::down(root, args.volumes)?,
        Command::Start(args) => {
            start::run(
                &cfg,
                root,
                &http,
                args.skip_images,
                args.skip_contracts,
                args.skip_wasm,
                args.skip_keys,
                args.skip_accounts,
                args.force_redeploy,
            )
            .await?
        }

        Command::Tx { cmd } => tx::run(&cfg, root, &http, cmd).await?,

        Command::Services { cmd } => services::run(&cfg, root, cmd)?,

        Command::Hermes { cmd } => match cmd {
            HermesCmd::KeysImport => hermes::keys::import(&cfg, root)?,
        },

        Command::Test(args) => tests::run(root, &http, args).await?,
    }

    Ok(())
}
