use anyhow::Result;

use crate::shared;

const REASON: &str =
    "provable-path queries (packet commitment 0x01, receipt 0x02, ack 0x03) + QueryIbcHeader are served by the gateway gRPC; a direct query surface is not exposed here yet.";

pub fn commitment() -> Result<()> {
    shared::pending("tx query commitment", REASON);

    Ok(())
}

pub fn receipt() -> Result<()> {
    shared::pending("tx query receipt", REASON);

    Ok(())
}

pub fn ack() -> Result<()> {
    shared::pending("tx query ack", REASON);

    Ok(())
}

pub fn header() -> Result<()> {
    shared::pending("tx query header", REASON);

    Ok(())
}
