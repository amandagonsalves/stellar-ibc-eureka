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
