//! Conservative source-line classification shared by previews and rule evidence.

use super::security::may_expose_credential;
use std::path::Path;

#[derive(Clone, Copy)]
enum MultilineCredential {
    Quoted(&'static str),
    YamlBlock { key_indent: usize },
    Pending { key_indent: usize },
    Parenthesized(ParenthesizedCredential),
    GrammarParenthesized { end_line: u32 },
}

#[derive(Clone, Copy)]
struct SourceSyntax {
    hash_comment: bool,
    slash_comment: bool,
    rust: bool,
    javascript: bool,
}

impl SourceSyntax {
    fn for_language(language: &str) -> Self {
        let hash_comment = matches!(
            language,
            "Python" | "Ruby" | "PHP" | "Shell" | "Bash" | "YAML" | "TOML"
        );
        Self {
            hash_comment,
            slash_comment: !hash_comment || language == "PHP",
            rust: language == "Rust",
            javascript: matches!(
                language,
                "JavaScript" | "TypeScript" | "JSX" | "TSX" | "Vue" | "Svelte" | "HTML"
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum ParenthesizedDelimiter {
    Quoted(&'static str),
    RustRaw(usize),
    Regex { character_class: bool },
}

#[derive(Clone, Copy)]
struct ParenthesizedCredential {
    depth: usize,
    delimiter: Option<ParenthesizedDelimiter>,
    block_comment_depth: usize,
    expects_operand: bool,
}

impl Default for ParenthesizedCredential {
    fn default() -> Self {
        Self {
            depth: 0,
            delimiter: None,
            block_comment_depth: 0,
            expects_operand: true,
        }
    }
}

impl ParenthesizedCredential {
    fn next(mut self, line: &str, syntax: SourceSyntax) -> Option<MultilineCredential> {
        let bytes = line.as_bytes();
        let mut offset = 0;
        while offset < bytes.len() {
            let remaining = &bytes[offset..];
            if self.block_comment_depth > 0 {
                if remaining.starts_with(b"*/") {
                    self.block_comment_depth -= 1;
                    offset += 2;
                } else if syntax.rust && remaining.starts_with(b"/*") {
                    self.block_comment_depth += 1;
                    offset += 2;
                } else {
                    offset += 1;
                }
                continue;
            }
            if let Some(delimiter) = self.delimiter {
                match delimiter {
                    ParenthesizedDelimiter::Quoted(delimiter) => {
                        if bytes[offset] == b'\\' {
                            offset += 2;
                        } else if remaining.starts_with(delimiter.as_bytes()) {
                            self.delimiter = None;
                            offset += delimiter.len();
                        } else {
                            offset += 1;
                        }
                    }
                    ParenthesizedDelimiter::RustRaw(hashes) => {
                        if remaining.starts_with(b"\"")
                            && remaining[1..]
                                .iter()
                                .take(hashes)
                                .filter(|byte| **byte == b'#')
                                .count()
                                == hashes
                            && remaining.len() > hashes
                        {
                            self.delimiter = None;
                            offset += hashes + 1;
                        } else {
                            offset += 1;
                        }
                    }
                    ParenthesizedDelimiter::Regex {
                        mut character_class,
                    } => {
                        if bytes[offset] == b'\\' {
                            offset += 2;
                            continue;
                        }
                        match bytes[offset] {
                            b'[' => character_class = true,
                            b']' => character_class = false,
                            b'/' if !character_class => self.delimiter = None,
                            _ => {}
                        }
                        if self.delimiter.is_some() {
                            self.delimiter =
                                Some(ParenthesizedDelimiter::Regex { character_class });
                        }
                        offset += 1;
                    }
                }
                continue;
            }
            if (syntax.hash_comment && remaining.starts_with(b"#"))
                || (syntax.slash_comment && remaining.starts_with(b"//"))
            {
                break;
            }
            if syntax.slash_comment && remaining.starts_with(b"/*") {
                self.block_comment_depth = 1;
                offset += 2;
                continue;
            }
            if syntax.rust {
                let raw = remaining
                    .strip_prefix(b"br")
                    .or_else(|| remaining.strip_prefix(b"cr"))
                    .or_else(|| remaining.strip_prefix(b"r"));
                if let Some(raw) = raw {
                    let hashes = raw.iter().take_while(|byte| **byte == b'#').count();
                    if raw.get(hashes) == Some(&b'"') {
                        self.delimiter = Some(ParenthesizedDelimiter::RustRaw(hashes));
                        self.expects_operand = false;
                        offset += remaining.len() - raw.len() + hashes + 1;
                        continue;
                    }
                }
            }
            if syntax.javascript && bytes[offset] == b'/' && self.expects_operand {
                self.delimiter = Some(ParenthesizedDelimiter::Regex {
                    character_class: false,
                });
                self.expects_operand = false;
                offset += 1;
                continue;
            }
            if let Some(delimiter) = ["\"\"\"", "'''", "\"", "'", "`"]
                .into_iter()
                .find(|delimiter| remaining.starts_with(delimiter.as_bytes()))
            {
                self.delimiter = Some(ParenthesizedDelimiter::Quoted(delimiter));
                self.expects_operand = false;
                offset += delimiter.len();
                continue;
            }
            match bytes[offset] {
                b'(' => {
                    self.depth += 1;
                    self.expects_operand = true;
                }
                b')' => {
                    self.depth = self.depth.saturating_sub(1);
                    if self.depth == 0 {
                        return None;
                    }
                    self.expects_operand = false;
                }
                b'=' | b':' | b',' | b'?' | b'!' | b'~' | b'+' | b'-' | b'*' | b'/' | b'%'
                | b'&' | b'|' | b'^' | b'<' | b'>' | b'[' | b'{' => self.expects_operand = true,
                byte if !byte.is_ascii_whitespace() => self.expects_operand = false,
                _ => {}
            }
            offset += 1;
        }
        (self.depth > 0).then_some(MultilineCredential::Parenthesized(self))
    }
}

pub(crate) fn sensitive_source_lines<'a>(
    text: &'a str,
    language: &str,
) -> impl Iterator<Item = (&'a str, bool)> {
    let syntax = SourceSyntax::for_language(language);
    let grammar_ends =
        crate::parse::tree_sitter::parenthesized_ends(language, text).unwrap_or_default();
    let mut inside_private_key = false;
    let mut multiline_credential = None;
    let mut line_start = 0usize;
    text.lines().enumerate().map(move |(line_index, line)| {
        let line_no = line_index as u32 + 1;
        if line.contains("-----BEGIN ") && line.contains("PRIVATE KEY-----") {
            inside_private_key = true;
        }
        let credential_line = may_expose_credential(line);
        let mut multiline_continuation = false;
        if let Some(state) = multiline_credential {
            match state {
                MultilineCredential::GrammarParenthesized { end_line } => {
                    multiline_continuation = line_no <= end_line;
                    if line_no >= end_line {
                        multiline_credential = None;
                    }
                }
                MultilineCredential::Parenthesized(expression) => {
                    multiline_continuation = true;
                    multiline_credential = expression.next(line, syntax);
                }
                MultilineCredential::Pending { key_indent } => {
                    multiline_continuation = true;
                    let value = line.trim_start();
                    if !value.is_empty() && !value.starts_with('#') {
                        let value_start = line_start + line.len() - value.len();
                        multiline_credential = multiline_value_start_with_grammar(
                            value,
                            key_indent,
                            syntax,
                            value_start,
                            &grammar_ends,
                        );
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
            multiline_credential = multiline_credential_start(
                line,
                credential_line,
                syntax,
                line_start,
                &grammar_ends,
            );
        }
        if line.contains("-----END ") && line.contains("PRIVATE KEY-----") {
            inside_private_key = false;
        }
        let after_line = line_start + line.len();
        let newline_len = if text.as_bytes().get(after_line) == Some(&b'\r') {
            if text.as_bytes().get(after_line + 1) == Some(&b'\n') {
                2
            } else {
                1
            }
        } else if text.as_bytes().get(after_line) == Some(&b'\n') {
            1
        } else {
            0
        };
        line_start = after_line + newline_len;
        (line, sensitive)
    })
}

fn multiline_credential_start(
    line: &str,
    credential_line: bool,
    syntax: SourceSyntax,
    line_start: usize,
    grammar_ends: &std::collections::HashMap<usize, u32>,
) -> Option<MultilineCredential> {
    if !credential_line {
        return None;
    }
    let key_indent = leading_indent(line);
    let start_after =
        |assignment: usize, prefix_len: usize, raw_value: &str, allow_pending: bool| {
            let leading = raw_value.len() - raw_value.trim_start().len();
            let value = raw_value.trim_start();
            if value.is_empty() && allow_pending {
                return Some(MultilineCredential::Pending { key_indent });
            }
            let value_start = line_start + assignment + prefix_len + leading;
            multiline_value_start_with_grammar(value, key_indent, syntax, value_start, grammar_ends)
        };

    // Prefer an initializer equals sign so typed declarations do not stop at
    // the annotation colon (for example, `api_key: string = ...`).
    if let Some(result) = line.match_indices('=').find_map(|(assignment, _)| {
        let raw = &line[assignment..];
        start_after(assignment, 1, &raw[1..], true)
    }) {
        return Some(result);
    }
    line.match_indices(':').find_map(|(assignment, _)| {
        let raw = &line[assignment..];
        let (raw_value, prefix_len) = raw
            .strip_prefix(":=")
            .map_or((&raw[1..], 1), |value| (value, 2));
        start_after(assignment, prefix_len, raw_value, false)
    })
}

fn multiline_value_start_with_grammar(
    value: &str,
    key_indent: usize,
    syntax: SourceSyntax,
    value_start: usize,
    grammar_ends: &std::collections::HashMap<usize, u32>,
) -> Option<MultilineCredential> {
    if value.starts_with('(')
        && let Some(&end_line) = grammar_ends.get(&value_start)
    {
        return Some(MultilineCredential::GrammarParenthesized { end_line });
    }
    multiline_value_start(value, key_indent, syntax)
}

fn multiline_value_start(
    value: &str,
    key_indent: usize,
    syntax: SourceSyntax,
) -> Option<MultilineCredential> {
    if value.starts_with('(') {
        return ParenthesizedCredential::default().next(value, syntax);
    }
    if value == "\\" {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_boundaries_handle_division_lifetimes_regex_and_nested_templates() {
        for (language, source, sensitive) in [
            (
                "JavaScript",
                "const password = (counter++ / 2);\nconst ordinary = 1;\n",
                vec![true, false],
            ),
            (
                "JavaScript",
                "const password = (typeof /[)]/ === 'object' ?\n \"fixture-only-value\" : \"fallback\"\n);\nconst ordinary = 1;\n",
                vec![true, true, true, false],
            ),
            (
                "Rust",
                "fn f() {\nlet password = (lookup::<&'static str>());\nlet ordinary = 1;\n}\n",
                vec![false, true, false, false],
            ),
            (
                "JavaScript",
                "const password = (`${`inner)text`}` +\n \"fixture-only-value\"\n);\nconst ordinary = 1;\n",
                vec![true, true, true, false],
            ),
        ] {
            let actual: Vec<_> = sensitive_source_lines(source, language)
                .map(|(_, hidden)| hidden)
                .collect();
            assert_eq!(actual, sensitive, "{language}: {source}");
        }
    }

    #[test]
    fn pending_parentheses_use_grammar_spans_for_f_strings() {
        let marker = "fixture-only-value".repeat(10);
        let mut source = String::from(
            r##"password = \
    (
    f"{")"}"
    "##,
        );
        source.push('"');
        source.push_str(&marker);
        source.push_str("\"\n)\nordinary = 1\n");
        for source in [source.clone(), source.replace('\n', "\r\n")] {
            let actual: Vec<_> = sensitive_source_lines(&source, "Python")
                .map(|(_, hidden)| hidden)
                .collect();
            assert_eq!(actual, [true, true, true, true, true, false]);
        }
    }

    #[test]
    fn typed_and_next_line_credential_values_stay_redacted() {
        let typed = "const api_key: string = `\nfixture-only-value\n`\nordinary = 1\n";
        let next_line = "const api_key =\n  `fixture-only-value`\nordinary = 1\n";
        for (language, source, expected) in [
            ("TypeScript", typed, [true, true, true, false].as_slice()),
            ("JavaScript", next_line, [true, true, false].as_slice()),
        ] {
            let actual: Vec<_> = sensitive_source_lines(source, language)
                .map(|(_, hidden)| hidden)
                .collect();
            assert_eq!(actual, expected, "{language}: {source}");
        }
    }

    #[test]
    fn grammar_parentheses_keep_crlf_and_composite_offsets_aligned() {
        let source = concat!(
            "const before = 0;\r\n",
            "const password = (typeof /[)]/ === 'object' ?\r\n",
            " \"fixture-only-valuefixture-only-value\" : \"fallback\"\r\n",
            ");\r\n",
            "const ordinary = 1;\r\n",
        );
        let actual: Vec<_> = sensitive_source_lines(source, "JavaScript")
            .map(|(_, hidden)| hidden)
            .collect();
        assert_eq!(actual, [false, true, true, true, false]);

        let composite = concat!(
            "<template><p>fixture-only-value</p></template>\r\n",
            "<script>\r\n",
            "const password = (typeof /[)]/ === 'object' ?\r\n",
            " \"fixture-only-valuefixture-only-value\" : \"fallback\"\r\n",
            ");\r\n",
            "const ordinary = 1;\r\n",
            "</script>\r\n",
        );
        let actual: Vec<_> = sensitive_source_lines(composite, "Vue")
            .map(|(_, hidden)| hidden)
            .collect();
        assert_eq!(actual, [false, false, true, true, true, false, false]);
    }

    #[test]
    fn parenthesized_operators_and_raw_strings_do_not_hide_following_code() {
        for (language, expression) in [
            ("Python", "(1024 // 2)"),
            ("Python", "(1024 // (2 + 1))"),
            ("Rust", "(r#\"abc\"#);"),
            ("Rust", "(br##\"abc ) \\\"\"##);"),
            ("Rust", "(/* outer /* nested ) */ comment */ 12);"),
        ] {
            let source = format!("password = {expression}\nordinary = 123\n");
            let lines: Vec<_> = sensitive_source_lines(&source, language).collect();
            assert!(lines[0].1);
            assert!(!lines[1].1, "ordinary code was hidden: {lines:?}");
        }
    }

    #[test]
    fn parenthesized_credentials_cover_concatenated_literals_until_the_real_close() {
        for (language, expression) in [
            ("Python", "(\n    \"part-one\"\n    \"synthetic-secret\"\n)"),
            (
                "Python",
                "(\n    \"part-one )\" + # ignored )\n    (\"synthetic-secret\")\n)",
            ),
            (
                "JavaScript",
                "(\n    \"part-one\" + /* ignored )\n       still a comment ) */\n    \"synthetic-secret\"\n)",
            ),
            (
                "Python",
                "(\n    r\"\"\"part-one )\nsynthetic-secret\n\"\"\"\n)",
            ),
            ("Rust", "(r##\"part-one )\nsynthetic-secret\n\"##\n)"),
            (
                "JavaScript",
                "(/[)]/.test(\"part-one\"),\n    \"synthetic-secret\"\n)",
            ),
            (
                "JavaScript",
                "(/\\/\\)/.test(\"part-one\"), // ignored )\n    \"synthetic-secret\"\n)",
            ),
        ] {
            let source = format!("password = {expression}\nordinary = 123\n");
            let lines: Vec<_> = sensitive_source_lines(&source, language).collect();
            assert!(
                !lines.last().unwrap().1,
                "normal code after the close must remain visible"
            );
            assert!(
                lines[..lines.len() - 1]
                    .iter()
                    .all(|(_, sensitive)| *sensitive),
                "credential continuation exposed: {lines:?}"
            );
        }
    }
}
