mod accounts;
mod api;
mod balances;
mod clients;
mod config;
mod contracts;
mod cosmos;
mod demo;
mod gateway;
mod hermes;
mod logger;
mod logs;
mod ops;
mod probe;
mod repo;
mod run;
mod shared;
mod stellar;
mod transfer;
mod tx;

use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use config::Config;

#[derive(Parser)]
#[command(
    name = "interstellar",
    version,
    about = "Orchestrator for the Stellar<->Cosmos IBC v2 bridge",
    long_about = "A caribic-style orchestrator for the Stellar<->Cosmos bridge, grouped by \
component: ops (install/check/status/up/down/start), clients, hermes, gateway, api, \
contracts, and tx. Drives docker, the stellar CLI, and the api directly — no shell scripts.",
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
    #[command(about = "Originate an ICS-20 transfer from the given source chain")]
    Transfer(TransferArgs),
    #[command(
        about = "End-to-end demo: start, client bootstrap, balances before, transfer, balances after"
    )]
    Demo(DemoArgs),
    #[command(about = "Show the dedicated sender + receiver accounts on each chain")]
    Accounts,
    #[command(about = "Show the Cosmos receiver voucher and the Stellar sender + escrow balances")]
    Balances(BalancesArgs),
    #[command(about = "Show the staged round-trip relay lines from the gateway + hermes logs")]
    Logs(LogsArgs),
    #[command(about = "Low-level tx surface: clients, msg, query")]
    Tx {
        #[command(subcommand)]
        cmd: TxCmd,
    },
}

#[derive(clap::Args)]
struct BalancesArgs {
    #[arg(long, default_value = "stake", help = "Token denom to read")]
    denom: String,
}

#[derive(clap::Args)]
struct LogsArgs {
    #[arg(
        long,
        default_value = "120s",
        help = "How far back to pull container logs"
    )]
    since: String,
}

#[derive(clap::Args)]
struct TransferArgs {
    #[arg(
        value_enum,
        default_value = "stellar",
        help = "Source chain to send from"
    )]
    from: Chain,
    #[arg(long, default_value = "stake", help = "Token denom to transfer")]
    denom: String,
    #[arg(long, default_value_t = 1000, help = "Amount to transfer")]
    amount: i128,
    #[arg(
        long,
        default_value = "",
        help = "Receiver address on the destination chain (default: the relayer key on the destination)"
    )]
    receiver: String,
    #[arg(long, default_value = "", help = "Optional transfer memo")]
    memo: String,
    #[arg(long, default_value_t = 600, help = "Timeout in seconds from now")]
    timeout_secs: u64,
    #[arg(
        long,
        help = "Skip minting the amount to the sender first (devnet mints by default)"
    )]
    no_mint: bool,
}

#[derive(clap::Args)]
struct DemoArgs {
    #[arg(
        long,
        help = "Run the ICS-20 transfer round-trip demo (the default and only scenario today)"
    )]
    transfer: bool,
    #[arg(
        value_enum,
        default_value = "stellar",
        help = "Source chain to transfer from"
    )]
    from: Chain,
    #[arg(long, default_value = "stake", help = "Token denom to transfer")]
    denom: String,
    #[arg(long, default_value_t = 1000, help = "Amount to transfer")]
    amount: i128,
    #[arg(
        long,
        default_value = "",
        help = "Receiver address on the destination chain (default: the relayer key on the destination)"
    )]
    receiver: String,
    #[arg(long, default_value = "", help = "Optional transfer memo")]
    memo: String,
    #[arg(
        long,
        default_value_t = 600,
        help = "Transfer timeout in seconds from now"
    )]
    timeout_secs: u64,
    #[arg(long, help = "Skip minting the amount to the sender first")]
    no_mint: bool,
    #[arg(
        long,
        help = "Skip the full `start` step (assume the stack is already up)"
    )]
    skip_start: bool,
    #[arg(
        long,
        help = "Force-redeploy contracts and re-bootstrap clients during start"
    )]
    force_redeploy: bool,
    #[arg(
        long,
        default_value_t = 120,
        help = "Max seconds to watch the relay round trip (exits early when it closes) before reading balances-after"
    )]
    wait_secs: u64,
}

#[derive(clap::Args)]
struct UpArgs {
    #[arg(long, help = "Start only the Cosmos chain (cosmos)")]
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
struct StartArgs {
    #[arg(long, help = "Skip pulling the docker images")]
    skip_images: bool,
    #[arg(long, help = "Skip the Soroban contract deploy")]
    skip_contracts: bool,
    #[arg(long, help = "Skip the light-client-wasm upload")]
    skip_wasm: bool,
    #[arg(long, help = "Skip importing the hermes relayer keys")]
    skip_keys: bool,
    #[arg(long, help = "Skip provisioning the sender + receiver accounts")]
    skip_accounts: bool,
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
enum CosmosCmd {
    #[command(about = "Start the local Cosmos chain (cardano-entrypoint, ibc-go v10 + 08-wasm)")]
    Start,
    #[command(about = "Stop the local Cosmos chain")]
    Stop,
    #[command(about = "Show the Cosmos chain endpoints and health")]
    Status,
    #[command(
        about = "Check the public cosmos-testnet (Cosmos Hub `provider`) — health + node/app version"
    )]
    Testnet {
        #[arg(
            long,
            value_name = "ADDRESS",
            help = "Query this cosmos address's balances on cosmos-testnet (and show the faucet) instead of the health check"
        )]
        balance: Option<String>,
    },
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
    #[command(
        about = "Create both clients and register both counterparties atomically (ids can't drift)"
    )]
    Bootstrap {
        #[arg(long, help = "Force-create fresh clients even if ids are already set")]
        force: bool,
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
    },
    #[command(about = "Build + gov-upload the light-client-wasm to Cosmos, patch hermes config")]
    UploadWasm {
        #[arg(
            long,
            help = "Prepare the 08-wasm store-code gov proposal for cosmos-testnet (provider) instead of auto-uploading to the local devnet"
        )]
        testnet: bool,
        #[arg(
            long,
            value_name = "KEY",
            help = "With --testnet: gaiad keyring key to submit the proposal from (else the command is printed)"
        )]
        from: Option<String>,
    },
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

        Command::Clients { cmd } => {
            let cc = clients::config::ClientsConfig::from(&cfg);

            match cmd {
                ClientsCmd::Cosmos { force } => {
                    clients::cosmos::run(&cc, root, &http, force).await?;
                }
                ClientsCmd::Stellar { force } => {
                    clients::stellar::run(&cc, root, &http, force).await?;
                }
                ClientsCmd::Counterparty { chain } => {
                    clients::counterparty::run(&cc, root, chain.as_str())?
                }
                ClientsCmd::Bootstrap { force } => {
                    clients::bootstrap(&cc, root, &http, force).await?
                }
                ClientsCmd::List => clients::list::run(&cc, &http).await?,
            }
        }

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

        Command::Contracts { cmd } => {
            let cc = contracts::config::ContractsConfig::from(&cfg);

            match cmd {
                ContractsCmd::Build => contracts::build::run(root)?,
                ContractsCmd::Upload { wasm } => contracts::upload::run(&cc, root, &wasm)?,
                ContractsCmd::Deploy { wasm, ctor } => {
                    contracts::deploy::run(&cc, root, &wasm, &ctor)?
                }
                ContractsCmd::Invoke { id, call } => contracts::invoke::run(&cc, root, &id, &call)?,
                ContractsCmd::DeployAll { force, attestation } => {
                    contracts::deploy_all::run(&cc, root, force, attestation)?;
                }
                ContractsCmd::UploadWasm { testnet, from } => {
                    contracts::wasm::upload(&cc, root, &http, testnet, from.as_deref()).await?
                }
            }
        }

        Command::Transfer(args) => {
            let ta = transfer::TransferArgs {
                denom: args.denom,
                amount: args.amount,
                receiver: args.receiver,
                memo: args.memo,
                timeout_secs: args.timeout_secs,
                mint: !args.no_mint,
            };

            match args.from {
                Chain::Stellar => transfer::stellar_to_cosmos(&cfg, root, &ta)?,
                Chain::Cosmos => transfer::cosmos_to_stellar(&cfg, root, &ta)?,
            }
        }

        Command::Demo(args) => {
            if !args.transfer {
                logger::warn("no scenario flag given — running the default --transfer scenario");
            }

            let ta = transfer::TransferArgs {
                denom: args.denom,
                amount: args.amount,
                receiver: args.receiver,
                memo: args.memo,
                timeout_secs: args.timeout_secs,
                mint: !args.no_mint,
            };

            demo::run(
                root,
                &http,
                demo::DemoArgs {
                    from_cosmos: matches!(args.from, Chain::Cosmos),
                    skip_start: args.skip_start,
                    force_redeploy: args.force_redeploy,
                    wait_secs: args.wait_secs,
                    transfer: ta,
                },
            )
            .await?
        }

        Command::Accounts => accounts::show(&cfg),
        Command::Balances(args) => balances::run(&cfg, root, &http, &args.denom).await?,
        Command::Logs(args) => logs::run(root, &args.since)?,

        Command::Tx { cmd } => match cmd {
            TxCmd::Clients { cmd } => match cmd {
                TxClientsCmd::Create => tx::clients::create()?,
                TxClientsCmd::Update => tx::clients::update()?,
            },
            TxCmd::Msg { cmd } => match cmd {
                TxMsgCmd::RegisterCounterparty { chain } => {
                    let cc = clients::config::ClientsConfig::from(&cfg);
                    tx::msg::register_counterparty(&cc, root, chain.as_str())?
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
