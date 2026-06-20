pub mod config;
pub mod query;

#[derive(clap::Subcommand)]
pub enum GatewayCmd {
    #[command(about = "Direct gateway gRPC reads")]
    Query,
}
