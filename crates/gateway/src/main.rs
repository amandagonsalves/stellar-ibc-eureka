use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod config;
pub mod proto;

fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
