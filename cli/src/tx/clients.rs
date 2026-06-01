use anyhow::Result;

use crate::shared;

const REASON: &str =
    "low-level client txs build on the gateway prepare->sign->submit path. create_client is wired via `stellaribc clients cosmos`; a direct tx surface lands with TASKS.md Task 3.";

pub fn create() -> Result<()> {
    shared::pending("tx clients create", REASON);

    Ok(())
}

pub fn update() -> Result<()> {
    shared::pending("tx clients update", REASON);

    Ok(())
}
