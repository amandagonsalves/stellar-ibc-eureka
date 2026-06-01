use anyhow::Result;

use crate::shared;

pub fn run() -> Result<()> {
    shared::pending(
        "gateway query",
        "direct gateway gRPC reads (LatestHeight, QueryPacketCommitment/Receipt/Acknowledgement, QueryIbcHeader, Events) are not exposed here yet — they live behind the relayer today.",
    );

    Ok(())
}
