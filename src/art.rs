//! Loading and normalising the ASCII art.

/// Shipped default art, compiled into the binary.
pub const DEFAULT_ART: &str = include_str!("../assets/screensaver.txt");

/// Load the art from `path` (if set and readable), else the built-in default.
/// Line endings are normalised to `\n`, tabs expanded to 4 spaces (ttfx's
/// default tab width), and leading/trailing blank lines dropped.
pub fn load(path: &str) -> String {
    let raw = if path.trim().is_empty() {
        None
    } else {
        std::fs::read(path.trim())
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    };
    let raw = match raw {
        Some(s) if !s.trim().is_empty() => s,
        _ => DEFAULT_ART.to_string(),
    };
    normalise(&raw)
}

fn normalise(s: &str) -> String {
    let s = s.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = s
        .split('\n')
        .map(|l| l.replace('\t', "    ").trim_end().to_string())
        .collect();
    let first = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
    let last = lines.iter().rposition(|l| !l.is_empty()).unwrap_or(0);
    lines[first..=last].join("\n")
}

/// (columns, rows) of the art.
pub fn size(art: &str) -> (usize, usize) {
    let cols = art.lines().map(|l| l.chars().count()).max().unwrap_or(1).max(1);
    let rows = art.lines().count().max(1);
    (cols, rows)
}
