//! Shared extraction helpers used by grammar-backed and lexical parsing.

/// Byte-offset to 1-based line/column lookup over a UTF-8 buffer.
#[derive(Debug, Clone)]
pub struct LineIndex {
    starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self {
            starts,
            len: text.len(),
        }
    }

    /// Returns the 1-based (line, byte column) for a byte offset.
    /// Offsets beyond the buffer clamp to the last position.
    pub fn line_col(&self, byte: usize) -> (u32, u32) {
        let byte = byte.min(self.len);
        let idx = self.starts.partition_point(|&s| s <= byte).max(1);
        let line = idx as u32;
        let col = (byte - self.starts[idx - 1]) as u32 + 1;
        (line, col)
    }

    pub fn line_count(&self) -> u32 {
        self.starts.len() as u32
    }
}

/// Classifies a comment body as `line`, `block`, or `doc`.
pub fn comment_kind(text: &str) -> &'static str {
    let t = text.trim_start();
    if t.starts_with("/**") || (t.starts_with("///") && !t.starts_with("////")) {
        "doc"
    } else if t.starts_with("//") || t.starts_with('#') || t.starts_with("--") {
        "line"
    } else {
        "block"
    }
}

/// Strips one layer of quotes from a string-literal node body.
pub fn unquote(raw: &str) -> String {
    let t = raw.trim();
    let bytes = t.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
            || (bytes[0] == b'`' && bytes[bytes.len() - 1] == b'`'))
    {
        t[1..t.len() - 1].to_owned()
    } else {
        t.to_owned()
    }
}

/// Truncates long extracted text for evidence fields.
pub fn clamp_text(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_owned()
    } else {
        let mut cut: String = trimmed.chars().take(max_chars).collect();
        cut.push('…');
        cut
    }
}

/// Lexical comment markers for languages without a grammar.
pub struct LexicalMarkers {
    pub line: &'static [&'static str],
    pub block: &'static [(&'static str, &'static str)],
}

pub fn lexical_markers(language: &str) -> LexicalMarkers {
    match language {
        "Shell" | "PowerShell" | "YAML" | "Dockerfile" => LexicalMarkers {
            line: &["#"],
            block: &[],
        },
        "CSS" | "SCSS" => LexicalMarkers {
            line: &["//"],
            block: &[("/*", "*/")],
        },
        "SQL" => LexicalMarkers {
            line: &["--"],
            block: &[("/*", "*/")],
        },
        "XML" | "HTML" => LexicalMarkers {
            line: &[],
            block: &[("<!--", "-->")],
        },
        _ => LexicalMarkers {
            line: &["//", "#"],
            block: &[("/*", "*/")],
        },
    }
}

/// Lexically scans `text` and returns (comment line count, comment spans as
/// (start_byte, end_byte, kind)).
pub fn lexical_comment_scan(
    text: &str,
    language: &str,
) -> (u32, Vec<(usize, usize, &'static str)>) {
    let markers = lexical_markers(language);
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0usize;
    let mut comment_line_ranges: Vec<(usize, usize)> = Vec::new();
    while i < bytes.len() {
        if !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let rest = &text[i..];
        let mut matched = false;
        for (open, close) in markers.block {
            if rest.starts_with(open) {
                let end = rest[open.len()..]
                    .find(close)
                    .map(|p| i + open.len() + p + close.len())
                    .unwrap_or(bytes.len());
                spans.push((i, end, "block"));
                comment_line_ranges.push((i, end));
                i = end;
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }
        for marker in markers.line {
            if rest.starts_with(marker) {
                let end = rest.find('\n').map(|p| i + p).unwrap_or(bytes.len());
                spans.push((i, end, "line"));
                comment_line_ranges.push((i, end));
                i = end;
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }
        // Skip string literals so comment markers inside strings are ignored.
        let b = bytes[i];
        if b == b'"' || b == b'\'' || b == b'`' {
            let quote = b;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == quote || bytes[j] == b'\n' {
                    j += 1;
                    break;
                }
                j += 1;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    let index = LineIndex::new(text);
    let mut lines: Vec<u32> = Vec::new();
    for (start, end) in comment_line_ranges {
        let (sline, _) = index.line_col(start);
        let (eline, _) = index.line_col(end.saturating_sub(1).max(start));
        for l in sline..=eline {
            lines.push(l);
        }
    }
    lines.sort_unstable();
    lines.dedup();
    (lines.len() as u32, spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_maps_byte_offsets_to_one_based_positions() {
        let index = LineIndex::new("ab\ncd\n");
        assert_eq!(index.line_col(0), (1, 1));
        assert_eq!(index.line_col(3), (2, 1));
        assert_eq!(index.line_col(5), (2, 3));
        assert_eq!(index.line_count(), 3);
    }

    #[test]
    fn line_index_keeps_columns_on_a_final_line_without_newline() {
        let index = LineIndex::new("ab\ncd");
        assert_eq!(index.line_col(3), (2, 1));
        assert_eq!(index.line_col(4), (2, 2));
        assert_eq!(index.line_col(5), (2, 3));
        // Past the end of the buffer clamps to the last position, not the line start.
        assert_eq!(index.line_col(99), (2, 3));
    }

    #[test]
    fn line_index_handles_single_line_utf8_and_empty_text() {
        let index = LineIndex::new("中文abc");
        assert_eq!(index.line_col(6), (1, 7));
        assert_eq!(index.line_col(usize::MAX), (1, 10));
        assert_eq!(LineIndex::new("").line_col(usize::MAX), (1, 1));
    }

    #[test]
    fn comment_kind_distinguishes_doc_line_block() {
        assert_eq!(comment_kind("/** api */"), "doc");
        assert_eq!(comment_kind("// note"), "line");
        assert_eq!(comment_kind("# note"), "line");
        assert_eq!(comment_kind("/* span */"), "block");
    }

    #[test]
    fn lexical_scan_ignores_comments_inside_strings() {
        let (lines, spans) = lexical_comment_scan("x = \"# not\"\n# yes\n", "Shell");
        assert_eq!(lines, 1);
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn unquote_strips_matching_quotes() {
        assert_eq!(unquote("'./store'"), "./store");
        assert_eq!(unquote("\"a\""), "a");
        assert_eq!(unquote("plain"), "plain");
    }
}
