use std::path::Path;

use anyhow::Result;

use crate::config::ImageRef;
use crate::{logger, run};

const SERVICE: &str = "api";

pub fn start(image: &ImageRef, root: &Path, pull: bool) -> Result<()> {
    logger::banner("api start");
    logger::detail(&format!("image: {}", image.reference()));

    if pull {
        run::compose(root, &["pull", SERVICE])?;
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

pub fn restart(image: &ImageRef, root: &Path, pull: bool) -> Result<()> {
    logger::banner("api restart");
    logger::detail(&format!("image: {}", image.reference()));

    if pull {
        run::compose(root, &["pull", SERVICE])?;

        logger::step("docker compose up -d --force-recreate api");
        run::compose(root, &["up", "-d", "--force-recreate", SERVICE])?;
    } else {
        logger::step("docker compose restart api");
        run::compose(root, &["restart", SERVICE])?;
    }

    logger::ok("api restarted");

    Ok(())
}
