use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::contracts::config::ContractsConfig;
use crate::cosmos::tx::CosmosSigner;
use crate::{logger, run, tools};

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

const GOV_AUTHORITY: &str = "cosmos10d07y265gmmuvt4z0w9aw880jnsr700j6zn9kn";
const TESTNET_DEPOSIT_UATOM: u64 = 10_000_000;

pub async fn upload(
    cfg: &ContractsConfig,
    root: &Path,
    http: &reqwest::Client,
    testnet: bool,
    from: Option<&str>,
) -> Result<()> {
    if testnet {
        return prepare_testnet_proposal(root, from);
    }

    logger::banner("contracts upload-wasm (light-client-wasm -> Cosmos 08-wasm)");

    let (bytes, local_sha) = build_wasm(root)?;

    let cosmos = crate::cosmos::config::CosmosConfig::devnet();
    let signer = CosmosSigner::from_config(&cosmos, http.clone())?;

    if !signer.node_info_ok().await {
        logger::warn(&format!(
            "cosmos REST not reachable at {} — start it with: interstellar cosmos start",
            cosmos.rest_url
        ));

        return Ok(());
    }

    let proposer = signer.proposer_address()?.to_string();
    logger::step(&format!("funding proposer {proposer}"));
    if signer
        .fund_account(
            &proposer,
            FUND_AMOUNT as u128,
            FUND_GAS_LIMIT,
            FUND_FEE_AMOUNT as u128,
            true,
        )
        .await?
    {
        logger::ok("proposer funded");
    } else {
        logger::detail("proposer already has an account, skipping funding");
    }

    logger::step("submitting store-code proposal");
    let proposal_id = signer
        .submit_store_code(
            bytes,
            format!("upload-light-client-wasm: {CRATE}"),
            "Registers light_client_wasm.wasm for the 08-wasm client type".to_string(),
            DEPOSIT_AMOUNT as u128,
            STORE_GAS_LIMIT,
            STORE_FEE_AMOUNT as u128,
            Duration::from_secs(TX_WAIT_TIMEOUT_SECS),
        )
        .await?;
    logger::ok(&format!("proposal id: {proposal_id}"));

    logger::step(&format!("voting YES on proposal {proposal_id}"));
    signer
        .vote(
            proposal_id,
            VOTE_OPTION as i32,
            VOTE_GAS_LIMIT,
            VOTE_FEE_AMOUNT as u128,
        )
        .await?;

    logger::detail(&format!(
        "waiting {VOTING_PERIOD_SECS}s for the voting period"
    ));
    tokio::time::sleep(Duration::from_secs(VOTING_PERIOD_SECS)).await;

    logger::step("verifying checksum on-chain");
    if !checksum_registered(&signer, local_sha.as_str()).await {
        bail!("local sha256 {local_sha} did not appear in on-chain checksums (proposal may not have passed)");
    }
    logger::ok(&format!("wasm registered with checksum {local_sha}"));

    logger::step("patching wasm_checksum_hex in the hermes config");
    if crate::hermes::patch_wasm_checksum(Path::new(&cfg.hermes_config), &local_sha)? {
        logger::ok(&format!("hermes config patched ({})", cfg.hermes_config));
    } else {
        logger::detail("hermes config already had this checksum");
    }

    Ok(())
}

fn build_wasm(root: &Path) -> Result<(Vec<u8>, String)> {
    logger::step("cargo build --target wasm32-unknown-unknown -p light-client-wasm --release");
    run::command(
        root,
        "cargo",
        &[
            "build",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            CRATE,
            "--release",
        ],
    )?;

    let wasm_file = root.join("target/wasm32-unknown-unknown/release/light_client_wasm.wasm");
    if !wasm_file.exists() {
        bail!(
            "expected wasm artifact not found at {}",
            wasm_file.display()
        );
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
        logger::warn(
            "wasm-opt not installed — install binaryen if the upload is rejected for bulk-memory",
        );
    }

    let bytes =
        std::fs::read(&wasm_file).with_context(|| format!("reading {}", wasm_file.display()))?;
    let sha = hex::encode(Sha256::digest(&bytes));
    logger::detail(&format!("{} bytes, sha256={sha}", bytes.len()));

    Ok((bytes, sha))
}

fn prepare_testnet_proposal(root: &Path, from: Option<&str>) -> Result<()> {
    logger::banner("contracts upload-wasm --testnet (08-wasm store-code gov proposal)");

    let testnet = crate::cosmos::config::CosmosConfig::testnet();
    let (bytes, sha) = build_wasm(root)?;
    let wasm_b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let authority = crate::config::get("COSMOS_TESTNET_GOV_AUTHORITY", GOV_AUTHORITY);
    let proposal = serde_json::json!({
        "messages": [{
            "@type": "/ibc.lightclients.wasm.v1.MsgStoreCode",
            "signer": authority,
            "wasm_byte_code": wasm_b64,
        }],
        "title": "Store Stellar 08-wasm light client",
        "summary": "Registers the Stellar light client wasm for the 08-wasm client type",
        "deposit": format!("{TESTNET_DEPOSIT_UATOM}uatom"),
    });

    let proposal_path = root.join("provider-store-code-proposal.json");
    std::fs::write(&proposal_path, serde_json::to_string_pretty(&proposal)?)
        .with_context(|| format!("writing {}", proposal_path.display()))?;
    logger::ok(&format!("wrote {}", proposal_path.display()));

    let rpc = testnet.rpc_url.as_str();
    let chain_id = testnet.chain_id.as_str();
    let keyring = crate::config::get("COSMOS_TESTNET_KEYRING_BACKEND", "test");
    let path = proposal_path.display().to_string();

    match resolve_testnet_key(root, from, &testnet, &keyring)? {
        Some(key) => {
            logger::step(&format!(
                "gaiad tx gov submit-proposal (from={key}, node={rpc}, chain-id={chain_id})"
            ));
            tools::gaiad::command(
                root,
                &[
                    "tx",
                    "gov",
                    "submit-proposal",
                    path.as_str(),
                    "--from",
                    key.as_str(),
                    "--keyring-backend",
                    keyring.as_str(),
                    "--node",
                    rpc,
                    "--chain-id",
                    chain_id,
                    "--gas",
                    "auto",
                    "--gas-adjustment",
                    "1.5",
                    "--gas-prices",
                    "0.025uatom",
                    "-y",
                ],
            )?;
            logger::ok("proposal submitted");
        }
        None => {
            logger::step("submit (set COSMOS_TESTNET_MNEMONIC, or pass --from <gaiad-key>, to run this for you):");
            println!("  gaiad tx gov submit-proposal {path} \\");
            println!("    --from <key> --keyring-backend {keyring} --node {rpc} --chain-id {chain_id} \\");
            println!("    --gas auto --gas-adjustment 1.5 --gas-prices 0.025uatom -y");
        }
    }

    println!();
    logger::step("it must pass governance, then confirm + wire it up:");
    println!("  gaiad query ibc-wasm checksums --node {rpc}");
    logger::step(&format!(
        "set it in hermes-config.toml:  wasm_checksum_hex = '{sha}'"
    ));

    if let Some(faucet) = &testnet.faucet_url {
        logger::hint(&format!("fund your key first via the faucet: {faucet}"));
    }
    logger::warn(
        "provider is ibc-go v10 / wasmvm 2.x — confirm the wasm + 08-wasm ABI before spending on the proposal",
    );

    Ok(())
}

fn resolve_testnet_key(
    root: &Path,
    from: Option<&str>,
    testnet: &crate::cosmos::config::CosmosConfig,
    keyring: &str,
) -> Result<Option<String>> {
    if let Some(key) = from {
        if !run::has("gaiad") {
            bail!("gaiad not found in PATH — install Gaia v27 to submit");
        }

        return Ok(Some(key.to_string()));
    }

    let mnemonic = {
        let testnet_specific = crate::config::get("COSMOS_TESTNET_MNEMONIC", "");
        if testnet_specific.trim().is_empty() {
            testnet.relayer_mnemonic.clone()
        } else {
            testnet_specific
        }
    };
    if mnemonic.trim().is_empty() {
        return Ok(None);
    }

    if !run::has("gaiad") {
        bail!("gaiad not found in PATH — install Gaia v27 to use the env mnemonic");
    }

    let key_name = crate::config::get("COSMOS_TESTNET_KEY_NAME", "interstellar-cosmos");
    ensure_gaiad_key(root, &key_name, mnemonic.trim(), keyring)?;

    Ok(Some(key_name))
}

fn ensure_gaiad_key(root: &Path, name: &str, mnemonic: &str, keyring: &str) -> Result<()> {
    if tools::gaiad::capture_quiet(
        root,
        &["keys", "show", name, "-a", "--keyring-backend", keyring],
    )
    .is_ok()
    {
        logger::detail(&format!("gaiad key '{name}' already in the keyring"));

        return Ok(());
    }

    logger::step(&format!(
        "importing mnemonic into the gaiad keyring as '{name}' (keyring-backend {keyring})"
    ));
    tools::gaiad::piped(
        root,
        &[
            "keys",
            "add",
            name,
            "--recover",
            "--keyring-backend",
            keyring,
        ],
        &format!("{mnemonic}\n"),
    )?;

    Ok(())
}

async fn checksum_registered(signer: &CosmosSigner, local_sha: &str) -> bool {
    for attempt in 1..=VERIFY_RETRIES {
        if let Ok(checksums) = signer.checksums().await {
            if checksums.iter().any(|c| c.eq_ignore_ascii_case(local_sha)) {
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
