use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use stellar_hermes_gateway::{config::GatewayConfig, runner};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = GatewayConfig::from_env();
    runner::run(cfg).await;
}
