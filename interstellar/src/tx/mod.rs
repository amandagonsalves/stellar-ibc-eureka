use std::path::Path;

use anyhow::{anyhow, bail, Result};

use crate::config::Config;
use crate::shared::{self, Chain};

pub mod clients;
pub mod contracts;
pub mod transfer;

use clients::config::ClientsConfig;
use contracts::config::ContractsConfig;
use transfer::TransferParams;

#[derive(clap::Subcommand)]
pub enum TxCmd {
    #[command(about = "Client lifecycle: create clients, register counterparties")]
    Clients {
        #[command(subcommand)]
        cmd: ClientsTx,
    },
    #[command(about = "Contracts: upload wasm, deploy on Stellar or store code on Cosmos")]
    Contracts {
        #[command(subcommand)]
        cmd: ContractsTx,
    },
    #[command(about = "Originate an ICS-20 transfer, routed by the from/to address")]
    Transfer(TransferTx),
}

#[derive(clap::Subcommand)]
pub enum ClientsTx {
    #[command(about = "Create the Cosmos and/or Stellar client (both when neither flag is given)")]
    Create(SideArgs),
    #[command(
        about = "Register the counterparty on Cosmos and/or Stellar (both when neither flag is given)"
    )]
    Counterparty(SideArgs),
}

#[derive(clap::Args)]
pub struct SideArgs {
    #[arg(long, help = "Act on the Cosmos side")]
    pub cosmos: bool,
    #[arg(long, help = "Act on the Stellar side")]
    pub stellar: bool,
    #[arg(
        long,
        help = "Force-recreate even if the id is already set (create only)"
    )]
    pub force: bool,
}

#[derive(clap::Subcommand)]
pub enum ContractsTx {
    #[command(about = "Build + upload (install) every Soroban contract wasm to the network")]
    Upload,
    #[command(
        about = "Deploy soroban contracts (--stellar) or store the contract code (--cosmos)"
    )]
    Deploy(DeployArgs),
    #[command(about = "Invoke a function on a deployed soroban contract (fn + args after `--`)")]
    Invoke(InvokeArgs),
}

#[derive(clap::Args)]
pub struct InvokeArgs {
    #[arg(long, help = "Deployed contract id")]
    pub id: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub call: Vec<String>,
}

#[derive(clap::Args)]
pub struct DeployArgs {
    #[arg(long, help = "Deploy soroban contracts on Stellar (the default)")]
    pub stellar: bool,
    #[arg(long, help = "Store the contract code on Cosmos (08-wasm)")]
    pub cosmos: bool,
    #[arg(
        long,
        value_name = "NAME",
        help = "Wasm file name without the .wasm extension; omit to act on all contracts"
    )]
    pub contract: Option<String>,
    #[arg(
        long,
        help = "Redeploy even if ROUTER_CONTRACT_ADDRESS is set (stellar, all)"
    )]
    pub force: bool,
    #[arg(long, help = "Also deploy the attestation light client (stellar, all)")]
    pub attestation: bool,
}

#[derive(clap::Args)]
pub struct TransferTx {
    #[arg(long, help = "Source address (cosmos or stellar — chain inferred)")]
    pub from: String,
    #[arg(
        long,
        help = "Destination address (cosmos or stellar — chain inferred)"
    )]
    pub to: String,
    #[arg(long, help = "Amount to transfer")]
    pub amount: i128,
    #[arg(long, default_value = "stake", help = "Token denom to transfer")]
    pub denom: String,
    #[arg(long, default_value_t = 600, help = "Timeout in seconds from now")]
    pub timeout_secs: u64,
    #[arg(long, help = "Skip minting the amount to the sender first")]
    pub no_mint: bool,
}

enum Sides {
    Cosmos,
    Stellar,
    Both,
}

fn sides(cosmos: bool, stellar: bool) -> Sides {
    match (cosmos, stellar) {
        (true, false) => Sides::Cosmos,
        (false, true) => Sides::Stellar,
        _ => Sides::Both,
    }
}

pub async fn run(cfg: &Config, root: &Path, http: &reqwest::Client, cmd: TxCmd) -> Result<()> {
    match cmd {
        TxCmd::Clients { cmd } => clients_tx(cfg, root, http, cmd).await,
        TxCmd::Contracts { cmd } => contracts_tx(cfg, root, http, cmd).await,
        TxCmd::Transfer(args) => transfer_tx(cfg, root, args),
    }
}

async fn clients_tx(
    cfg: &Config,
    root: &Path,
    http: &reqwest::Client,
    cmd: ClientsTx,
) -> Result<()> {
    let cc = ClientsConfig::from(cfg);

    match cmd {
        ClientsTx::Create(args) => match sides(args.cosmos, args.stellar) {
            Sides::Cosmos => clients::cosmos::run(&cc, root, http, args.force)
                .await
                .map(|_| ()),
            Sides::Stellar => clients::stellar::run(&cc, root, http, args.force)
                .await
                .map(|_| ()),
            Sides::Both => {
                clients::cosmos::run(&cc, root, http, args.force).await?;
                clients::stellar::run(&cc, root, http, args.force).await?;

                Ok(())
            }
        },
        ClientsTx::Counterparty(args) => match sides(args.cosmos, args.stellar) {
            Sides::Cosmos => clients::counterparty::run(&cc, root, "cosmos"),
            Sides::Stellar => clients::counterparty::run(&cc, root, "stellar"),
            Sides::Both => {
                clients::counterparty::run(&cc, root, "cosmos")?;
                clients::counterparty::run(&cc, root, "stellar")?;

                Ok(())
            }
        },
    }
}

async fn contracts_tx(
    cfg: &Config,
    root: &Path,
    http: &reqwest::Client,
    cmd: ContractsTx,
) -> Result<()> {
    let cc = ContractsConfig::from(cfg);

    match cmd {
        ContractsTx::Upload => contracts::upload_all(&cc, root),
        ContractsTx::Deploy(args) => {
            if args.cosmos {
                return contracts::wasm::upload(&cc, root, http, false, None).await;
            }

            match args.contract {
                Some(name) => contracts::deploy_one(&cc, root, &name).map(|_| ()),
                None => {
                    contracts::deploy_all::run(&cc, root, args.force, args.attestation).map(|_| ())
                }
            }
        }
        ContractsTx::Invoke(args) => {
            let call: Vec<&str> = args.call.iter().map(String::as_str).collect();

            contracts::invoke(&cc, root, &args.id, &call)
        }
    }
}

fn transfer_tx(cfg: &Config, root: &Path, args: TransferTx) -> Result<()> {
    let from = shared::chain_of(&args.from)
        .ok_or_else(|| anyhow!("could not classify --from address {:?}", args.from))?;
    let to = shared::chain_of(&args.to)
        .ok_or_else(|| anyhow!("could not classify --to address {:?}", args.to))?;

    let params = TransferParams {
        denom: args.denom,
        amount: args.amount,
        receiver: args.to,
        sender: args.from,
        memo: String::new(),
        timeout_secs: args.timeout_secs,
        mint: !args.no_mint,
    };

    match (from, to) {
        (Chain::Stellar, Chain::Cosmos) => transfer::stellar_to_cosmos(cfg, root, &params),
        (Chain::Cosmos, Chain::Stellar) => transfer::cosmos_to_stellar(cfg, root, &params),
        _ => bail!(
            "transfer must cross chains (stellar↔cosmos): got {} → {}",
            from.as_str(),
            to.as_str()
        ),
    }
}
