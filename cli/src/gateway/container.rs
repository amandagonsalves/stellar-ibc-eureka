use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::gateway::image;
use crate::{logger, run};

const SERVICE: &str = "gateway";

pub fn start(cfg: &Config, root: &Path, rebuild: bool) -> Result<()> {
    logger::banner("gateway start");

    if rebuild {
        image::build(cfg, root)?;
    }

    logger::step("docker compose up -d gateway");
    run::compose(root, &["up", "-d", SERVICE])?;

    logger::ok("gateway started");

    Ok(())
}

pub fn stop(root: &Path) -> Result<()> {
    logger::banner("gateway stop");

    logger::step("docker compose stop gateway");
    run::compose(root, &["stop", SERVICE])?;

    logger::ok("gateway stopped");

    Ok(())
}

pub fn restart(cfg: &Config, root: &Path, rebuild: bool) -> Result<()> {
    logger::banner("gateway restart");

    if rebuild {
        image::build(cfg, root)?;

        logger::step("docker compose up -d --force-recreate gateway");
        run::compose(root, &["up", "-d", "--force-recreate", SERVICE])?;
    } else {
        logger::step("docker compose restart gateway");
        run::compose(root, &["restart", SERVICE])?;
    }

    logger::ok("gateway restarted");

    Ok(())
}
