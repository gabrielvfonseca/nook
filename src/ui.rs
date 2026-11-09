//! Terminal output helpers: colors, status lines, aligned tables, human units.
//!
//! Color is on only when the stream is a TTY and `NO_COLOR` is unset
//! (https://no-color.org). Machine output (`--json`, the `new` path on stdout)
//! never contains escape codes.

use std::io::IsTerminal;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";

fn color_for(is_tty: bool) -> bool {
    is_tty && std::env::var_os("NO_COLOR").is_none()
}

/// Color for stdout-bound output (tables, reports).
pub fn stdout_color() -> bool {
    color_for(std::io::stdout().is_terminal())
}

/// Color for stderr-bound output (status lines, errors).
pub fn stderr_color() -> bool {
    color_for(std::io::stderr().is_terminal())
}

fn paint(s: &str, code: &str, on: bool) -> String {
    if on {
        format!("{code}{s}{RESET}")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str, on: bool) -> String {
    paint(s, BOLD, on)
}

pub fn dim(s: &str, on: bool) -> String {
    paint(s, DIM, on)
}

pub fn cyan(s: &str, on: bool) -> String {
    paint(s, CYAN, on)
}

pub fn green(s: &str, on: bool) -> String {
    paint(s, GREEN, on)
}

pub fn yellow(s: &str, on: bool) -> String {
    paint(s, YELLOW, on)
}

pub fn red(s: &str, on: bool) -> String {
    paint(s, RED, on)
}

/// Status line on stderr: `✓ msg`, `! msg`, `✗ msg`, or `· msg`.
pub fn status(symbol: Status, msg: &str) {
    let on = stderr_color();
    let mark = match symbol {
        Status::Ok => green("✓", on),
        Status::Warn => yellow("!", on),
        Status::Err => red("✗", on),
        Status::Info => dim("·", on),
    };
    eprintln!("{mark} {msg}");
}

#[derive(Clone, Copy)]
pub enum Status {
    Ok,
    Warn,
    Err,
    Info,
}

/// Indented detail line under a status line, dimmed.
pub fn detail(msg: &str) {
    eprintln!("  {}", dim(msg, stderr_color()));
}

/// Render an aligned table. Widths are measured on the plain text, so the
/// same call works with and without color (pass cells already painted).
pub fn table(headers: &[&str], rows: &[Vec<String>], color: bool) -> String {
    let plain_len = |s: &str| strip_ansi(s).chars().count();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(plain_len(cell));
        }
    }
    let mut out = String::new();
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let padded = format!("{:<w$}", h, w = widths[i]);
            bold(&padded, color)
        })
        .collect();
    out.push_str(header_line.join("  ").trim_end());
    out.push('\n');
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let pad = widths[i].saturating_sub(plain_len(cell));
                format!("{cell}{}", " ".repeat(pad))
            })
            .collect();
        out.push_str(cells.join("  ").trim_end());
        out.push('\n');
    }
    out
}

/// Remove ANSI SGR sequences (only the ones this module emits).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for n in chars.by_ref() {
                if n == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Quote and escape a string for JSON output.
pub fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `1.5 GB`, `320 MB`, `12 KB`, `0 B`. Decimal units, matching `df -H`.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// `45s`, `12m`, `3h`, `2d` (largest unit that fits, no decimals).
pub fn human_duration(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// A 20-cell usage bar, e.g. `██████░░░░░░░░░░░░░░`.
pub fn usage_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64)
        .round()
        .clamp(0.0, width as f64) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1_500), "1.5 KB");
        assert_eq!(human_bytes(2_000_000_000), "2.0 GB");
    }

    #[test]
    fn durations() {
        assert_eq!(human_duration(-5), "0s");
        assert_eq!(human_duration(59), "59s");
        assert_eq!(human_duration(600), "10m");
        assert_eq!(human_duration(7200), "2h");
        assert_eq!(human_duration(172800), "2d");
    }

    #[test]
    fn json_escaping() {
        assert_eq!(json_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
        assert_eq!(json_string("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn bar_bounds() {
        assert_eq!(usage_bar(0.0, 4), "░░░░");
        assert_eq!(usage_bar(50.0, 4), "██░░");
        assert_eq!(usage_bar(250.0, 4), "████");
    }

    #[test]
    fn table_aligns_colored_cells() {
        let rows = vec![vec![green("ok", true), "x".to_string()]];
        let t = table(&["STATUS", "NAME"], &rows, false);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "STATUS  NAME");
        assert_eq!(strip_ansi(lines[1]), "ok      x");
        assert_eq!(strip_ansi(&green("ok", true)), "ok");
    }
}
