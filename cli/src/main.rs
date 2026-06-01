mod api;
mod clients;
mod config;
mod contracts;
mod gateway;
mod hermes;
mod logger;
mod ops;
mod osmosis;
mod probe;
mod repo;
mod run;
mod shared;
mod stellar;
mod tx;

use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use config::Config;

#[derive(Parser)]
#[command(
    name = "stellaribc",
    version,
    about = "Orchestrator for the Stellar<->Cosmos IBC v2 bridge",
    long_about = "A caribic-style orchestrator for the Stellar<->Cosmos bridge, grouped by \
component: ops (install/doctor/status/up/down/bootstrap), clients, hermes, gateway, api, \
contracts, and tx. Drives docker, the stellar CLI, and the api directly — no shell scripts.",
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Install the stellaribc binary to the cargo bin dir")]
    Install,
    #[command(about = "Check prerequisites, configuration, and service health")]
    Check,
    #[command(about = "Show chain/service health, deployed contracts, and created clients")]
    Status,
    #[command(about = "Bring the stack up via docker compose (osmosis + api + gateway)")]
    Up(UpArgs),
    #[command(about = "Stop the stack via docker compose")]
    Down(DownArgs),
    #[command(
        about = "Full bootstrap: build images, start chains, deploy contracts, upload wasm, import keys"
    )]
    Bootstrap(BootstrapArgs),
    #[command(about = "Osmosis chain: start/stop the local devnet or point at a testnet")]
    Osmosis {
        #[command(subcommand)]
        cmd: OsmosisCmd,
    },
    #[command(about = "Client lifecycle: create on each chain, register counterparties, list")]
    Clients {
        #[command(subcommand)]
        cmd: ClientsCmd,
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
    #[command(about = "Soroban contracts + light-client wasm: deploy, upload")]
    Contracts {
        #[command(subcommand)]
        cmd: ContractsCmd,
    },
    #[command(about = "Low-level tx surface: clients, msg, query")]
    Tx {
        #[command(subcommand)]
        cmd: TxCmd,
    },
}

#[derive(clap::Args)]
struct UpArgs {
    #[arg(long, help = "Start only the Cosmos chain (osmosis)")]
    cosmos: bool,
    #[arg(long, help = "Start only the Stellar-side services (api + gateway)")]
    stellar: bool,
}

#[derive(clap::Args)]
struct DownArgs {
    #[arg(long, help = "Also remove named volumes (wipes chain + key state)")]
    volumes: bool,
}

#[derive(clap::Args)]
struct BootstrapArgs {
    #[arg(long, help = "Skip building the docker images")]
    skip_images: bool,
    #[arg(long, help = "Skip the Soroban contract deploy")]
    skip_contracts: bool,
    #[arg(long, help = "Skip the light-client-wasm upload")]
    skip_wasm: bool,
    #[arg(long, help = "Skip importing the hermes relayer keys")]
    skip_keys: bool,
    #[arg(
        long,
        help = "Redeploy contracts even if ROUTER_CONTRACT_ADDRESS is already set"
    )]
    force_redeploy: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Chain {
    Stellar,
    Cosmos,
}

impl Chain {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stellar => "stellar",
            Self::Cosmos => "cosmos",
        }
    }
}

#[derive(Subcommand)]
enum OsmosisCmd {
    #[command(
        about = "Start the osmosis chain (local docker; no-op + reachability check for testnet)"
    )]
    Start,
    #[command(about = "Stop the local osmosis chain (no-op for testnet)")]
    Stop,
    #[command(about = "Show the osmosis chain network, endpoints, and health")]
    Status,
}

#[derive(Subcommand)]
enum ClientsCmd {
    #[command(about = "Create the Cosmos (Tendermint) client on Stellar")]
    Cosmos {
        #[arg(
            long,
            help = "Create a new client even if COSMOS_CLIENT_ID is already set"
        )]
        force: bool,
    },
    #[command(about = "Create the Stellar (08-wasm) client on Cosmos")]
    Stellar {
        #[arg(
            long,
            help = "Create a new client even if STELLAR_CLIENT_ID is already set"
        )]
        force: bool,
    },
    #[command(about = "Register a counterparty on the given side (stellar or cosmos)")]
    Counterparty {
        #[arg(value_enum, help = "Which side to register the counterparty on")]
        chain: Chain,
    },
    #[command(about = "List clients created on the Stellar router")]
    List,
}

#[derive(Subcommand)]
enum HermesCmd {
    #[command(about = "Start the hermes relayer container")]
    Start {
        #[arg(long, help = "Pull the latest image before starting")]
        pull: bool,
    },
    #[command(about = "Stop the hermes relayer container")]
    Stop,
    #[command(about = "Restart the hermes relayer container")]
    Restart {
        #[arg(long, help = "Pull the latest image and recreate the container")]
        pull: bool,
    },
    #[command(about = "Import the relayer keys (must equal the router admin key)")]
    KeysImport,
}

#[derive(Subcommand)]
enum GatewayCmd {
    #[command(about = "Start the gateway container")]
    Start {
        #[arg(long, help = "Pull the latest image before starting")]
        pull: bool,
    },
    #[command(about = "Stop the gateway container")]
    Stop,
    #[command(about = "Restart the gateway container")]
    Restart {
        #[arg(long, help = "Pull the latest image and recreate the container")]
        pull: bool,
    },
    #[command(about = "Direct gateway gRPC reads")]
    Query,
}

#[derive(Subcommand)]
enum ApiCmd {
    #[command(about = "Start the api container")]
    Start {
        #[arg(long, help = "Pull the latest image before starting")]
        pull: bool,
    },
    #[command(about = "Stop the api container")]
    Stop,
    #[command(about = "Restart the api container")]
    Restart {
        #[arg(long, help = "Pull the latest image and recreate the container")]
        pull: bool,
    },
}

#[derive(Subcommand)]
enum ContractsCmd {
    #[command(about = "Build all Soroban contracts to wasm")]
    Build,
    #[command(about = "Upload a contract wasm, print the wasm hash")]
    Upload {
        #[arg(long, help = "Path to the .wasm artifact")]
        wasm: String,
    },
    #[command(about = "Deploy a contract wasm (constructor args after `--`), print the id")]
    Deploy {
        #[arg(long, help = "Path to the .wasm artifact")]
        wasm: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        ctor: Vec<String>,
    },
    #[command(about = "Invoke a function on a deployed contract (fn + args after `--`)")]
    Invoke {
        #[arg(long, help = "Deployed contract id")]
        id: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        call: Vec<String>,
    },
    #[command(about = "Full orchestration: build + deploy + wire router + write .env")]
    DeployAll {
        #[arg(long, help = "Redeploy even if ROUTER_CONTRACT_ADDRESS is already set")]
        force: bool,
        #[arg(long, help = "Also deploy + register the attestation light client")]
        attestation: bool,
        #[arg(long, help = "Also deploy + register the tendermint light client")]
        tendermint: bool,
    },
    #[command(about = "Build + gov-upload the light-client-wasm to Cosmos, patch hermes config")]
    UploadWasm,
}

#[derive(Subcommand)]
enum TxCmd {
    #[command(about = "Client txs (create / update)")]
    Clients {
        #[command(subcommand)]
        cmd: TxClientsCmd,
    },
    #[command(about = "Packet / counterparty messages")]
    Msg {
        #[command(subcommand)]
        cmd: TxMsgCmd,
    },
    #[command(about = "Provable-path queries")]
    Query {
        #[command(subcommand)]
        cmd: TxQueryCmd,
    },
}

#[derive(Subcommand)]
enum TxClientsCmd {
    Create,
    Update,
}

#[derive(Subcommand)]
enum TxMsgCmd {
    #[command(about = "Register a counterparty (stellar / cosmos)")]
    RegisterCounterparty {
        #[arg(value_enum)]
        chain: Chain,
    },
    Recv,
    Ack,
    Timeout,
}

#[derive(Subcommand)]
enum TxQueryCmd {
    Commitment,
    Receipt,
    Ack,
    Header,
}

#[tokio::main]
async fn main() -> Result<()> {
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
        Command::Status => ops::status::run(&ops::config::OpsConfig::from(&cfg), &http).await?,
        Command::Up(args) => ops::stack::up(root, args.cosmos, args.stellar)?,
        Command::Down(args) => ops::stack::down(root, args.volumes)?,
        Command::Bootstrap(args) => {
            ops::bootstrap::run(
                &cfg,
                root,
                &http,
                args.skip_images,
                args.skip_contracts,
                args.skip_wasm,
                args.skip_keys,
                args.force_redeploy,
            )
            .await?
        }

        Command::Osmosis { cmd } => match cmd {
            OsmosisCmd::Start => osmosis::start(&cfg.osmosis, root, &http).await?,
            OsmosisCmd::Stop => osmosis::stop(&cfg.osmosis, root)?,
            OsmosisCmd::Status => osmosis::status(&cfg.osmosis, &http).await?,
        },

        Command::Clients { cmd } => {
            let cc = clients::config::ClientsConfig::from(&cfg);

            match cmd {
                ClientsCmd::Cosmos { force } => {
                    clients::cosmos::run(&cc, root, &http, force).await?
                }
                ClientsCmd::Stellar { force } => {
                    clients::stellar::run(&cc, root, &http, force).await?
                }
                ClientsCmd::Counterparty { chain } => clients::counterparty::run(chain.as_str())?,
                ClientsCmd::List => clients::list::run(&cc, &http).await?,
            }
        }

        Command::Hermes { cmd } => match cmd {
            HermesCmd::Start { pull } => hermes::container::start(&cfg.hermes, root, pull)?,
            HermesCmd::Stop => hermes::container::stop(root)?,
            HermesCmd::Restart { pull } => {
                hermes::container::restart(&cfg.hermes, root, pull)?
            }
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

        Command::Contracts { cmd } => {
            let cc = contracts::config::ContractsConfig::from(&cfg);

            match cmd {
                ContractsCmd::Build => contracts::build::run(root)?,
                ContractsCmd::Upload { wasm } => contracts::upload::run(&cc, root, &wasm)?,
                ContractsCmd::Deploy { wasm, ctor } => {
                    contracts::deploy::run(&cc, root, &wasm, &ctor)?
                }
                ContractsCmd::Invoke { id, call } => contracts::invoke::run(&cc, root, &id, &call)?,
                ContractsCmd::DeployAll {
                    force,
                    attestation,
                    tendermint,
                } => contracts::deploy_all::run(&cc, root, force, attestation, tendermint)?,
                ContractsCmd::UploadWasm => contracts::wasm::upload(&cc, root, &http).await?,
            }
        }

        Command::Tx { cmd } => match cmd {
            TxCmd::Clients { cmd } => match cmd {
                TxClientsCmd::Create => tx::clients::create()?,
                TxClientsCmd::Update => tx::clients::update()?,
            },
            TxCmd::Msg { cmd } => match cmd {
                TxMsgCmd::RegisterCounterparty { chain } => {
                    tx::msg::register_counterparty(chain.as_str())?
                }
                TxMsgCmd::Recv => tx::msg::recv()?,
                TxMsgCmd::Ack => tx::msg::ack()?,
                TxMsgCmd::Timeout => tx::msg::timeout()?,
            },
            TxCmd::Query { cmd } => match cmd {
                TxQueryCmd::Commitment => tx::query::commitment()?,
                TxQueryCmd::Receipt => tx::query::receipt()?,
                TxQueryCmd::Ack => tx::query::ack()?,
                TxQueryCmd::Header => tx::query::header()?,
            },
        },
    }

    Ok(())
}
