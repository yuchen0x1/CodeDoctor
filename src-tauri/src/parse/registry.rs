//! Parser dispatch: grammar-backed languages via the shared tree-sitter
//! engine, composite files via script partitioning, everything else via an
//! honestly-labeled lexical fallback that never claims parser metrics.

use super::languages::common::{LineIndex, clamp_text, lexical_comment_scan};
use super::languages::{is_composite, spec_for};
use super::tree_sitter;
use super::{FileRecord, ParseDiagnostic, ParseResult};
use crate::domain::types::CoverageKind;
use crate::parse::{CommentInfo, SourceRange};

#[derive(Default)]
pub struct ParserRegistry;

impl ParserRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(file: &FileRecord, text: &str) -> ParseResult {
        let language = file.language.as_str();
        if is_composite(language) {
            return tree_sitter::parse_composite(text);
        }
        if let Some(spec) = spec_for(language) {
            return tree_sitter::parse_with_spec(spec, text);
        }
        lexical_parse(language, text)
    }
}

/// Lexical fallback: counts lines and comments only, and records an explicit
/// coverage diagnostic so no metric is silently reported as parsed.
fn lexical_parse(language: &str, text: &str) -> ParseResult {
    let index = LineIndex::new(text);
    let (comment_lines, spans) = lexical_comment_scan(text, language);
    let non_blank = text.lines().filter(|l| !l.trim().is_empty()).count() as u32;
    let comments = spans
        .into_iter()
        .map(|(start, end, kind)| {
            let (start_line, start_column) = index.line_col(start);
            let (end_line, end_column) = index.line_col(end);
            let raw = &text[start..end.min(text.len()).max(start)];
            CommentInfo {
                text: clamp_text(raw, 500),
                range: SourceRange {
                    start_line,
                    start_column,
                    end_line,
                    end_column,
                    start_byte: start,
                    end_byte: end,
                },
                kind: kind.to_owned(),
            }
        })
        .collect();
    ParseResult {
        parser_name: "lexical".to_owned(),
        parser_version: "1".to_owned(),
        coverage: CoverageKind::Lexical,
        diagnostics: vec![ParseDiagnostic {
            message: "该语言暂无语法解析器，仅启用词法级规则；函数/类/导入指标不参与统计"
                .to_owned(),
            range: None,
            kind: "coverage".to_owned(),
        }],
        functions: vec![],
        classes: vec![],
        imports: vec![],
        calls: vec![],
        comments,
        identifiers: vec![],
        control_flow: vec![],
        inheritance: vec![],
        component_references: vec![],
        python_quality: Default::default(),
        metrics: super::metrics::FileMetrics {
            line_count: index.line_count(),
            code_lines: non_blank.saturating_sub(comment_lines),
            comment_lines,
            function_count: None,
            class_count: None,
            import_count: None,
        },
    }
}
