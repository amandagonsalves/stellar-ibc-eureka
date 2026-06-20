mod accounts;
mod api;
mod balances;
mod config;
mod cosmos;
mod demo;
mod gateway;
mod hermes;
mod logger;
mod logs;
mod ops;
mod probe;
mod query;
mod repo;
mod run;
mod service;
mod shared;
mod stellar;
mod test;
mod tools;
mod tx;

use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};

use api::ApiCmd;
use balances::BalancesArgs;
use config::Config;
use cosmos::CosmosCmd;
use demo::DemoArgs;
use gateway::GatewayCmd;
use hermes::HermesCmd;
use logs::LogsArgs;
use ops::{DownArgs, StartArgs, UpArgs};
use query::QueryArgs;
use shared::Chain;
use test::TestArgs;
use tx::TxCmd;

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
    #[command(about = "Show chain/service health, deployed contracts, and created clients")]
    Status,
    #[command(about = "Bring the stack up via docker compose (cosmos + api + gateway)")]
    Up(UpArgs),
    #[command(about = "Stop the stack via docker compose")]
    Down(DownArgs),
    #[command(
        about = "Full start: pull images, start chains, deploy contracts, upload wasm, import keys"
    )]
    Start(StartArgs),
    #[command(about = "Cosmos chain: start/stop the local devnet or point at a testnet")]
    Cosmos {
        #[command(subcommand)]
        cmd: CosmosCmd,
    },
    #[command(about = "Write operations: clients (create, counterparty), contracts, transfer")]
    Tx {
        #[command(subcommand)]
        cmd: TxCmd,
    },
    #[command(about = "Relayer (hermes): build image, import keys, start packet relay")]
    Hermes {
        #[command(subcommand)]
        cmd: HermesCmd,
    },
    #[command(about = "Gateway service: build image, gRPC queries")]
    Gateway {
        #[command(subcommand)]
        cmd: GatewayCmd,
    },
    #[command(about = "API service: build image")]
    Api {
        #[command(subcommand)]
        cmd: ApiCmd,
    },
    #[command(about = "Read client states on either or both networks")]
    Query(QueryArgs),
    #[command(
        about = "End-to-end demo: start, client bootstrap, balances before, transfer, balances after"
    )]
    Demo(DemoArgs),
    #[command(
        about = "Run the ICS integration flows (clients, counterparty, transfer, query) against a running stack"
    )]
    Test(TestArgs),
    #[command(about = "Show the dedicated sender + receiver accounts on each chain")]
    Accounts,
    #[command(about = "Read balances for an address (cosmos or stellar — chain inferred)")]
    Balances(BalancesArgs),
    #[command(about = "Show the staged round-trip relay lines from the gateway + hermes logs")]
    Logs(LogsArgs),
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
        Command::Install => ops::install::run(root)?,
        Command::Check => ops::check::run(root, &ops::config::OpsConfig::from(&cfg), &http).await?,
        Command::Status => ops::status::run(&ops::config::OpsConfig::from(&cfg)).await?,
        Command::Up(args) => ops::stack::up(root, args.cosmos, args.stellar)?,
        Command::Down(args) => ops::stack::down(root, args.volumes)?,
        Command::Start(args) => {
            ops::start::run(
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

        Command::Cosmos { cmd } => match cmd {
            CosmosCmd::Start => cosmos::start(&cfg.cosmos, root, &http).await?,
            CosmosCmd::Stop => cosmos::stop(&cfg.cosmos, root)?,
            CosmosCmd::Status => cosmos::check(&cfg.cosmos, &http).await?,
            CosmosCmd::Testnet { balance } => {
                let tcfg = cosmos::config::CosmosConfig::testnet();
                match balance {
                    Some(address) => cosmos::balance(&tcfg, &http, &address).await?,
                    None => cosmos::check(&tcfg, &http).await?,
                }
            }
        },

        Command::Tx { cmd } => tx::run(&cfg, root, &http, cmd).await?,

        Command::Hermes { cmd } => match cmd {
            HermesCmd::Start { pull } => hermes::container::start(&cfg.hermes, root, pull)?,
            HermesCmd::Stop => hermes::container::stop(root)?,
            HermesCmd::Restart { pull } => hermes::container::restart(&cfg.hermes, root, pull)?,
            HermesCmd::KeysImport => hermes::keys::import(&cfg, root)?,
        },

        Command::Gateway { cmd } => match cmd {
            GatewayCmd::Start { pull } => gateway::container::start(&cfg.gateway, root, pull)?,
            GatewayCmd::Stop => gateway::container::stop(root)?,
            GatewayCmd::Restart { pull } => gateway::container::restart(&cfg.gateway, root, pull)?,
            GatewayCmd::Query => gateway::query::run()?,
        },

        Command::Api { cmd } => match cmd {
            ApiCmd::Start { pull } => api::container::start(&cfg.api, root, pull)?,
            ApiCmd::Stop => api::container::stop(root)?,
            ApiCmd::Restart { pull } => api::container::restart(&cfg.api, root, pull)?,
        },

        Command::Query(args) => query::run(&cfg, &http, args).await?,

        Command::Demo(args) => {
            if !args.transfer {
                logger::warn("no scenario flag given — running the default --transfer scenario");
            }

            let ta = tx::transfer::TransferParams {
                denom: args.denom,
                amount: args.amount,
                receiver: args.receiver,
                sender: String::new(),
                memo: args.memo,
                timeout_secs: args.timeout_secs,
                mint: !args.no_mint,
            };

            demo::run(
                root,
                &http,
                demo::DemoParams {
                    from_cosmos: matches!(args.from, Chain::Cosmos),
                    skip_start: args.skip_start,
                    force_redeploy: args.force_redeploy,
                    wait_secs: args.wait_secs,
                    transfer: ta,
                },
            )
            .await?
        }

        Command::Test(args) => test::run(root, &http, args).await?,
        Command::Accounts => accounts::show(&cfg),
        Command::Balances(args) => {
            balances::run(&cfg, root, &http, &args.address, &args.denom).await?
        }
        Command::Logs(args) => logs::run(root, &args.since)?,
    }

    Ok(())
}
