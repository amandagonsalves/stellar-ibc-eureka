use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;

use crate::{logger, tools};

#[derive(clap::Args)]
pub struct LogsArgs {
    #[arg(
        long,
        default_value = "120s",
        help = "How far back to pull container logs"
    )]
    pub since: String,
}

const MARKERS: [&str; 6] = [
    "[stellar→cosmos]",
    "[cosmos→stellar]",
    "found router event",
    "applied ibc state changes into smt",
    "served packet commitment proof",
    "round trip",
];

const SERVICES: [&str; 2] = ["gateway", "hermes"];

pub fn run(root: &Path, since: &str) -> Result<()> {
    logger::banner("relay logs (the staged round trip)");

    for svc in SERVICES {
        logger::step(svc);

        let out = tools::docker::capture_all(root, &["compose", "logs", "--since", since, svc])
            .unwrap_or_default();

        let hero: Vec<String> = out
            .lines()
            .filter(|line| is_hero(line))
            .map(strip_ansi)
            .collect();

        if hero.is_empty() {
            logger::detail("(no relay lines in window — widen with --since or run a transfer)");

            continue;
        }

        let start = hero.len().saturating_sub(25);

        for line in &hero[start..] {
            logger::plain(line);
        }
    }

    logger::hint("look for: 2/3 recv accepted — 08-wasm verified on-chain  →  3/3 ack accepted — round trip closed ✓");

    Ok(())
}

/// Poll the gateway + hermes logs and print each relay hero line as it appears,
/// returning as soon as the round trip closes (or `timeout_secs` elapses).
///
/// Unlike a blind sleep, this keeps the screen moving — the recv → ack →
/// round-trip lines stream in live, which is the moment a demo recording wants.
/// Returns `true` if the `round trip` marker was seen.
pub async fn watch(root: &Path, since: &str, timeout_secs: u64) -> Result<bool> {
    logger::banner("relay — watching the round trip");
    logger::detail("tailing gateway + hermes for the recv → ack hero lines");

    const POLL_SECS: u64 = 3;
    let polls = (timeout_secs / POLL_SECS).max(1);
    let mut seen: HashSet<String> = HashSet::new();
    let mut closed = false;

    for _ in 0..polls {
        for svc in SERVICES {
            let out = tools::docker::capture_all(root, &["compose", "logs", "--since", since, svc])
                .unwrap_or_default();

            for line in out.lines().filter(|line| is_hero(line)) {
                let clean = strip_ansi(line);

                if seen.insert(clean.clone()) {
                    logger::plain(&clean);

                    if clean.contains("round trip") {
                        closed = true;
                    }
                }
            }
        }

        if closed {
            break;
        }

        tokio::time::sleep(Duration::from_secs(POLL_SECS)).await;
    }

    if closed {
        logger::ok(
            "round trip closed ✓ — recv verified on-chain by the 08-wasm LC, ack relayed back",
        );
    } else {
        logger::warn(&format!(
            "no round-trip marker within {timeout_secs}s — relay may still be in flight (interstellar logs)"
        ));
    }

    Ok(closed)
}

fn is_hero(line: &str) -> bool {
    MARKERS.iter().any(|marker| line.contains(marker))
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            for esc in chars.by_ref() {
                if esc == 'm' {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }

    out
}
