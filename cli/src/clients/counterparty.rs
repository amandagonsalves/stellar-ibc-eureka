use std::path::Path;

use anyhow::Result;

use crate::{logger, run};

pub fn run(root: &Path, side: &str) -> Result<()> {
    let (label, script) = match side {
        "stellar" => (
            "clients counterparty stellar (F1.3 — register Cosmos client as counterparty on Stellar)",
            "f1-register-counterparty-stellar.sh",
        ),
        _ => (
            "clients counterparty cosmos (F1.4 — register Stellar client as counterparty on Cosmos)",
            "f1-register-counterparty-cosmos.sh",
        ),
    };

    logger::banner(label);

    if run::script_exists(root, script) {
        return run::script(root, script, run::NO_ENV);
    }

    logger::warn("not wired yet");
    logger::detail("blocked on migrating the gateway's register_counterparty RPC to prepare->sign->submit (TASKS.md Task 3).");
    logger::detail(&format!(
        "once ci/flows/{script} lands, this command runs it automatically."
    ));

    Ok(())
}
