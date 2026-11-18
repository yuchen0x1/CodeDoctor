use super::SourceRange;
use super::languages::Node;
use super::languages::common::LineIndex;
use super::tree_sitter::range_of;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;

const MAX_IDENTIFIER_REFERENCES: usize = 100_000;
const FRAME_INSPECTION_NAMES: &[&str] = &[
    "breakpoint",
    "currentframe",
    "dir",
    "eval",
    "exec",
    "f_locals",
    "getframeinfo",
    "locals",
    "settrace",
    "vars",
    "_getframe",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PythonQuality {
    pub(crate) unused_bindings: Vec<UnusedBinding>,
    pub(crate) unreachable: Vec<UnreachableStatement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnusedBinding {
    pub(crate) name: String,
    pub(crate) range: SourceRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnreachableStatement {
    pub(crate) range: SourceRange,
    pub(crate) terminator: String,
    pub(crate) terminator_line: u32,
}

/// Index the existing syntax tree once, including references inside nested
/// scopes and f-string expressions. Ambiguous references suppress findings.
pub(super) fn extract(root: Node<'_>, text: &str, index: &LineIndex) -> PythonQuality {
    let mut stack = vec![root];
    let mut functions = Vec::new();
    let mut references: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut ambiguous = Vec::new();
    let mut identifier_count = 0;

    while let Some(node) = stack.pop() {
        match node.kind() {
            "function_definition" => functions.push(node),
            "global_statement" | "nonlocal_statement" => ambiguous.push(node.start_byte()),
            "identifier" if identifier_count < MAX_IDENTIFIER_REFERENCES => {
                identifier_count += 1;
                let name = node.utf8_text(text.as_bytes()).unwrap_or("");
                if !name.is_ascii() || FRAME_INSPECTION_NAMES.contains(&name) {
                    ambiguous.push(node.start_byte());
                }
                references.entry(name).or_default().push(node.start_byte());
            }
            _ => {}
        }

        let mut cursor = node.walk();
        let children: Vec<_> = node.named_children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }

    let mut findings = PythonQuality::default();
    for function in functions {
        let can_check_bindings = identifier_count < MAX_IDENTIFIER_REFERENCES
            && offsets_in(&ambiguous, &function.byte_range()) == 0;
        let Some(body) = function.child_by_field_name("body") else {
            continue;
        };
        let mut terminator: Option<(&str, u32)> = None;
        for statement in body.named_children(&mut body.walk()) {
            if let Some((terminal, line)) = terminator {
                if !matches!(
                    statement.kind(),
                    "comment"
                        | "global_statement"
                        | "nonlocal_statement"
                        | "function_definition"
                        | "class_definition"
                        | "decorated_definition"
                ) {
                    findings.unreachable.push(UnreachableStatement {
                        range: range_of(statement, index),
                        terminator: terminal.to_owned(),
                        terminator_line: line,
                    });
                }
                continue;
            }

            let terminal = match statement.kind() {
                "return_statement" => Some("return"),
                "raise_statement" => Some("raise"),
                _ => None,
            };
            if let Some(terminal) = terminal {
                terminator = Some((terminal, range_of(statement, index).start_line));
            } else if can_check_bindings
                && let Some(binding) = simple_assignment(statement)
                && let Ok(name) = binding.utf8_text(text.as_bytes())
                && !name.starts_with('_')
                && references
                    .get(name)
                    .is_some_and(|offsets| offsets_in(offsets, &function.byte_range()) == 1)
            {
                findings.unused_bindings.push(UnusedBinding {
                    name: name.to_owned(),
                    range: range_of(binding, index),
                });
            }
        }
    }
    findings
}

fn offsets_in(offsets: &[usize], range: &Range<usize>) -> usize {
    offsets.partition_point(|offset| *offset < range.end)
        - offsets.partition_point(|offset| *offset < range.start)
}

fn simple_assignment(statement: Node<'_>) -> Option<Node<'_>> {
    if statement.kind() != "expression_statement" || statement.named_child_count() != 1 {
        return None;
    }
    let assignment = statement.named_child(0)?;
    if assignment.kind() != "assignment" || assignment.child_by_field_name("type").is_some() {
        return None;
    }
    let right = assignment.child_by_field_name("right")?;
    let left = assignment.child_by_field_name("left")?;
    (right.kind() != "assignment" && left.kind() == "identifier").then_some(left)
}
