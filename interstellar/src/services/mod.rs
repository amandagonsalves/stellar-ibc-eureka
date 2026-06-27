use std::path::Path;

use anyhow::{bail, Result};

use crate::config::{get, Config};
use crate::{logger, tools};

pub mod cosmos;
pub mod gateway;
pub mod hermes;
pub mod stellar;

#[derive(clap::Subcommand)]
pub enum ServicesCmd {
    #[command(about = "Pull the service image(s) — tag from .env; all when no flag")]
    Pull(ServiceArgs),
    #[command(about = "Pull + start the container(s); all when no flag")]
    Up(ServiceArgs),
    #[command(about = "Remove the existing container(s) then bring them up; all when no flag")]
    Restart(ServiceArgs),
    #[command(about = "Stop + remove the container(s); all when no flag")]
    Down(ServiceArgs),
    #[command(about = "Build the service image(s) — tag from .env; all when no flag")]
    Build(ServiceArgs),
    #[command(about = "Build + push the service image(s) — tag from .env; all when no flag")]
    Push(ServiceArgs),
}

#[derive(clap::Args)]
pub struct ServiceArgs {
    #[arg(long, help = "Act on the api service")]
    pub api: bool,
    #[arg(long, help = "Act on the gateway service")]
    pub gateway: bool,
    #[arg(long, help = "Act on the hermes service")]
    pub hermes: bool,
    #[arg(long, help = "Act on the cosmos service")]
    pub cosmos: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Api,
    Gateway,
    Hermes,
    Cosmos,
}

impl Service {
    fn compose(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Gateway => "gateway",
            Self::Hermes => "hermes",
            Self::Cosmos => "cosmos",
        }
    }

    fn buildable(self) -> bool {
        !matches!(self, Self::Cosmos)
    }

    fn image(self, cfg: &Config) -> String {
        match self {
            Self::Api => cfg.api.reference(),
            Self::Gateway => cfg.gateway.image.reference(),
            Self::Hermes => cfg.hermes.image.reference(),
            Self::Cosmos => get(
                "COSMOS_CHAIN_IMAGE",
                "ghcr.io/cosmos/ibc-go-wasm-simd:v11.0.0",
            ),
        }
    }
}

const ALL: [Service; 4] = [
    Service::Api,
    Service::Gateway,
    Service::Hermes,
    Service::Cosmos,
];

fn selected(args: &ServiceArgs) -> Vec<Service> {
    let mut picked = Vec::new();

    if args.api {
        picked.push(Service::Api);
    }

    if args.gateway {
        picked.push(Service::Gateway);
    }

    if args.hermes {
        picked.push(Service::Hermes);
    }

    if args.cosmos {
        picked.push(Service::Cosmos);
    }

    if picked.is_empty() {
        ALL.to_vec()
    } else {
        picked
    }
}

fn names(services: &[Service]) -> Vec<&'static str> {
    services.iter().map(|service| service.compose()).collect()
}

pub fn run(cfg: &Config, root: &Path, cmd: ServicesCmd) -> Result<()> {
    match cmd {
        ServicesCmd::Pull(args) => pull(root, &selected(&args)),
        ServicesCmd::Up(args) => up(root, &selected(&args)),
        ServicesCmd::Restart(args) => restart(root, &selected(&args)),
        ServicesCmd::Down(args) => down(root, &selected(&args)),
        ServicesCmd::Build(args) => build_or_push(cfg, root, &args, false),
        ServicesCmd::Push(args) => build_or_push(cfg, root, &args, true),
    }
}

fn pull(root: &Path, services: &[Service]) -> Result<()> {
    logger::banner(&format!("services pull ({})", names(services).join(", ")));

    let mut argv = vec!["pull"];
    argv.extend(names(services));

    tools::docker::compose(root, &argv)
}

pub fn up(root: &Path, services: &[Service]) -> Result<()> {
    pull(root, services)?;

    logger::banner(&format!("services up ({})", names(services).join(", ")));

    let mut argv = vec!["up", "-d"];
    argv.extend(names(services));

    tools::docker::compose(root, &argv)
}

fn down(root: &Path, services: &[Service]) -> Result<()> {
    logger::banner(&format!("services down ({})", names(services).join(", ")));

    let mut argv = vec!["rm", "-s", "-f"];
    argv.extend(names(services));

    tools::docker::compose(root, &argv)
}

fn restart(root: &Path, services: &[Service]) -> Result<()> {
    logger::banner(&format!(
        "services restart ({})",
        names(services).join(", ")
    ));

    down(root, services)?;
    up(root, services)
}

fn build_or_push(cfg: &Config, root: &Path, args: &ServiceArgs, push: bool) -> Result<()> {
    if args.cosmos {
        bail!("cosmos uses an upstream image — there is nothing to build or push");
    }

    for service in selected(args)
        .into_iter()
        .filter(|service| service.buildable())
    {
        build_one(cfg, root, service, push)?;
    }

    Ok(())
}

fn build_one(cfg: &Config, root: &Path, service: Service, push: bool) -> Result<()> {
    let verb = if push { "push" } else { "build" };
    logger::banner(&format!("services {verb} {}", service.compose()));
    logger::detail(&format!("image: {}", service.image(cfg)));

    let (dockerfile, context) = match service {
        Service::Api => ("crates/api/Dockerfile".to_string(), ".".to_string()),
        Service::Gateway => ("crates/gateway/Dockerfile".to_string(), ".".to_string()),
        Service::Hermes => {
            let repo = hermes_repo(cfg, root)?;

            (format!("{repo}/ci/release/hermes.Dockerfile"), repo)
        }
        Service::Cosmos => return Ok(()),
    };

    build_image(cfg, root, service, &dockerfile, &context, push)
}

fn build_image(
    cfg: &Config,
    root: &Path,
    service: Service,
    dockerfile: &str,
    context: &str,
    push: bool,
) -> Result<()> {
    let image = service.image(cfg);

    logger::step("docker build (host arch)");
    tools::docker::command(root, &["build", "-t", &image, "-f", dockerfile, context])?;

    if !push {
        return Ok(());
    }

    docker_login(root)?;

    logger::step("docker push");
    tools::docker::command(root, &["push", &image])
}

fn hermes_repo(cfg: &Config, root: &Path) -> Result<String> {
    let hermes = &cfg.hermes;

    if hermes.repo_url.trim().is_empty() {
        bail!("HERMES_REPO_URL is unset — set it to the hermes relayer repository to build hermes");
    }

    let checkout = root.join("target/hermes-relayer");
    let repo = checkout.display().to_string();

    if checkout.join(".git").is_dir() {
        logger::step(&format!("updating hermes relayer in {repo}"));
        tools::git::command(
            &checkout,
            &["remote", "set-url", "origin", &hermes.repo_url],
        )?;
    } else {
        logger::step(&format!("cloning hermes relayer from {}", hermes.repo_url));
        tools::git::command(root, &["clone", &hermes.repo_url, &repo])?;
    }

    logger::step(&format!("checking out {}", hermes.branch));
    tools::git::command(&checkout, &["fetch", "origin", &hermes.branch])?;
    tools::git::command(&checkout, &["checkout", &hermes.branch])?;
    tools::git::command(
        &checkout,
        &["reset", "--hard", &format!("origin/{}", hermes.branch)],
    )?;

    Ok(repo)
}

fn docker_login(root: &Path) -> Result<()> {
    let user = get("DOCKER_USERNAME", "");
    let token = get("DOCKER_TOKEN", "");

    if user.is_empty() || token.is_empty() {
        logger::detail("DOCKER_USERNAME / DOCKER_TOKEN unset — assuming an existing docker login");

        return Ok(());
    }

    logger::step("docker login");

    tools::docker::piped(root, &["login", "-u", &user, "--password-stdin"], &token)
}
