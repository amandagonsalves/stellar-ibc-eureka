use std::path::Path;

use anyhow::Result;

use crate::hermes::config::HermesConfig;
use crate::run;
use crate::service::Service;

const SERVICE: Service = Service::new("hermes");

pub fn start(cfg: &HermesConfig, root: &Path, pull: bool) -> Result<()> {
    SERVICE.start(root, &cfg.image, pull)
}

pub fn stop(root: &Path) -> Result<()> {
    SERVICE.stop(root)
}

pub fn restart(cfg: &HermesConfig, root: &Path, pull: bool) -> Result<()> {
    SERVICE.restart(root, &cfg.image, pull)
}

pub fn exec(root: &Path, config_path: &str, args: &[&str]) -> Result<String> {
    run::compose(root, &["up", "-d", SERVICE.name()])?;

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

    run::capture_all(root, "docker", &full)
}
