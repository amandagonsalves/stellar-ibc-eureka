use std::path::Path;

use anyhow::{bail, Result};

use crate::contracts::config::ContractsConfig;
use crate::logger;

pub fn run(cfg: &ContractsConfig, root: &Path, id: &str, call: &[String]) -> Result<()> {
    logger::banner("contracts invoke");

    if call.is_empty() {
        bail!("nothing to invoke — pass the function + args after `--`, e.g. `-- register_port --port_id transfer --app_address C...`");
    }

    logger::step(&format!(
        "stellar contract invoke --id {id} -- {}",
        call.join(" ")
    ));

    let call_refs: Vec<&str> = call.iter().map(String::as_str).collect();

    super::invoke(cfg, root, id, &call_refs)
}
