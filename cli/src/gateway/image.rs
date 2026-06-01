use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::{logger, run};

pub fn build(cfg: &Config, root: &Path) -> Result<()> {
    logger::banner("gateway build-image");

    let local_ref = cfg.gateway_local_ref();
    let dockerfile = root.join("Dockerfile");

    logger::step(&format!("docker build -t {local_ref}"));
    run::command(
        root,
        "docker",
        &[
            "build",
            "-t",
            &local_ref,
            "-f",
            dockerfile.to_str().unwrap_or("Dockerfile"),
            ".",
        ],
    )?;

    logger::ok(&format!("built {local_ref}"));

    Ok(())
}

pub fn push(cfg: &Config, root: &Path, rebuild: bool) -> Result<()> {
    logger::banner("gateway push-image");

    if rebuild {
        build(cfg, root)?;
    }

    let local_ref = cfg.gateway_local_ref();
    let remote_ref = cfg.gateway_remote_ref();

    if local_ref != remote_ref {
        logger::step(&format!("docker tag {local_ref} {remote_ref}"));
        run::command(root, "docker", &["tag", &local_ref, &remote_ref])?;
    }

    if cfg.docker_username.is_empty() || cfg.docker_token.is_empty() {
        logger::warn("DOCKER_USERNAME/DOCKER_TOKEN unset — relying on an existing docker login");
    } else {
        logger::step(&format!("docker login as {}", cfg.docker_username));
        run::docker_login(&cfg.docker_username, &cfg.docker_token)?;
    }

    logger::step(&format!("docker push {remote_ref}"));
    run::command(root, "docker", &["push", &remote_ref])?;

    logger::ok(&format!("pushed {remote_ref}"));

    Ok(())
}
