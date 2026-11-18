//! Line-level normalization for clone candidates: identifiers and literal
//! values collapse to placeholders while control-flow tokens stay intact, so
//! renamed-but-equivalent logic still matches.

/// Normalizes one source line into a clone-comparable token string. Returns an
/// empty string for blank lines.
pub fn normalize_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            // consume the identifier
            let mut ident = String::new();
            ident.push(c);
            while let Some(&n) = chars.peek() {
                if n.is_alphanumeric() || n == '_' {
                    ident.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            if is_keyword(&ident) {
                out.push_str(&ident);
            } else if looks_like_constant(&ident) {
                out.push_str("<CONST>");
            } else {
                out.push_str("<id>");
            }
            continue;
        }
        if c.is_ascii_digit() {
            while let Some(&n) = chars.peek() {
                if n.is_ascii_digit() || n == '.' {
                    chars.next();
                } else {
                    break;
                }
            }
            out.push_str("<num>");
            continue;
        }
        if c == '"' || c == '\'' || c == '`' {
            // consume the string literal (no escape handling needed for
            // normalization purposes; newlines end the scan)
            let quote = c;
            while let Some(&n) = chars.peek() {
                chars.next();
                if n == quote || n == '\n' {
                    break;
                }
            }
            out.push_str("<str>");
            continue;
        }
        out.push(c);
    }
    out
}

const KEYWORDS: &[&str] = &[
    "if",
    "else",
    "elif",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "return",
    "try",
    "catch",
    "except",
    "finally",
    "throw",
    "throws",
    "match",
    "fn",
    "func",
    "function",
    "def",
    "class",
    "struct",
    "interface",
    "enum",
    "impl",
    "trait",
    "public",
    "private",
    "protected",
    "static",
    "const",
    "let",
    "var",
    "new",
    "await",
    "async",
    "yield",
    "import",
    "from",
    "export",
    "package",
    "use",
    "with",
    "as",
    "in",
    "is",
    "not",
    "and",
    "or",
    "nil",
    "null",
    "true",
    "false",
    "self",
    "this",
    "super",
    "print",
    "echo",
    "when",
];

fn is_keyword(ident: &str) -> bool {
    KEYWORDS.contains(&ident)
}

/// UPPER_SNAKE names stay distinct — they usually carry real semantic weight.
fn looks_like_constant(ident: &str) -> bool {
    ident
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        && ident.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_normalize_to_the_same_string() {
        let a = normalize_line("if (userCount > maxLimit) { total += userCount; }");
        let b = normalize_line("if (cnt > limit) { sum += cnt; }");
        assert_eq!(a, b);
    }

    #[test]
    fn literals_collapse_but_keywords_survive() {
        assert_eq!(normalize_line("return \"hello world\";"), "return<str>;");
        assert_eq!(normalize_line("if x == 42:"), "if<id>==<num>:");
        assert_eq!(normalize_line("for item in items:"), "for<id>in<id>:");
    }

    #[test]
    fn constants_stay_distinct() {
        assert_eq!(normalize_line("MAX_RETRY = 5"), "<CONST>=<num>");
    }
}
