pub mod config;
pub mod container;
pub mod keys;

use std::path::Path;

use anyhow::{bail, Context, Result};

pub fn patch_wasm_checksum(config_path: &Path, checksum: &str) -> Result<bool> {
    let text = std::fs::read_to_string(config_path)
        .with_context(|| format!("reading {}", config_path.display()))?;

    let mut found = false;
    let mut changed = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("wasm_checksum_hex") {
                return line.to_string();
            }
            found = true;
            let indent = &line[..line.len() - trimmed.len()];
            let replaced = format!("{indent}wasm_checksum_hex = '{checksum}'");
            if replaced != line {
                changed = true;
            }
            replaced
        })
        .collect();

    if !found {
        bail!(
            "wasm_checksum_hex line not found in {}",
            config_path.display()
        );
    }

    if changed {
        let mut out = lines.join("\n");
        if text.ends_with('\n') {
            out.push('\n');
        }
        std::fs::write(config_path, out)
            .with_context(|| format!("writing {}", config_path.display()))?;
    }

    Ok(changed)
}
