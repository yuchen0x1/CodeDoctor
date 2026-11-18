//! Versioned, serializable parser output. No parser executes project code.
pub mod languages;
pub mod metrics;
pub(crate) mod python_quality;
pub mod registry;
pub mod tree_sitter;

use crate::domain::types::{CoverageKind, FileRecord};
pub use registry::ParserRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedFile {
    pub file: FileRecord,
    pub text: String,
    pub parse: ParseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub parser_name: String,
    pub parser_version: String,
    pub coverage: CoverageKind,
    pub diagnostics: Vec<ParseDiagnostic>,
    pub functions: Vec<FunctionInfo>,
    pub classes: Vec<ClassInfo>,
    pub imports: Vec<ImportInfo>,
    pub calls: Vec<CallInfo>,
    pub comments: Vec<CommentInfo>,
    pub identifiers: Vec<IdentifierInfo>,
    pub control_flow: Vec<ControlFlowInfo>,
    pub inheritance: Vec<RelationInfo>,
    pub component_references: Vec<RelationInfo>,
    #[serde(default)]
    pub(crate) python_quality: python_quality::PythonQuality,
    pub metrics: metrics::FileMetrics,
}

impl Default for ParseResult {
    fn default() -> Self {
        Self {
            parser_name: "none".to_owned(),
            parser_version: "0".to_owned(),
            coverage: CoverageKind::NotCovered,
            diagnostics: vec![],
            functions: vec![],
            classes: vec![],
            imports: vec![],
            calls: vec![],
            comments: vec![],
            identifiers: vec![],
            control_flow: vec![],
            inheritance: vec![],
            component_references: vec![],
            python_quality: Default::default(),
            metrics: metrics::FileMetrics::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseDiagnostic {
    pub message: String,
    pub range: Option<SourceRange>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionInfo {
    pub name: String,
    pub range: SourceRange,
    pub body_range: Option<SourceRange>,
    pub parameters: Vec<String>,
    pub metrics: metrics::FunctionMetrics,
    pub parent_class: Option<String>,
    pub is_exported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassInfo {
    pub name: String,
    pub range: SourceRange,
    pub bases: Vec<String>,
    pub method_count: Option<u32>,
    pub property_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBinding {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportInfo {
    pub module: String,
    pub names: Vec<ImportBinding>,
    pub range: SourceRange,
    pub dynamic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallInfo {
    pub callee: String,
    pub range: SourceRange,
    pub caller: Option<String>,
    pub dynamic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInfo {
    pub text: String,
    pub range: SourceRange,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifierInfo {
    pub name: String,
    pub range: SourceRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlFlowInfo {
    pub kind: String,
    pub range: SourceRange,
    pub function: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationInfo {
    pub source: String,
    pub target: String,
    pub range: SourceRange,
}

#[cfg(test)]
mod tests;
