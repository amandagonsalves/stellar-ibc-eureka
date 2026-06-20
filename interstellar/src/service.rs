use std::path::Path;

use anyhow::Result;

use crate::config::ImageRef;
use crate::{logger, tools};

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
}
