use std::path::Path;

use anyhow::Result;

use crate::config::ImageRef;
use crate::service::Service;

const SERVICE: Service = Service::new("api");

pub fn start(image: &ImageRef, root: &Path, pull: bool) -> Result<()> {
    SERVICE.start(root, image, pull)
}

pub fn stop(root: &Path) -> Result<()> {
    SERVICE.stop(root)
}

pub fn restart(image: &ImageRef, root: &Path, pull: bool) -> Result<()> {
    SERVICE.restart(root, image, pull)
}
