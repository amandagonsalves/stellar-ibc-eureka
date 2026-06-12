//! # stellar-api
//!
//! HTTP/REST service that fronts the Stellar IBC stack and a configured Cosmos
//! chain. Built on `axum`; entry point lives in [`runner::run`].
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
//! - `GET  /balance/{address}` — account balances ([`services::balance::balance`]).
//! - `POST /tx/prepare` — build an unsigned router-invoke tx for the relayer to sign ([`services::contract::prepare_invoke`]).
//! - `POST /tx/submit` — submit a relayer-signed tx ([`services::tx::submit_signed_tx`]).
//!
//! ### Cosmos read
//! - `GET  /cosmos/node-info` ([`services::cosmos::node_info`])
//! - `GET  /cosmos/proposer` ([`services::cosmos::proposer_info`])
//! - `GET  /cosmos/funder` ([`services::cosmos::funder_info`])
//! - `GET  /cosmos/gov/proposals?status=…` ([`services::cosmos::proposals`])
//! - `GET  /cosmos/gov/proposals/{id}` ([`services::cosmos::proposal_by_id`])
//! - `GET  /cosmos/gov/params/deposit` ([`services::cosmos::gov_deposit_params`])
//! - `GET  /cosmos/tx/{hash}` ([`services::cosmos::tx_by_hash`])
//! - `GET  /cosmos/ibc-wasm/checksums` ([`services::cosmos::ibc_wasm_checksums`])
//!
//! ### Cosmos write (signed by api-held keys)
//! - `POST /cosmos/ibc-wasm/store-code` ([`services::cosmos::submit_store_code`])
//! - `POST /cosmos/gov/vote` ([`services::cosmos::submit_vote`])
//! - `POST /cosmos/bank/send` ([`services::cosmos::submit_bank_send`])
//!
//! ### Hermes config
//! - `POST /hermes/wasm-checksum` ([`services::hermes::patch_wasm_checksum`])
//!
//! ## Configuration
//!
//! All settings come from environment variables; see [`config::ApiConfig`] for
//! the Stellar side and [`config::CosmosConfig`] for the Cosmos side. The api
//! holds two optional Cosmos signing keys:
//! - `COSMOS_PROPOSER_PRIVATE_KEY` — pays deposits and submits proposals.
//! - `COSMOS_FUNDER_PRIVATE_KEY` — sends bank transfers and casts weighted votes
//!   (typically the genesis validator on localnets).

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod config;
mod rpc;
pub mod runner;
pub mod services;
mod state;

pub use state::AppState;

#[tokio::main]
async fn main() {
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
