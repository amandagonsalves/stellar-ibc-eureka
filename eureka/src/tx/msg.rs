use std::path::Path;

use anyhow::Result;

use crate::clients;
use crate::clients::config::ClientsConfig;
use crate::shared;

const REASON: &str =
    "the gateway now returns signable recv/ack/timeout txs; these need the relayer (hermes fork) to sign + submit them, then the relayer packet worker.";

pub fn register_counterparty(cfg: &ClientsConfig, root: &Path, side: &str) -> Result<()> {
    clients::counterparty::run(cfg, root, side)
}

pub fn recv() -> Result<()> {
    shared::pending("tx msg recv-packet", REASON);

    Ok(())
}

pub fn ack() -> Result<()> {
    shared::pending("tx msg ack-packet", REASON);

    Ok(())
}

pub fn timeout() -> Result<()> {
    shared::pending("tx msg timeout-packet", REASON);

    Ok(())
}
