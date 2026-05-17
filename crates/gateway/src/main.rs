use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod config;
pub mod proto;
pub mod rpc;
pub mod runner;
pub mod state;
pub mod query;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("protos built");

    let cfg = config::GatewayConfig::from_env();

    runner::run(cfg).await;
}
