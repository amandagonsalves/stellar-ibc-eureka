pub mod build;
pub mod deploy;
pub mod deploy_all;
pub mod invoke;
pub mod upload;
pub mod wasm;

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::run;

pub(crate) fn net_flags(cfg: &Config) -> Vec<String> {
    vec![
        "--rpc-url".to_string(),
        cfg.stellar_rpc_url.clone(),
        "--network-passphrase".to_string(),
        cfg.network_passphrase.clone(),
    ]
}

pub(crate) fn last_line(out: &str) -> String {
    out.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

fn stellar_base(cfg: &Config, sub: &str) -> Vec<String> {
    let mut args = vec![
        "contract".to_string(),
        sub.to_string(),
        "--source".to_string(),
        cfg.deployer_identity.clone(),
    ];
    args.extend(net_flags(cfg));

    args
}

pub(crate) fn deploy(cfg: &Config, root: &Path, wasm: &str, ctor: &[&str]) -> Result<String> {
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

pub(crate) fn invoke(cfg: &Config, root: &Path, id: &str, call: &[&str]) -> Result<()> {
    let mut args = stellar_base(cfg, "invoke");
    args.push("--id".to_string());
    args.push(id.to_string());
    args.push("--".to_string());
    args.extend(call.iter().map(|s| s.to_string()));

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    run::command(root, "stellar", &refs)
}
