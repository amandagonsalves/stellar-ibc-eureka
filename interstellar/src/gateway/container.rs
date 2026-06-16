use std::path::Path;

use anyhow::Result;

use crate::gateway::config::GatewayConfig;
use crate::service::Service;

const SERVICE: Service = Service::new("gateway");

pub fn start(cfg: &GatewayConfig, root: &Path, pull: bool) -> Result<()> {
    SERVICE.start(root, &cfg.image, pull)
}

pub fn stop(root: &Path) -> Result<()> {
    SERVICE.stop(root)
}

pub fn restart(cfg: &GatewayConfig, root: &Path, pull: bool) -> Result<()> {
    SERVICE.restart(root, &cfg.image, pull)
}
