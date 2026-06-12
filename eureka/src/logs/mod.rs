use std::path::Path;

use anyhow::Result;

use crate::{logger, run};

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

        let out = run::capture_all(root, "docker", &["compose", "logs", "--since", since, svc])
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
            println!("    {line}");
        }
    }

    logger::hint("look for: 2/3 recv accepted — 08-wasm verified on-chain  →  3/3 ack accepted — round trip closed ✓");

    Ok(())
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
