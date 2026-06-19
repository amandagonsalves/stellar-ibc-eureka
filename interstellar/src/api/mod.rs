pub mod container;

#[derive(clap::Subcommand)]
pub enum ApiCmd {
    #[command(about = "Start the api container")]
    Start {
        #[arg(long, help = "Pull the latest image before starting")]
        pull: bool,
    },
    #[command(about = "Stop the api container")]
    Stop,
    #[command(about = "Restart the api container")]
    Restart {
        #[arg(long, help = "Pull the latest image and recreate the container")]
        pull: bool,
    },
}
