/// Cleans raw terminal output by stripping ANSI escape sequences,
/// discarding carriage-return progress-bar lines, trimming empty lines,
/// and retaining only the trailing `tail_limit` lines (default 8~12 lines).
pub fn sanitize_stderr(raw: &[u8], tail_limit: usize) -> Vec<String> {
    let mut resolved_bytes = Vec::with_capacity(raw.len());
    let mut line_start = 0;

    for &b in raw {
        if b == b'\r' {
            resolved_bytes.truncate(line_start);
        } else if b == b'\n' {
            resolved_bytes.push(b'\n');
            line_start = resolved_bytes.len();
        } else {
            resolved_bytes.push(b);
        }
    }

    let stripped = strip_ansi_escapes::strip(&resolved_bytes);
    let text = String::from_utf8_lossy(&stripped);

    let mut clean_lines: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if !trimmed.is_empty() && !is_noise_line(trimmed) {
            clean_lines.push(trimmed.to_string());
        }
    }

    if clean_lines.len() > tail_limit {
        let start = clean_lines.len() - tail_limit;
        clean_lines.drain(..start);
    }

    clean_lines
}

fn is_noise_line(line: &str) -> bool {
    // Ignore pure spinner or progress percentage updates (e.g. "[==>    ] 25%")
    let is_pure_progress = line.ends_with('%') && line.chars().any(|c| c == '[' || c == ']');
    is_pure_progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_with_ansi_and_tail() {
        let raw = b"\x1b[31mError 1\x1b[0m\nLine 2\n\nLine 3\n\x1b[32mLine 4\x1b[0m\nLine 5\n";
        let lines = sanitize_stderr(raw, 3);
        assert_eq!(lines, vec!["Line 3", "Line 4", "Line 5"]);
    }

    #[test]
    fn test_carriage_return_progress_filtering() {
        let raw = b"downloading 10%\rdownloading 50%\rdownloading 100%\nActual error message\n";
        let lines = sanitize_stderr(raw, 5);
        assert_eq!(lines, vec!["downloading 100%", "Actual error message"]);
    }
}
