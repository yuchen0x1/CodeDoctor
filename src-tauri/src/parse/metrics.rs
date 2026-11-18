use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionMetrics {
    pub length: Option<u32>,
    pub parameter_count: Option<u32>,
    pub branch_count: Option<u32>,
    pub nesting_depth: Option<u32>,
    pub cyclomatic_complexity: Option<u32>,
    pub cognitive_complexity: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMetrics {
    pub line_count: u32,
    pub code_lines: u32,
    pub comment_lines: u32,
    pub function_count: Option<u32>,
    pub class_count: Option<u32>,
    pub import_count: Option<u32>,
}
