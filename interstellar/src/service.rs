use std::path::Path;

use anyhow::Result;

use crate::config::ImageRef;
use crate::{logger, tools};

/// Generic docker-compose service wrapper shared by the api / gateway / hermes
/// modules. Each per-service module supplies its compose service name and image;
/// the start / stop / restart mechanics live here so they're defined once.
pub struct Service {
    name: &'static str,
}

impl Service {
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub fn start(&self, root: &Path, image: &ImageRef, pull: bool) -> Result<()> {
        logger::banner(&format!("{} start", self.name));
        logger::detail(&format!("image: {}", image.reference()));

        if pull {
            tools::docker::compose(root, &["pull", self.name])?;
        }

        logger::step(&format!("docker compose up -d {}", self.name));
        tools::docker::compose(root, &["up", "-d", self.name])?;

        logger::ok(&format!("{} started", self.name));

        Ok(())
    }

    pub fn stop(&self, root: &Path) -> Result<()> {
        logger::banner(&format!("{} stop", self.name));

        logger::step(&format!("docker compose stop {}", self.name));
        tools::docker::compose(root, &["stop", self.name])?;

        logger::ok(&format!("{} stopped", self.name));

        Ok(())
    }

    pub fn restart(&self, root: &Path, image: &ImageRef, pull: bool) -> Result<()> {
        logger::banner(&format!("{} restart", self.name));
        logger::detail(&format!("image: {}", image.reference()));

        if pull {
            tools::docker::compose(root, &["pull", self.name])?;

            logger::step(&format!(
                "docker compose up -d --force-recreate {}",
                self.name
            ));
            tools::docker::compose(root, &["up", "-d", "--force-recreate", self.name])?;
        } else {
            logger::step(&format!("docker compose restart {}", self.name));
            tools::docker::compose(root, &["restart", self.name])?;
        }

        logger::ok(&format!("{} restarted", self.name));

        Ok(())
    }
}
