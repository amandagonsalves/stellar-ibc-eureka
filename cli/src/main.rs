mod api;
mod clients;
mod config;
mod contracts;
mod gateway;
mod hermes;
mod logger;
mod ops;
mod probe;
mod repo;
mod run;
mod shared;
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
    long_about = "A caribic-style front door to the ci/flows scripts and services, grouped by \
component: ops (install/doctor/status/up/down/bootstrap), clients, hermes, gateway, api, \
contracts, and tx.",
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
    Doctor,
    #[command(about = "Show chain/service health, deployed contracts, and created clients")]
    Status,
    #[command(about = "Bring the stack up via docker compose (osmosis + api + gateway)")]
    Up(UpArgs),
    #[command(about = "Stop the stack via docker compose")]
    Down(DownArgs),
    #[command(alias = "f0", about = "Full bootstrap: images, chains, contracts, wasm, keys (F0)")]
    Bootstrap(BootstrapArgs),
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
    #[arg(long, help = "Redeploy contracts even if IBC_CONTRACT_ID is already set")]
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
enum ClientsCmd {
    #[command(about = "Create the Cosmos (Tendermint) client on Stellar (F1.1)")]
    Cosmos {
        #[arg(long, help = "Create a new client even if COSMOS_CLIENT_ID is already set")]
        force: bool,
    },
    #[command(about = "Create the Stellar (08-wasm) client on Cosmos (F1.2)")]
    Stellar {
        #[arg(long, help = "Create a new client even if STELLAR_CLIENT_ID is already set")]
        force: bool,
    },
    #[command(about = "Register a counterparty: stellar = F1.3, cosmos = F1.4")]
    Counterparty {
        #[arg(value_enum, help = "Which side to register the counterparty on")]
        chain: Chain,
    },
    #[command(about = "List clients created on the Stellar router")]
    List,
}

#[derive(Subcommand)]
enum HermesCmd {
    #[command(about = "Build the hermes docker image (from the hermes-relayer repo)")]
    BuildImage,
    #[command(about = "Push the hermes docker image to the registry")]
    PushImage {
        #[arg(long, help = "Rebuild the image before pushing")]
        rebuild: bool,
    },
    #[command(about = "Start the hermes relayer container")]
    Start {
        #[arg(long, help = "Rebuild the image before starting")]
        rebuild: bool,
    },
    #[command(about = "Stop the hermes relayer container")]
    Stop,
    #[command(about = "Restart the hermes relayer container")]
    Restart {
        #[arg(long, help = "Rebuild the image and recreate the container")]
        rebuild: bool,
    },
    #[command(about = "Import the relayer keys (must equal the router admin key)")]
    KeysImport,
}

#[derive(Subcommand)]
enum GatewayCmd {
    #[command(about = "Build the gateway docker image")]
    BuildImage,
    #[command(about = "Push the gateway docker image to the registry")]
    PushImage {
        #[arg(long, help = "Rebuild the image before pushing")]
        rebuild: bool,
    },
    #[command(about = "Start the gateway container")]
    Start {
        #[arg(long, help = "Rebuild the image before starting")]
        rebuild: bool,
    },
    #[command(about = "Stop the gateway container")]
    Stop,
    #[command(about = "Restart the gateway container")]
    Restart {
        #[arg(long, help = "Rebuild the image and recreate the container")]
        rebuild: bool,
    },
    #[command(about = "Direct gateway gRPC reads")]
    Query,
}

#[derive(Subcommand)]
enum ApiCmd {
    #[command(about = "Build the api docker image")]
    BuildImage,
    #[command(about = "Push the api docker image to the registry")]
    PushImage {
        #[arg(long, help = "Rebuild the image before pushing")]
        rebuild: bool,
    },
    #[command(about = "Start the api container")]
    Start {
        #[arg(long, help = "Rebuild the image before starting")]
        rebuild: bool,
    },
    #[command(about = "Stop the api container")]
    Stop,
    #[command(about = "Restart the api container")]
    Restart {
        #[arg(long, help = "Rebuild the image and recreate the container")]
        rebuild: bool,
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
        #[arg(long, help = "Redeploy even if IBC_CONTRACT_ID is already set")]
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
        Command::Doctor => ops::doctor::run(root, &cfg, &http).await?,
        Command::Status => ops::status::run(&cfg, &http).await?,
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

        Command::Clients { cmd } => match cmd {
            ClientsCmd::Cosmos { force } => clients::cosmos::run(&cfg, root, &http, force).await?,
            ClientsCmd::Stellar { force } => clients::stellar::run(&cfg, root, &http, force).await?,
            ClientsCmd::Counterparty { chain } => clients::counterparty::run(root, chain.as_str())?,
            ClientsCmd::List => clients::list::run(&cfg, &http).await?,
        },

        Command::Hermes { cmd } => match cmd {
            HermesCmd::BuildImage => hermes::image::build(&cfg, root)?,
            HermesCmd::PushImage { rebuild } => hermes::image::push(&cfg, root, rebuild)?,
            HermesCmd::Start { rebuild } => hermes::container::start(&cfg, root, rebuild)?,
            HermesCmd::Stop => hermes::container::stop(root)?,
            HermesCmd::Restart { rebuild } => hermes::container::restart(&cfg, root, rebuild)?,
            HermesCmd::KeysImport => hermes::keys::import(&cfg, root)?,
        },

        Command::Gateway { cmd } => match cmd {
            GatewayCmd::BuildImage => gateway::image::build(&cfg, root)?,
            GatewayCmd::PushImage { rebuild } => gateway::image::push(&cfg, root, rebuild)?,
            GatewayCmd::Start { rebuild } => gateway::container::start(&cfg, root, rebuild)?,
            GatewayCmd::Stop => gateway::container::stop(root)?,
            GatewayCmd::Restart { rebuild } => gateway::container::restart(&cfg, root, rebuild)?,
            GatewayCmd::Query => gateway::query::run()?,
        },

        Command::Api { cmd } => match cmd {
            ApiCmd::BuildImage => api::image::build(&cfg, root)?,
            ApiCmd::PushImage { rebuild } => api::image::push(&cfg, root, rebuild)?,
            ApiCmd::Start { rebuild } => api::container::start(&cfg, root, rebuild)?,
            ApiCmd::Stop => api::container::stop(root)?,
            ApiCmd::Restart { rebuild } => api::container::restart(&cfg, root, rebuild)?,
        },

        Command::Contracts { cmd } => match cmd {
            ContractsCmd::Build => contracts::build::run(root)?,
            ContractsCmd::Upload { wasm } => contracts::upload::run(&cfg, root, &wasm)?,
            ContractsCmd::Deploy { wasm, ctor } => contracts::deploy::run(&cfg, root, &wasm, &ctor)?,
            ContractsCmd::Invoke { id, call } => contracts::invoke::run(&cfg, root, &id, &call)?,
            ContractsCmd::DeployAll {
                force,
                attestation,
                tendermint,
            } => contracts::deploy_all::run(&cfg, root, force, attestation, tendermint)?,
            ContractsCmd::UploadWasm => contracts::wasm::upload(&cfg, root, &http).await?,
        },

        Command::Tx { cmd } => match cmd {
            TxCmd::Clients { cmd } => match cmd {
                TxClientsCmd::Create => tx::clients::create()?,
                TxClientsCmd::Update => tx::clients::update()?,
            },
            TxCmd::Msg { cmd } => match cmd {
                TxMsgCmd::RegisterCounterparty { chain } => {
                    tx::msg::register_counterparty(root, chain.as_str())?
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
