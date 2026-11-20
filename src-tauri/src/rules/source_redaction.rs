//! Conservative source-line classification shared by previews and rule evidence.

use super::security::may_expose_credential;
use std::path::Path;

#[derive(Clone, Copy)]
enum MultilineCredential {
    Quoted(&'static str),
    YamlBlock { key_indent: usize },
    Pending { key_indent: usize },
}

pub(crate) fn sensitive_source_lines(text: &str) -> impl Iterator<Item = (&str, bool)> {
    let mut inside_private_key = false;
    let mut multiline_credential = None;
    text.lines().map(move |line| {
        if line.contains("-----BEGIN ") && line.contains("PRIVATE KEY-----") {
            inside_private_key = true;
        }
        let credential_line = may_expose_credential(line);
        let mut multiline_continuation = false;
        if let Some(state) = multiline_credential {
            match state {
                MultilineCredential::Pending { key_indent } => {
                    multiline_continuation = true;
                    let value = line.trim_start();
                    if !value.is_empty() && !value.starts_with('#') {
                        multiline_credential = multiline_value_start(value, key_indent);
                    }
                }
                MultilineCredential::Quoted(delimiter) => {
                    multiline_continuation = true;
                    if contains_unescaped_delimiter(line, delimiter) {
                        multiline_credential = None;
                    }
                }
                MultilineCredential::YamlBlock { key_indent } => {
                    if line.trim().is_empty() {
                        // Blank lines do not expose a value, but remain inside the block.
                    } else if leading_indent(line) > key_indent {
                        multiline_continuation = true;
                    } else {
                        multiline_credential = None;
                    }
                }
            }
        }
        let sensitive = inside_private_key || multiline_continuation || credential_line;
        if multiline_credential.is_none() {
            multiline_credential = multiline_credential_start(line, credential_line);
        }
        if line.contains("-----END ") && line.contains("PRIVATE KEY-----") {
            inside_private_key = false;
        }
        (line, sensitive)
    })
}

fn multiline_credential_start(line: &str, credential_line: bool) -> Option<MultilineCredential> {
    if !credential_line {
        return None;
    }
    // Type annotations may contain separators before the initializer. Keep YAML
    // block values ahead of any equals signs in their trailing comments.
    line.match_indices(['=', ':']).find_map(|(assignment, _)| {
        let value = &line[assignment..];
        let value = value.strip_prefix(":=").unwrap_or(&value[1..]).trim_start();
        multiline_value_start(value, leading_indent(line))
    })
}

fn multiline_value_start(value: &str, key_indent: usize) -> Option<MultilineCredential> {
    let parenthesized = value.starts_with('(');
    let value =
        value.trim_start_matches(|character: char| character == '(' || character.is_whitespace());
    if (parenthesized && (value.is_empty() || value.starts_with('#'))) || value == "\\" {
        return Some(MultilineCredential::Pending { key_indent });
    }
    let python_value = value.trim_start_matches(['r', 'R', 'u', 'U', 'b', 'B', 'f', 'F', 't', 'T']);
    for (delimiter, state) in [
        ("\"\"\"", MultilineCredential::Quoted("\"\"\"")),
        ("'''", MultilineCredential::Quoted("'''")),
        ("`", MultilineCredential::Quoted("`")),
    ] {
        let literal = if delimiter == "`" {
            value
        } else {
            python_value
        };
        if let Some(after_open) = literal.strip_prefix(delimiter)
            && !contains_unescaped_delimiter(after_open, delimiter)
        {
            return Some(state);
        }
    }
    if value.starts_with(['|', '>']) {
        return Some(MultilineCredential::YamlBlock { key_indent });
    }
    None
}

fn contains_unescaped_delimiter(line: &str, delimiter: &str) -> bool {
    line.match_indices(delimiter).any(|(index, _)| {
        let backslashes = line[..index]
            .chars()
            .rev()
            .take_while(|character| *character == '\\')
            .count();
        backslashes % 2 == 0
    })
}

fn leading_indent(line: &str) -> usize {
    line.chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .count()
}

pub(crate) fn is_credential_path(relative: &str) -> bool {
    let name = relative
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || matches!(
            name.as_str(),
            ".npmrc" | ".netrc" | "credentials" | "id_rsa" | "id_ed25519"
        )
        || Path::new(&name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "pem" | "key" | "p12" | "pfx"))
}
