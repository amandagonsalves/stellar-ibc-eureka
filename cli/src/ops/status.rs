use anyhow::Result;

use crate::ops::config::OpsConfig;
use crate::{logger, probe, shared};

pub async fn run(cfg: &OpsConfig, http: &reqwest::Client) -> Result<()> {
    logger::banner("status");

    logger::step("Chains & services");

    let gateway = probe::tcp_ok(&cfg.gateway_url);
    logger::status_line("gateway-grpc", gateway, &cfg.gateway_url);

    logger::step("Endpoints");
    logger::detail(&format!("cosmos rpc   {}", cfg.osmosis_rpc_url));
    logger::detail(&format!("hermes cfg   {}", cfg.hermes_config));

    logger::step("Images");
    for (label, reference) in &cfg.images {
        logger::detail(&format!("{label:<8} {reference}"));
    }

    logger::step("Stellar contracts (from .env)");
    if cfg.addresses.is_empty() {
        logger::detail("none deployed yet");
    } else {
        for (kind, value) in &cfg.addresses {
            shared::contract(kind.as_str(), value);
        }
    }

    Ok(())
}
