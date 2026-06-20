pub mod build;
pub mod config;
pub mod deploy_all;
pub mod upload;
pub mod wasm;

use std::path::Path;

use anyhow::{bail, Result};

use crate::tx::contracts::config::ContractsConfig;
use crate::{logger, tools};

const SOROBAN_WASMS: [&str; 5] = [
    "stellar_mock_light_client",
    "stellar_ibc_router",
    "stellar_ibc_transfer",
    "stellar_tendermint_light_client",
    "stellar_attestation_light_client",
];

fn wasm_path(root: &Path, name: &str) -> String {
    root.join("contracts/soroban/target/wasm32v1-none/contract")
        .join(format!("{name}.wasm"))
        .display()
        .to_string()
}

pub fn upload_all(cfg: &ContractsConfig, root: &Path) -> Result<()> {
    logger::banner("tx contracts upload (install all soroban wasms)");

    build::run(root)?;

    for name in SOROBAN_WASMS {
        logger::step(&format!("uploading {name}"));
        upload::run(cfg, root, &wasm_path(root, name))?;
    }

    Ok(())
}

pub fn deploy_one(cfg: &ContractsConfig, root: &Path, name: &str) -> Result<String> {
    logger::banner(&format!("tx contracts deploy --contract {name}"));

    if !SOROBAN_WASMS.contains(&name) {
        bail!(
            "unknown contract '{name}' — expected one of: {}",
            SOROBAN_WASMS.join(", ")
        );
    }

    build::run(root)?;

    let deployer = deploy_all::deployer_address(cfg, root)?;
    let wasm = wasm_path(root, name);

    let ctor: Vec<String> = match name {
        "stellar_ibc_router" => vec!["--admin".into(), deployer],
        "stellar_ibc_transfer" => {
            if cfg.ibc_router.is_empty() {
                bail!("ROUTER_CONTRACT_ADDRESS is unset — deploy the router first or run `tx contracts deploy --stellar`");
            }

            vec![
                "--router".into(),
                cfg.ibc_router.clone(),
                "--admin".into(),
                deployer,
            ]
        }
        _ => vec![],
    };

    let refs: Vec<&str> = ctor.iter().map(String::as_str).collect();
    let id = deploy(cfg, root, &wasm, &refs)?;

    logger::ok(&format!("{name}: {id}"));
    println!("{id}");

    Ok(id)
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

    Ok(last_line(&tools::stellar::capture(root, &refs)?))
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

    tools::stellar::command(root, &refs)
}
