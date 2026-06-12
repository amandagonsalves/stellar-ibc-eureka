use anyhow::Result;

use crate::shared;

const REASON: &str =
    "low-level client txs build on the gateway prepare->sign->submit path. create_client is wired via `eurekastellar clients cosmos`; a direct tx surface lands once the relayer signs + submits the gateway's prepared txs.";

pub fn create() -> Result<()> {
    shared::pending("tx clients create", REASON);

    Ok(())
}

pub fn update() -> Result<()> {
    shared::pending("tx clients update", REASON);

    Ok(())
}
