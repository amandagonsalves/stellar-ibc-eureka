use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::logger;

pub fn run(cfg: &Config, root: &Path, wasm: &str, ctor: &[String]) -> Result<()> {
    logger::banner("contracts deploy");

    logger::step(&format!("stellar contract deploy --wasm {wasm}"));

    let ctor_refs: Vec<&str> = ctor.iter().map(String::as_str).collect();
    let id = super::deploy(cfg, root, wasm, &ctor_refs)?;

    logger::ok(&format!("contract id: {id}"));
    println!("{id}");

    Ok(())
}
