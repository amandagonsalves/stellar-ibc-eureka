use std::path::Path;

use anyhow::{bail, Result};

use crate::config::ClientTypes;
use crate::tx::contracts::config::ContractsConfig;
use crate::{logger, shared};

/// Returns `true` if contracts were (re)deployed, `false` if the existing
/// deployment was kept — so callers know whether dependent services need to be
/// recreated to pick up a new `ROUTER_CONTRACT_ADDRESS`.
pub fn run(cfg: &ContractsConfig, root: &Path, force: bool, attestation: bool) -> Result<bool> {
    logger::banner("contracts deploy-all (build + deploy + wire router + write .env)");

    if cfg.signing_key.is_empty() {
        bail!("STELLAR_SIGNING_KEY is empty in .env — generate + fund a testnet key and set it");
    }

    if !cfg.ibc_router.is_empty() && !force {
        logger::warn(&format!(
            "ROUTER_CONTRACT_ADDRESS already set ({}). Use --force to redeploy.",
            cfg.ibc_router
        ));

        return Ok(false);
    }

    super::build::run(root)?;

    let deployer = deployer_address(cfg)?;
    logger::detail(&format!("deployer: {deployer}"));

    let wasm_dir = root.join("contracts/soroban/target/wasm32v1-none/contract");
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
        &wasm("stellar_ibc_transfer.wasm"),
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

    logger::step("deploying tendermint light client");
    let tendermint_id = super::deploy(
        cfg,
        root,
        &wasm("stellar_tendermint_light_client.wasm"),
        &[],
    )?;
    logger::ok(&format!("tendermint LC: {tendermint_id}"));

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
    logger::hint("recreate services to pick up ROUTER_CONTRACT_ADDRESS: interstellar api restart --pull && interstellar gateway restart --pull");

    Ok(true)
}

pub(crate) fn deployer_address(cfg: &ContractsConfig) -> Result<String> {
    if !cfg.deployer_address.is_empty() {
        return Ok(cfg.deployer_address.clone());
    }

    address_from_secret(&cfg.signing_key)
}

fn address_from_secret(secret: &str) -> Result<String> {
    let private = stellar_strkey::ed25519::PrivateKey::from_string(secret)
        .map_err(|e| anyhow::anyhow!("invalid STELLAR_SIGNING_KEY: {e}"))?;
    let signing = ed25519_dalek::SigningKey::from_bytes(&private.0);
    let public = stellar_strkey::ed25519::PublicKey(signing.verifying_key().to_bytes());

    Ok(String::from(public.to_string().as_str()))
}

#[cfg(test)]
mod tests {
    use super::address_from_secret;

    #[test]
    fn derives_the_stellar_address_from_the_secret() {
        let secret = "SCY47WDEEXHWBUD42ZYYG3DPIVX52RLQULF6KRPEEHLRU5TIWDKIJTGH";
        let address = "GCSWETSPE54NXRXLEXXI2FBIL2KRGOMF6JEJQ62OAGSXXOJVKO6WDAVO";

        assert_eq!(address_from_secret(secret).unwrap(), address);
    }
}
