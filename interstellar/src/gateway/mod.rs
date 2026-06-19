pub mod config;
pub mod container;
pub mod query;

#[derive(clap::Subcommand)]
pub enum GatewayCmd {
    #[command(about = "Start the gateway container")]
    Start {
        #[arg(long, help = "Pull the latest image before starting")]
        pull: bool,
    },
    #[command(about = "Stop the gateway container")]
    Stop,
    #[command(about = "Restart the gateway container")]
    Restart {
        #[arg(long, help = "Pull the latest image and recreate the container")]
        pull: bool,
    },
    #[command(about = "Direct gateway gRPC reads")]
    Query,
}
