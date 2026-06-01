use anyhow::Result;

use crate::logger;

pub fn run(side: &str) -> Result<()> {
    let label = match side {
        "stellar" => "clients counterparty stellar (F1.3 — register Cosmos client as counterparty on Stellar)",
        _ => "clients counterparty cosmos (F1.4 — register Stellar client as counterparty on Cosmos)",
    };

    logger::banner(label);
    logger::warn("not wired yet");
    logger::detail("counterparty registration needs the gateway's register_counterparty RPC migrated to prepare->sign->submit (TASKS.md Task 3), then a native flow here.");

    Ok(())
}
