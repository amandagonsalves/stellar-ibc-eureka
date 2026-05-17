use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod api;
pub mod config;
pub mod msg;
pub mod proto;
pub mod query;
pub mod rpc;
pub mod runner;
pub mod state;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = config::GatewayConfig::from_env();

    runner::run(cfg).await;
}
