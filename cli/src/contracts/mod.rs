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

pub(crate) fn last_line(out: &str) -> String {
    out.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

fn stellar_base(cfg: &ContractsConfig, sub: &str) -> Vec<String> {
    let mut args = vec![
        "contract".to_string(),
        sub.to_string(),
        "--source".to_string(),
        cfg.cli_identity.clone(),
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
    let mut args = stellar_base(cfg, "invoke");
    args.push("--id".to_string());
    args.push(id.to_string());
    args.push("--".to_string());
    args.extend(call.iter().map(|s| s.to_string()));

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    run::command(root, "stellar", &refs)
}
