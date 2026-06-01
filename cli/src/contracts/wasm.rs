use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::contracts::config::ContractsConfig;
use crate::{logger, probe, run};

const CRATE: &str = "light-client-wasm";
const DEPOSIT_AMOUNT: u64 = 10_000_000;
const STORE_GAS_LIMIT: u64 = 60_000_000;
const STORE_FEE_AMOUNT: u64 = 1_800_000;
const VOTE_OPTION: u64 = 1;
const VOTE_GAS_LIMIT: u64 = 200_000;
const VOTE_FEE_AMOUNT: u64 = 10_000;
const TX_WAIT_TIMEOUT_SECS: u64 = 60;
const VOTING_PERIOD_SECS: u64 = 25;
const VERIFY_RETRIES: u32 = 15;
const VERIFY_INTERVAL_SECS: u64 = 2;
const FUND_AMOUNT: u64 = 100_000_000;
const FUND_GAS_LIMIT: u64 = 200_000;
const FUND_FEE_AMOUNT: u64 = 10_000;

pub async fn upload(cfg: &ContractsConfig, root: &Path, http: &reqwest::Client) -> Result<()> {
    logger::banner("contracts upload-wasm (light-client-wasm -> Cosmos 08-wasm)");

    logger::step("cargo build --target wasm32-unknown-unknown -p light-client-wasm --release");
    run::command(
        root,
        "cargo",
        &["build", "--target", "wasm32-unknown-unknown", "-p", CRATE, "--release"],
    )?;

    let wasm_file = root.join("target/wasm32-unknown-unknown/release/light_client_wasm.wasm");
    if !wasm_file.exists() {
        bail!("expected wasm artifact not found at {}", wasm_file.display());
    }

    if run::has("wasm-opt") {
        logger::step("wasm-opt (lower bulk-memory)");
        let path = wasm_file.display().to_string();
        run::command(
            root,
            "wasm-opt",
            &[
                "--enable-bulk-memory",
                "--llvm-memory-copy-fill-lowering",
                "-O1",
                "--strip-debug",
                path.as_str(),
                "-o",
                path.as_str(),
            ],
        )?;
    } else {
        logger::warn("wasm-opt not installed — install binaryen if the upload is rejected for bulk-memory");
    }

    let bytes = std::fs::read(&wasm_file).with_context(|| format!("reading {}", wasm_file.display()))?;
    let local_sha = hex::encode(Sha256::digest(&bytes));
    logger::detail(&format!("{} bytes, sha256={local_sha}", bytes.len()));

    if !probe::http_ok(http, &format!("{}/cosmos/node-info", cfg.api_url)).await {
        logger::warn(&format!(
            "api not reachable at {} — start it with: stellaribc api start",
            cfg.api_url
        ));

        return Ok(());
    }

    let proposer = proposer_address(http, cfg).await?;
    logger::step(&format!("funding proposer {proposer}"));
    post(
        http,
        &format!("{}/cosmos/bank/send", cfg.api_url),
        serde_json::json!({
            "to": proposer,
            "amount": FUND_AMOUNT,
            "gas_limit": FUND_GAS_LIMIT,
            "fee_amount": FUND_FEE_AMOUNT,
            "skip_if_account_exists": true,
        }),
    )
    .await?;

    logger::step("submitting store-code proposal");
    let wasm_b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let store = post(
        http,
        &format!("{}/cosmos/ibc-wasm/store-code", cfg.api_url),
        serde_json::json!({
            "wasm_base64": wasm_b64,
            "title": format!("upload-light-client-wasm: {CRATE}"),
            "summary": "Registers light_client_wasm.wasm for the 08-wasm client type",
            "deposit_amount": DEPOSIT_AMOUNT,
            "gas_limit": STORE_GAS_LIMIT,
            "fee_amount": STORE_FEE_AMOUNT,
            "wait_for_landing": true,
            "wait_timeout_secs": TX_WAIT_TIMEOUT_SECS,
        }),
    )
    .await?;

    let proposal_id = store
        .get("proposal_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow!("proposal_id not present in api response: {store}"))?;
    logger::ok(&format!("proposal id: {proposal_id}"));

    logger::step(&format!("voting YES on proposal {proposal_id}"));
    post(
        http,
        &format!("{}/cosmos/gov/vote", cfg.api_url),
        serde_json::json!({
            "proposal_id": proposal_id,
            "option": VOTE_OPTION,
            "gas_limit": VOTE_GAS_LIMIT,
            "fee_amount": VOTE_FEE_AMOUNT,
        }),
    )
    .await?;

    logger::detail(&format!("waiting {VOTING_PERIOD_SECS}s for the voting period"));
    tokio::time::sleep(Duration::from_secs(VOTING_PERIOD_SECS)).await;

    logger::step("verifying checksum on-chain");
    if !checksum_registered(http, cfg, local_sha.as_str()).await {
        bail!("local sha256 {local_sha} did not appear in on-chain checksums (proposal may not have passed)");
    }
    logger::ok(&format!("wasm registered with checksum {local_sha}"));

    logger::step("patching wasm_checksum_hex via api");
    post(
        http,
        &format!("{}/hermes/wasm-checksum", cfg.api_url),
        serde_json::json!({ "checksum": local_sha }),
    )
    .await?;
    logger::ok("hermes config patched");

    Ok(())
}

async fn checksum_registered(http: &reqwest::Client, cfg: &ContractsConfig, local_sha: &str) -> bool {
    let url = format!("{}/cosmos/ibc-wasm/checksums", cfg.api_url);

    for attempt in 1..=VERIFY_RETRIES {
        if let Some(value) = probe::get_json(http, &url).await {
            let present = value
                .get("checksums")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .any(|c| c.eq_ignore_ascii_case(local_sha))
                })
                .unwrap_or(false);

            if present {
                return true;
            }
        }

        logger::detail(&format!(
            "attempt {attempt}/{VERIFY_RETRIES}: not yet on-chain, retrying in {VERIFY_INTERVAL_SECS}s"
        ));
        tokio::time::sleep(Duration::from_secs(VERIFY_INTERVAL_SECS)).await;
    }

    false
}

async fn proposer_address(http: &reqwest::Client, cfg: &ContractsConfig) -> Result<String> {
    let value = probe::get_json(http, &format!("{}/cosmos/proposer", cfg.api_url))
        .await
        .ok_or_else(|| anyhow!("api did not return a proposer (OSMOSIS_PROPOSER_PRIVATE_KEY missing?)"))?;

    value
        .get("address")
        .and_then(|a| a.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("api proposer response missing address"))
}

async fn post(http: &reqwest::Client, url: &str, body: serde_json::Value) -> Result<serde_json::Value> {
    let resp = http
        .post(url)
        .json(&body)
        .timeout(Duration::from_secs(TX_WAIT_TIMEOUT_SECS + 30))
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        bail!("POST {url} -> {status}: {text}");
    }

    Ok(serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
}
