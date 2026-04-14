use std::path::Path;

use eagraph_core::Symbol;

/// Read source lines for a symbol from disk.
/// Returns the symbol's lines plus `context` extra lines above and below.
/// Returns an empty string if the file can't be read.
pub fn read_snippet(repo_root: &Path, symbol: &Symbol, context: u32) -> String {
    let full_path = repo_root.join(&symbol.file_path);
    let source = match std::fs::read_to_string(&full_path) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };

    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let start = symbol.line_start.saturating_sub(1).saturating_sub(context) as usize;
    let end = (symbol.line_end + context) as usize;
    let end = end.min(lines.len());

    if start >= lines.len() {
        return String::new();
    }

    lines[start..end].join("\n")
}
