use std::path::Path;

use anyhow::Result;

use super::last_line;
use crate::contracts::config::ContractsConfig;
use crate::{logger, run};

pub fn run(cfg: &ContractsConfig, root: &Path, wasm: &str) -> Result<()> {
    logger::banner("contracts upload");

    let mut args: Vec<String> = vec![
        "contract".into(),
        "upload".into(),
        "--source".into(),
        cfg.cli_identity.clone(),
    ];
    args.extend(cfg.net_flags());
    args.push("--wasm".into());
    args.push(wasm.to_string());

    logger::step(&format!("stellar contract upload --wasm {wasm}"));

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let hash = last_line(&run::capture(root, "stellar", &refs)?);

    logger::ok(&format!("wasm hash: {hash}"));
    println!("{hash}");

    Ok(())
}
