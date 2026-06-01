use std::path::Path;

use anyhow::Result;

use crate::api::image;
use crate::config::Config;
use crate::{logger, run};

const SERVICE: &str = "api";

pub fn start(cfg: &Config, root: &Path, rebuild: bool) -> Result<()> {
    logger::banner("api start");

    if rebuild {
        image::build(cfg, root)?;
    }

    logger::step("docker compose up -d api");
    run::compose(root, &["up", "-d", SERVICE])?;

    logger::ok("api started");

    Ok(())
}

pub fn stop(root: &Path) -> Result<()> {
    logger::banner("api stop");

    logger::step("docker compose stop api");
    run::compose(root, &["stop", SERVICE])?;

    logger::ok("api stopped");

    Ok(())
}

pub fn restart(cfg: &Config, root: &Path, rebuild: bool) -> Result<()> {
    logger::banner("api restart");

    if rebuild {
        image::build(cfg, root)?;

        logger::step("docker compose up -d --force-recreate api");
        run::compose(root, &["up", "-d", "--force-recreate", SERVICE])?;
    } else {
        logger::step("docker compose restart api");
        run::compose(root, &["restart", SERVICE])?;
    }

    logger::ok("api restarted");

    Ok(())
}
