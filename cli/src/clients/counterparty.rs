use anyhow::Result;

use crate::logger;

pub fn run(side: &str) -> Result<()> {
    let label = match side {
        "stellar" => {
            "clients counterparty stellar (register the Cosmos client as counterparty on Stellar)"
        }
        _ => "clients counterparty cosmos (register the Stellar client as counterparty on Cosmos)",
    };

    logger::banner(label);
    logger::warn("not wired yet");

    Ok(())
}
