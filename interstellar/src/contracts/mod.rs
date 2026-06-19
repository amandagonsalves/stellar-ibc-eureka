pub mod build;
pub mod config;
pub mod deploy;
pub mod deploy_all;
pub mod invoke;
pub mod upload;
pub mod wasm;

use std::path::Path;

use anyhow::Result;

use crate::contracts::config::ContractsConfig;
use crate::run;

#[derive(clap::Subcommand)]
pub enum ContractsCmd {
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

pub(crate) fn last_line(out: &str) -> String {
    out.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

fn stellar_base(cfg: &ContractsConfig, sub: &str) -> Vec<String> {
    stellar_base_as(cfg, sub, &cfg.cli_identity)
}

fn stellar_base_as(cfg: &ContractsConfig, sub: &str, source: &str) -> Vec<String> {
    let mut args = vec![
        "contract".to_string(),
        sub.to_string(),
        "--source".to_string(),
        source.to_string(),
    ];
    args.extend(cfg.net_flags());

    args
}

pub(crate) fn deploy(
    cfg: &ContractsConfig,
    root: &Path,
    wasm: &str,
    ctor: &[&str],
) -> Result<String> {
    let mut args = stellar_base(cfg, "deploy");
    args.push("--wasm".to_string());
    args.push(wasm.to_string());

    if !ctor.is_empty() {
        args.push("--".to_string());
        args.extend(ctor.iter().map(|s| s.to_string()));
    }

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    Ok(last_line(&run::capture(root, "stellar", &refs)?))
}

pub(crate) fn invoke(cfg: &ContractsConfig, root: &Path, id: &str, call: &[&str]) -> Result<()> {
    invoke_as(cfg, root, id, call, &cfg.cli_identity)
}

pub(crate) fn invoke_as(
    cfg: &ContractsConfig,
    root: &Path,
    id: &str,
    call: &[&str],
    source: &str,
) -> Result<()> {
    let mut args = stellar_base_as(cfg, "invoke", source);
    args.push("--id".to_string());
    args.push(id.to_string());
    args.push("--".to_string());
    args.extend(call.iter().map(|s| s.to_string()));

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    run::command(root, "stellar", &refs)
}
