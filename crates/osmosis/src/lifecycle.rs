use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::{config, health, home_dir};

pub fn find_compose_file() -> Result<PathBuf> {
    if let Ok(value) = std::env::var("STELLAR_IBC_COMPOSE_FILE") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
        bail!(
            "STELLAR_IBC_COMPOSE_FILE points to a missing file: {}",
            path.display()
        );
    }

    let mut dir = std::env::current_dir().context("failed to read current directory")?;
    loop {
        let candidate = dir.join("docker-compose.yml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }

    bail!(
        "Could not locate docker-compose.yml. Run from inside the stellar-ibc repo or set STELLAR_IBC_COMPOSE_FILE."
    )
}

fn compose(compose_file: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg(compose_file)
        .arg("--profile")
        .arg(config::PROFILE)
        .args(args)
        .status()
        .context("failed to invoke `docker compose` (is Docker installed and running?)")?;

    if !status.success() {
        bail!("`docker compose {}` exited with {}", args.join(" "), status);
    }
    Ok(())
}

fn local_home_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join(config::LOCAL_HOME_DIR_NAME))
}

fn clean_local_state(compose_file: &Path) {
    let _ = compose(compose_file, &["down"]);

    if let Some(home) = local_home_dir() {
        if home.exists() {
            match std::fs::remove_dir_all(&home) {
                Ok(()) => println!("Removed existing local Osmosis state at {}", home.display()),
                Err(error) => eprintln!(
                    "warning: could not remove {} ({error}); continuing with existing state",
                    home.display()
                ),
            }
        }
    }
}

pub async fn start(stateful: bool) -> Result<()> {
    let compose_file = find_compose_file()?;

    if stateful {
        println!("Starting Osmosis appchain (keeping existing state) ...");
    } else {
        println!("Starting Osmosis appchain (fresh state) ...");
        clean_local_state(&compose_file.as_path());
    }

    compose(&compose_file.as_path(), &["up", "-d", config::SERVICE])?;
    health::wait_until_healthy().await
}

pub fn stop() -> Result<()> {
    let compose_file = find_compose_file()?;
    compose(&compose_file.as_path(), &["down"])
}
