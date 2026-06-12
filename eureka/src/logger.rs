use std::io::IsTerminal;

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

pub fn banner(text: &str) {
    println!("\n{}", paint("1;36", &format!("=== {text} ===")));
}

pub fn step(text: &str) {
    println!("{} {}", paint("1;34", "▶"), paint("1", text));
}

pub fn ok(text: &str) {
    println!("  {} {}", paint("1;32", "✓"), text);
}

pub fn warn(text: &str) {
    println!("  {} {}", paint("1;33", "!"), text);
}

pub fn fail(text: &str) {
    println!("  {} {}", paint("1;31", "✗"), text);
}

pub fn detail(text: &str) {
    println!("    {}", paint("2", text));
}

pub fn hint(text: &str) {
    println!("\n{} {}", paint("1;35", "→"), text);
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
    println!("  {dot} {label:<16} {state:<6} {}", paint("2", note));
}
