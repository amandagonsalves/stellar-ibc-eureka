use std::path::Path;

use anyhow::{Context, Result};

use crate::logger;

pub fn env_upsert(path: &Path, updates: &[(&str, &str)]) -> Result<()> {
    let mut text = std::fs::read_to_string(path).unwrap_or_default();

    for (key, value) in updates {
        if value.is_empty() {
            continue;
        }

        let rendered = if value.contains(char::is_whitespace) {
            format!("\"{value}\"")
        } else {
            (*value).to_string()
        };

        let prefix = format!("{key}=");
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        let mut replaced = false;

        for line in lines.iter_mut() {
            if line.trim_start().starts_with(&prefix) {
                *line = format!("{key}={rendered}");
                replaced = true;
                break;
            }
        }

        if !replaced {
            lines.push(format!("{key}={rendered}"));
        }

        text = lines.join("\n");
        text.push('\n');
    }

    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;

    Ok(())
}

pub fn pending(label: &str, reason: &str) {
    logger::banner(label);

    logger::warn("not wired yet");
    logger::detail(reason);
}

pub fn print_clients(value: &serde_json::Value) {
    let Some(clients) = value.get("clients").and_then(|c| c.as_array()) else {
        logger::warn("unexpected response shape from /stellar/clients");

        return;
    };

    if clients.is_empty() {
        logger::detail("no clients created yet");

        return;
    }

    for client in clients {
        let client_type = client
            .get("client_type")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        let ids = client
            .get("client_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        logger::ok(&format!("{client_type}: {ids}"));
    }
}

pub fn check(name: &str, present: bool, note: &str) {
    if present {
        logger::ok(&format!("{name} found"));
    } else {
        logger::fail(&format!("{name} not found — {note}"));
    }
}

pub fn flag(name: &str, set: bool, note: &str) {
    if set {
        logger::ok(&format!("{name} set"));
    } else {
        logger::warn(&format!("{name} unset — {note}"));
    }
}

pub fn contract(label: &str, id: &str) {
    if id.is_empty() {
        logger::warn(&format!("{label:<13} (unset)"));
    } else {
        logger::ok(&format!("{label:<13} {id}"));
    }
}
