use std::path::Path;

use anyhow::Result;

use crate::config::ImageRef;
use crate::{logger, run};

const SERVICE: &str = "gateway";

pub fn start(image: &ImageRef, root: &Path, pull: bool) -> Result<()> {
    logger::banner("gateway start");
    logger::detail(&format!("image: {}", image.reference()));

    if pull {
        run::compose(root, &["pull", SERVICE])?;
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

pub fn restart(image: &ImageRef, root: &Path, pull: bool) -> Result<()> {
    logger::banner("gateway restart");
    logger::detail(&format!("image: {}", image.reference()));

    if pull {
        run::compose(root, &["pull", SERVICE])?;

        logger::step("docker compose up -d --force-recreate gateway");
        run::compose(root, &["up", "-d", "--force-recreate", SERVICE])?;
    } else {
        logger::step("docker compose restart gateway");
        run::compose(root, &["restart", SERVICE])?;
    }

    logger::ok("gateway restarted");

    Ok(())
}
