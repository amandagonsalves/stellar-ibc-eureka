//! # stellar-api
//!
//! HTTP/REST service that fronts the Stellar IBC stack. Built on `axum`; entry
//! point lives in [`runner::run`]. Cosmos operations and hermes-config patching
//! are driven directly by the `interstellar` CLI, not through this service.
//!
//! Run locally with `cargo run -p stellar-api`; in the docker compose stack
//! the service listens on `${STELLAR_API_PORT}` (default `8101`).
//!
//! ## Routes
//!
//! ### Health + Stellar
//! - `GET  /health` — liveness + latest Stellar ledger.
//! - `GET  /ledger/latest` — latest Stellar ledger ([`services::ledgers::get_latest_ledger`]).
//! - `GET  /ledger/{sequence}` — fetch a specific ledger ([`services::ledgers::get_ledger`]).
//! - `GET  /events` — Soroban events ([`services::events::get_events`]).
//! - `GET  /stellar/transfer/balance/{denom}/{address}` — escrowed transfer balance ([`services::balance::transfer_balance`]).
//! - `GET  /stellar/clients` — list IBC clients ([`services::clients::list_clients`]).
//! - `POST /tx/prepare` — build an unsigned router-invoke tx for the relayer to sign ([`services::tx::prepare_tx`]).
//! - `POST /tx/submit` — submit a relayer-signed tx ([`services::tx::submit_signed_tx`]).
//!
//! ## Configuration
//!
//! All settings come from environment variables; see [`config::ApiConfig`].

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod config;
pub mod rpc;
pub mod runner;
pub mod services;
pub mod state;

pub use state::AppState;

pub async fn start() {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = config::ApiConfig::from_env();

    if let Err(error) = runner::run(cfg).await {
        tracing::error!("api server error: {error}");
        std::process::exit(1);
    }
}
