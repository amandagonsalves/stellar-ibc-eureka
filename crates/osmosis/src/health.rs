use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::config;

pub async fn current_height() -> Result<Option<u64>> {
    let body = reqwest::get(config::STATUS_URL).await?.text().await?;
    let json: Value = serde_json::from_str(&body).unwrap_or_default();
    Ok(json["result"]["sync_info"]["latest_block_height"]
        .as_str()
        .and_then(|height| height.parse::<u64>().ok()))
}

pub async fn wait_until_healthy() -> Result<()> {
    for attempt in 1..=config::HEALTH_RETRIES {
        if let Ok(Some(height)) = current_height().await {
            if height > 0 {
                println!(
                    "Osmosis is healthy at block height {height} ({})",
                    config::RPC_URL
                );
                return Ok(());
            }
        }

        if attempt < config::HEALTH_RETRIES {
            tokio::time::sleep(Duration::from_millis(config::HEALTH_INTERVAL_MS)).await;
        }
    }

    bail!(
        "Osmosis did not report a block height > 0 within {} attempts at {}. Check `docker compose logs osmosis`.",
        config::HEALTH_RETRIES,
        config::STATUS_URL
    )
}

pub async fn report() -> Result<()> {
    match current_height().await {
        Ok(Some(height)) if height > 0 => {
            println!(
                "healthy: localosmosis at block height {height} ({})",
                config::RPC_URL
            );
            Ok(())
        }
        Ok(_) => bail!(
            "unhealthy: no positive block height reported at {}",
            config::STATUS_URL
        ),
        Err(error) => bail!("unhealthy: {error} ({})", config::STATUS_URL),
    }
}
