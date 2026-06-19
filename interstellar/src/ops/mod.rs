pub mod check;
pub mod config;
pub mod install;
pub mod stack;
pub mod start;
pub mod status;

#[derive(clap::Args)]
pub struct UpArgs {
    #[arg(long, help = "Start only the Cosmos chain (cosmos)")]
    pub cosmos: bool,
    #[arg(long, help = "Start only the Stellar-side services (api + gateway)")]
    pub stellar: bool,
}

#[derive(clap::Args)]
pub struct DownArgs {
    #[arg(long, help = "Also remove named volumes (wipes chain + key state)")]
    pub volumes: bool,
}

#[derive(clap::Args)]
pub struct StartArgs {
    #[arg(long, help = "Skip pulling the docker images")]
    pub skip_images: bool,
    #[arg(long, help = "Skip the Soroban contract deploy")]
    pub skip_contracts: bool,
    #[arg(long, help = "Skip the light-client-wasm upload")]
    pub skip_wasm: bool,
    #[arg(long, help = "Skip importing the hermes relayer keys")]
    pub skip_keys: bool,
    #[arg(long, help = "Skip provisioning the sender + receiver accounts")]
    pub skip_accounts: bool,
    #[arg(
        long,
        help = "Redeploy contracts even if ROUTER_CONTRACT_ADDRESS is already set"
    )]
    pub force_redeploy: bool,
}
