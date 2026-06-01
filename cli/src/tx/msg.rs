use std::path::Path;

use anyhow::Result;

use crate::clients;
use crate::shared;

const REASON: &str =
    "packet messages need the gateway's recv/ack/timeout RPCs migrated to prepare->sign->submit (TASKS.md Task 3), then the relayer packet worker (Task 5).";

pub fn register_counterparty(root: &Path, side: &str) -> Result<()> {
    clients::counterparty::run(root, side)
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
