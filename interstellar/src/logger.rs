use std::io::IsTerminal;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressState, ProgressStyle};

/// Process-start instant, set once on first use (or explicitly via [`init`]).
/// The running spinner shows the elapsed time since this point as a single,
/// continuously-advancing clock across all phases.
static START: OnceLock<Instant> = OnceLock::new();

/// The spinner currently running, if any. While set, finished log lines are
/// printed *above* it (via `ProgressBar::println`) and subprocess output is fed
/// into it as the live message — so nothing fights the in-place repaint.
static CURRENT: OnceLock<Mutex<Option<ProgressBar>>> = OnceLock::new();

fn start() -> Instant {
    *START.get_or_init(Instant::now)
}

fn current() -> &'static Mutex<Option<ProgressBar>> {
    CURRENT.get_or_init(|| Mutex::new(None))
}

/// Anchor the running clock at process start (optional; the first log call does
/// it lazily, but calling this early makes the clock exact).
pub fn init() {
    let _ = start();
}

fn tty() -> bool {
    std::io::stdout().is_terminal()
}

fn paint(code: &str, text: &str) -> String {
    if tty() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Format a duration as `mm:ss` (minutes uncapped, e.g. `03:07`).
pub fn fmt_elapsed(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// The spinner currently running, if any (a cheap `Arc` clone).
pub fn current_bar() -> Option<ProgressBar> {
    current()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())
}

/// Print a finished line — above the active spinner if one is running so the
/// spinner stays put, otherwise straight to stdout.
fn emit(line: String) {
    match current_bar() {
        Some(bar) => bar.println(line),
        None => println!("{line}"),
    }
}

pub fn banner(text: &str) {
    emit(format!("\n{}", paint("1;36", &format!("=== {text} ==="))));
}

pub fn step(text: &str) {
    emit(format!("{} {}", paint("1;34", "▶"), paint("1", text)));
}

pub fn ok(text: &str) {
    emit(format!("  {} {}", paint("1;32", "✓"), text));
}

pub fn warn(text: &str) {
    emit(format!("  {} {}", paint("1;33", "!"), text));
}

pub fn fail(text: &str) {
    emit(format!("  {} {}", paint("1;31", "✗"), text));
}

pub fn detail(text: &str) {
    emit(format!("    {}", paint("2", text)));
}

/// A plain (undimmed) indented line — for dumping captured output such as relay
/// log lines above the running spinner.
pub fn plain(text: &str) {
    emit(format!("    {text}"));
}

pub fn hint(text: &str) {
    emit(format!("\n{} {}", paint("1;35", "→"), text));
}

/// A caribic-style background spinner. While held it repaints a single line a
/// few times a second — so the elapsed clock advances on screen even while a
/// long operation runs and prints nothing — and any subprocess launched through
/// [`crate::run::command`] streams its latest output line into the spinner's
/// message. Finished log lines print above it. Stops and clears its line when
/// dropped, so wrap a phase in a scope (or `drop` it) before the next phase.
///
/// No-op when stdout is not a TTY (piped / CI), so captured logs stay clean.
pub struct Ticker {
    bar: Option<ProgressBar>,
}

impl Drop for Ticker {
    fn drop(&mut self) {
        if let Some(bar) = self.bar.take() {
            bar.finish_and_clear();

            if let Ok(mut guard) = current().lock() {
                *guard = None;
            }
        }
    }
}

pub fn ticker(label: &str) -> Ticker {
    if !tty() {
        return Ticker { bar: None };
    }

    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{running}] {prefix:.bold} {wide_msg}")
            .unwrap()
            .with_key(
                "running",
                |_: &ProgressState, w: &mut dyn std::fmt::Write| {
                    let _ = write!(w, "{}", fmt_elapsed(start().elapsed()));
                },
            )
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    bar.set_prefix(label.to_string());
    bar.enable_steady_tick(Duration::from_millis(120));

    if let Ok(mut guard) = current().lock() {
        *guard = Some(bar.clone());
    }

    Ticker { bar: Some(bar) }
}

pub fn status_line(label: &str, up: bool, note: &str) {
    let dot = if up {
        paint("1;32", "●")
    } else {
        paint("1;31", "●")
    };
    let state = if up {
        paint("32", "up")
    } else {
        paint("31", "down")
    };
    emit(format!(
        "  {dot} {label:<16} {state:<6} {}",
        paint("2", note)
    ));
}
