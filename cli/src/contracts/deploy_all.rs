use std::path::Path;

use anyhow::{bail, Result};

use crate::config::ClientTypes;
use crate::contracts::config::ContractsConfig;
use crate::{logger, run, shared};

pub fn run(
    cfg: &ContractsConfig,
    root: &Path,
    force: bool,
    attestation: bool,
    tendermint: bool,
) -> Result<()> {
    logger::banner("contracts deploy-all (build + deploy + wire router + write .env)");

    if cfg.signing_key.is_empty() {
        bail!("STELLAR_SIGNING_KEY is empty in .env — generate + fund a testnet key and set it");
    }

    if !cfg.ibc_router.is_empty() && !force {
        logger::warn(&format!(
            "ROUTER_CONTRACT_ADDRESS already set ({}). Use --force to redeploy.",
            cfg.ibc_router
        ));

        return Ok(());
    }

    super::build::run(root)?;

    let deployer = deployer_address(cfg, root)?;
    logger::detail(&format!("deployer: {deployer}"));

    let wasm_dir = root.join("contracts/target/wasm32v1-none/contract");
    let wasm = |name: &str| wasm_dir.join(name).display().to_string();

    logger::step("deploying mock light client");
    let mock = super::deploy(cfg, root, &wasm("stellar_mock_light_client.wasm"), &[])?;
    logger::ok(&format!("mock LC: {mock}"));

    logger::step("deploying router");
    let router = super::deploy(
        cfg,
        root,
        &wasm("stellar_ibc_router.wasm"),
        &["--admin", &deployer],
    )?;
    logger::ok(&format!("router: {router}"));

    logger::step("deploying transfer-app");
    let transfer = super::deploy(
        cfg,
        root,
        &wasm("stellar_transfer_app.wasm"),
        &["--router", &router, "--admin", &deployer],
    )?;
    logger::ok(&format!("transfer-app: {transfer}"));

    let mut attestation_id = String::new();
    if attestation {
        logger::step("deploying attestation light client");
        attestation_id = super::deploy(
            cfg,
            root,
            &wasm("stellar_attestation_light_client.wasm"),
            &[],
        )?;
        logger::ok(&format!("attestation LC: {attestation_id}"));
    }

    let mut tendermint_id = String::new();
    if tendermint {
        logger::step("deploying tendermint light client");
        tendermint_id = super::deploy(
            cfg,
            root,
            &wasm("stellar_tendermint_light_client.wasm"),
            &[],
        )?;
        logger::ok(&format!("tendermint LC: {tendermint_id}"));
    }

    logger::step("wiring router (register_client_type + register_port)");
    super::invoke(
        cfg,
        root,
        &router,
        &[
            "register_client_type",
            "--client_type",
            cfg.client_type(ClientTypes::Mock),
            "--lc_address",
            &mock,
        ],
    )?;

    if !attestation_id.is_empty() {
        super::invoke(
            cfg,
            root,
            &router,
            &[
                "register_client_type",
                "--client_type",
                cfg.client_type(ClientTypes::Attestation),
                "--lc_address",
                &attestation_id,
            ],
        )?;
    }

    if !tendermint_id.is_empty() {
        super::invoke(
            cfg,
            root,
            &router,
            &[
                "register_client_type",
                "--client_type",
                cfg.client_type(ClientTypes::Tendermint),
                "--lc_address",
                &tendermint_id,
            ],
        )?;
    }

    super::invoke(
        cfg,
        root,
        &router,
        &[
            "register_port",
            "--port_id",
            &cfg.transfer_port,
            "--app_address",
            &transfer,
        ],
    )?;

    logger::step("writing contract ids to .env");
    shared::env_upsert(
        &root.join(".env"),
        &[
            ("ROUTER_CONTRACT_ADDRESS", router.as_str()),
            ("TRANSFER_CONTRACT_ADDRESS", transfer.as_str()),
            ("MOCK_LC_CONTRACT_ID", mock.as_str()),
            ("ATTESTATION_LC_CONTRACT_ID", attestation_id.as_str()),
            ("TENDERMINT_LC_CONTRACT_ID", tendermint_id.as_str()),
            ("DEPLOYER_ADDRESS", deployer.as_str()),
        ],
    )?;

    logger::ok("deploy-all complete");
    logger::hint("recreate services to pick up ROUTER_CONTRACT_ADDRESS: stellaribc api restart --pull && stellaribc gateway restart --pull");

    Ok(())
}

fn deployer_address(cfg: &ContractsConfig, root: &Path) -> Result<String> {
    if !cfg.deployer_address.is_empty() {
        return Ok(cfg.deployer_address.clone());
    }

    let out = run::capture(
        root,
        "stellar",
        &["keys", "public-key", cfg.cli_identity.as_str()],
    )?;
    let addr = super::last_line(&out);

    if addr.is_empty() {
        bail!(
            "could not resolve deployer address for identity '{}'",
            cfg.cli_identity
        );
    }

    Ok(addr)
}
