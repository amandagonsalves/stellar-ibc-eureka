use std::path::Path;

use anyhow::Result;

use crate::ops::config::OpsConfig;
use crate::{logger, probe, run, shared};

pub async fn run(root: &Path, cfg: &OpsConfig, http: &reqwest::Client) -> Result<()> {
    logger::banner("doctor — prerequisites & configuration");

    logger::step("Toolchain");
    shared::check(
        "docker",
        run::has("docker"),
        "required to run the chain + services",
    );
    shared::check(
        "stellar",
        run::has("stellar"),
        "Soroban CLI, used to build/deploy contracts",
    );
    shared::check(
        "cargo",
        run::has("cargo"),
        "builds the wasm light client + this CLI",
    );

    logger::step("Repository");
    logger::ok(&format!("repo root: {}", root.display()));

    let env_file = root.join(".env");

    if env_file.exists() {
        logger::ok(".env present");
    } else {
        logger::fail(".env missing — copy .env.example and fill it in");
    }

    logger::step("Configuration");
    shared::flag(
        "STELLAR_SIGNING_KEY",
        !cfg.stellar_signing_key.is_empty(),
        "needed to deploy + sign on Stellar",
    );
    shared::flag(
        "ROUTER_CONTRACT_ADDRESS",
        !cfg.ibc_router.is_empty(),
        "router address (set by `stellaribc contracts deploy-all`)",
    );
    shared::flag(
        "TRANSFER_CONTRACT_ADDRESS",
        !cfg.transfer_app.is_empty(),
        "transfer-app address",
    );
    shared::flag(
        "STELLAR_CLIENT_ID",
        !cfg.stellar_client_id.is_empty(),
        "08-wasm client id (set by `stellaribc clients stellar`)",
    );

    logger::step("Services");

    let cosmos = probe::http_ok(http, &format!("{}/cosmos", cfg.cosmos_rest_url)).await;
    logger::status_line(&cfg.cosmos_chain_id, cosmos, &cfg.cosmos_rest_url);

    let api = probe::http_ok(http, &format!("{}/health", cfg.api_url)).await;
    logger::status_line("stellar-api", api, &cfg.api_url);

    let gateway = probe::tcp_ok(&cfg.gateway_url);
    logger::status_line("gateway-grpc", gateway, &cfg.gateway_url);

    if !cosmos || !api {
        logger::hint("bring the stack up with: stellaribc up");
    }

    Ok(())
}
