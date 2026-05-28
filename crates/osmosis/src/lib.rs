use std::path::PathBuf;

mod config;
mod health;
mod lifecycle;

pub use config::{CHAIN_ID, GRPC_URL, REST_URL, RPC_URL, STATUS_URL};

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub async fn start(stateful: bool) -> anyhow::Result<()> {
    lifecycle::start(stateful).await
}

pub fn stop() -> anyhow::Result<()> {
    lifecycle::stop()
}

pub async fn report() -> anyhow::Result<()> {
    health::report().await
}
