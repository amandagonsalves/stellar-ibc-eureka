use anyhow::Result;

use crate::ops::config::OpsConfig;
use crate::{logger, probe, shared};

pub async fn run(cfg: &OpsConfig) -> Result<()> {
    logger::banner("status");

    logger::step("Chains & services");

    let gateway = probe::tcp_ok(&cfg.gateway_url);
    logger::status_line("gateway-grpc", gateway, &cfg.gateway_url);

    logger::step("Endpoints");
    logger::detail(&format!("cosmos rpc   {}", cfg.cosmos_rpc_url));
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

    logger::step("Accounts (sender + receiver, from .env)");
    for (label, address) in &cfg.accounts {
        shared::contract(label, address);
    }

    Ok(())
}
