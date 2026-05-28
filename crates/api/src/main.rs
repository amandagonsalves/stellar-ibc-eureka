use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod config;
pub(crate) mod endpoints;
pub mod runner;
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
