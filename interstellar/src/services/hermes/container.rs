use std::path::Path;

use anyhow::Result;

use crate::service::Service;
use crate::services::hermes::config::HermesConfig;
use crate::tools;

const SERVICE: Service = Service::new("hermes");

pub fn start(cfg: &HermesConfig, root: &Path, pull: bool) -> Result<()> {
    SERVICE.start(root, &cfg.image, pull)
}

pub fn exec(root: &Path, config_path: &str, args: &[&str]) -> Result<String> {
    tools::docker::compose(root, &["up", "-d", SERVICE.name()])?;

    let mut full: Vec<&str> = vec![
        "compose",
        "--profile",
        "local",
        "--profile",
        "hermes",
        "exec",
        "-T",
        SERVICE.name(),
        "hermes",
        "--config",
        config_path,
    ];
    full.extend_from_slice(args);

    tools::docker::capture_all(root, &full)
}
