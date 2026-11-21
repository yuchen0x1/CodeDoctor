//! Shared tree-sitter extraction engine. Walks one syntax tree driven by a
//! [`LangSpec`], producing the serializable [`ParseResult`] contract. Composite
//! files (Vue/Svelte/HTML) are partitioned into script blocks and parsed with
//! the matching JS/TS grammar, with all ranges mapped back to the original
//! source bytes and line numbers.

use super::languages::common::{LineIndex, clamp_text, comment_kind, lexical_comment_scan};
use super::languages::{LangSpec, Node, is_binary_kind, logical_kind, spec_for};
use crate::domain::types::CoverageKind;
use crate::parse::metrics::{FileMetrics, FunctionMetrics};
use crate::parse::{
    CallInfo, ClassInfo, CommentInfo, ControlFlowInfo, FunctionInfo, IdentifierInfo,
    ParseDiagnostic, ParseResult, RelationInfo, SourceRange,
};
use std::collections::HashSet;

const MAX_IDENTIFIERS: usize = 20_000;
const MAX_SYNTAX_DIAGNOSTICS: usize = 50;
const MAX_NAME_LEN: usize = 80;

struct Ctx {
    function_index: Option<usize>,
    class: Option<String>,
    ctrl_depth: u32,
}

#[derive(Default)]
struct WalkOutput {
    functions: Vec<FunctionInfo>,
    classes: Vec<ClassInfo>,
    imports: Vec<crate::parse::ImportInfo>,
    calls: Vec<CallInfo>,
    comments: Vec<CommentInfo>,
    identifiers: Vec<IdentifierInfo>,
    control_flow: Vec<ControlFlowInfo>,
    /// (function index, nesting depth, decision kind) for metric aggregation.
    control_records: Vec<(usize, u32, &'static str)>,
    inheritance: Vec<RelationInfo>,
    component_references: Vec<RelationInfo>,
}

pub(super) fn range_of(node: Node, index: &LineIndex) -> SourceRange {
    let range = node.byte_range();
    let (start_line, start_column) = index.line_col(range.start);
    let (end_line, end_column) = index.line_col(range.end);
    SourceRange {
        start_line,
        start_column,
        end_line,
        end_column,
        start_byte: range.start,
        end_byte: range.end,
    }
}

fn walk_root(spec: &'static LangSpec, root: Node, text: &str, index: &LineIndex) -> WalkOutput {
    let bytes = text.as_bytes();
    let mut out = WalkOutput::default();
    let mut stack: Vec<(Node, Ctx)> = vec![(
        root,
        Ctx {
            function_index: None,
            class: None,
            ctrl_depth: 0,
        },
    )];

    while let Some((node, ctx)) = stack.pop() {
        let kind = node.kind();

        // Anonymous tokens (keywords, punctuation) are leaves; skipping them
        // prevents keyword tokens like `function` from matching named rules
        // that share the same kind string.
        if !node.is_named() || spec.no_descend_kinds.contains(&kind) {
            continue;
        }
        if spec.comment_kinds.contains(&kind) {
            let raw = node.utf8_text(bytes).unwrap_or("");
            out.comments.push(CommentInfo {
                text: clamp_text(raw, 500),
                range: range_of(node, index),
                kind: comment_kind(raw).to_owned(),
            });
            continue;
        }

        let mut handled_import = false;
        if spec.import_kinds.contains(&kind) {
            let mut entries = (spec.import_fn)(node, text);
            if !entries.is_empty() {
                handled_import = true;
                for entry in &mut entries {
                    entry.range = range_of(node, index);
                }
                out.imports.append(&mut entries);
            }
        }

        if spec.call_kinds.contains(&kind) && !handled_import {
            let callee = (spec.callee_fn)(node, text);
            if !callee.is_empty() {
                let caller = ctx
                    .function_index
                    .and_then(|i| out.functions.get(i))
                    .map(|f| f.name.clone());
                out.calls.push(CallInfo {
                    callee: clamp_text(&callee, MAX_NAME_LEN),
                    range: range_of(node, index),
                    caller,
                    dynamic: is_dynamic(&callee, spec),
                });
            }
        }

        let mut new_ctx = Ctx {
            function_index: ctx.function_index,
            class: ctx.class.clone(),
            ctrl_depth: ctx.ctrl_depth,
        };

        if spec.is_function_kind(kind) {
            let function = extract_function(spec, node, text, index, ctx.class.as_deref());
            out.functions.push(function);
            new_ctx.function_index = Some(out.functions.len() - 1);
            new_ctx.ctrl_depth = 0;
        } else if spec.class_kinds.contains(&kind) {
            let class = extract_class(spec, node, text);
            new_ctx.class = Some(class.name.clone());
            out.classes.push(class);
        } else if spec.impl_kinds.contains(&kind) {
            let owner = node
                .child_by_field_name("type")
                .and_then(|n| n.utf8_text(bytes).ok())
                .map(|raw| {
                    let t = raw.split('<').next().unwrap_or(raw).trim();
                    clamp_text(t, MAX_NAME_LEN)
                });
            if let Some(owner) = owner {
                new_ctx.class = Some(owner);
            }
        }

        if spec.jsx_kinds.contains(&kind) {
            if let Some(target) = jsx_component_target(node, bytes) {
                let source = ctx
                    .function_index
                    .and_then(|i| out.functions.get(i))
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                out.component_references.push(RelationInfo {
                    source,
                    target,
                    range: range_of(node, index),
                });
            }
        }

        if let Some((label, is_logical)) = control_label(spec, node, bytes) {
            if let Some(i) = ctx.function_index {
                out.control_records.push((i, ctx.ctrl_depth + 1, label));
            }
            out.control_flow.push(ControlFlowInfo {
                kind: label.to_owned(),
                range: range_of(node, index),
                function: ctx
                    .function_index
                    .and_then(|i| out.functions.get(i))
                    .map(|f| f.name.clone()),
            });
            new_ctx.ctrl_depth = ctx.ctrl_depth + 1;
            let _ = is_logical;
        }

        if spec.identifier_kinds.contains(&kind) && out.identifiers.len() < MAX_IDENTIFIERS {
            let name = node.utf8_text(bytes).unwrap_or("");
            out.identifiers.push(IdentifierInfo {
                name: clamp_text(name, MAX_NAME_LEN),
                range: range_of(node, index),
            });
        }

        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((
                child,
                Ctx {
                    function_index: new_ctx.function_index,
                    class: new_ctx.class.clone(),
                    ctrl_depth: new_ctx.ctrl_depth,
                },
            ));
        }
    }
    out
}

/// Returns the semantic control-flow label for a node, if it counts as a
/// decision point. The second flag marks logical operators (shared engine).
fn control_label(
    spec: &'static LangSpec,
    node: Node,
    bytes: &[u8],
) -> Option<(&'static str, bool)> {
    let kind = node.kind();
    if spec.control_flow_kinds.contains(&kind)
        || spec.case_arm_kinds.contains(&kind)
        || spec.ternary_kinds.contains(&kind)
    {
        return Some((decision_kind_label(kind), false));
    }
    if is_binary_kind(kind) {
        let operator = node
            .child_by_field_name("operator")
            .and_then(|n| n.utf8_text(bytes).ok())
            .or_else(|| {
                node.children(&mut node.walk())
                    .find(|c| !c.is_named())
                    .and_then(|c| c.utf8_text(bytes).ok())
            })
            .unwrap_or("");
        return logical_kind(operator).map(|label| (label, true));
    }
    None
}

fn decision_kind_label(kind: &str) -> &'static str {
    match kind {
        "if_statement" | "if_expression" => "if",
        "elif_clause" | "else_if_clause" => "elif",
        "for_statement" | "for_expression" | "async_for_statement" => "for",
        "while_statement" | "while_expression" => "while",
        "do_statement" | "do_while_statement" => "do_while",
        "except_clause" | "catch_clause" | "catch_block" => "catch",
        "case_clause"
        | "case_statement"
        | "expression_case"
        | "default_case"
        | "communication_case"
        | "switch_case"
        | "switch_default"
        | "switch_section"
        | "switch_block_statement_group"
        | "match_arm"
        | "when_entry" => "case",
        "conditional_expression" | "ternary_expression" => "ternary",
        "loop_expression" => "loop",
        _ => "branch",
    }
}

fn is_dynamic(callee: &str, spec: &'static LangSpec) -> bool {
    let normalized = callee.replace("::", ".");
    normalized
        .rsplit('.')
        .next()
        .map(|last| spec.dynamic_calls.contains(&last))
        .unwrap_or(false)
}

fn extract_function(
    spec: &'static LangSpec,
    node: Node,
    text: &str,
    index: &LineIndex,
    enclosing_class: Option<&str>,
) -> FunctionInfo {
    let name = resolve_function_name(spec, node, text);
    let parent_class = spec
        .parent_class_fn
        .and_then(|f| f(node, text))
        .or_else(|| enclosing_class.map(str::to_owned));
    let (parameters, parameter_ranges) = find_params(spec, node)
        .map(|container| params_of(container, text, index))
        .unwrap_or_default();
    let body_range = node.child_by_field_name("body").map(|n| range_of(n, index));
    FunctionInfo {
        name,
        range: range_of(node, index),
        body_range,
        parameters,
        parameter_ranges,
        metrics: FunctionMetrics::default(),
        parent_class,
        is_exported: (spec.exported_fn)(node, text),
    }
}

fn resolve_function_name(spec: &'static LangSpec, node: Node, text: &str) -> String {
    let bytes = text.as_bytes();
    if !spec.arrow_kinds.contains(&node.kind()) {
        if let Some(named) = node.child_by_field_name("name") {
            if let Ok(raw) = named.utf8_text(bytes) {
                return clamp_text(raw, MAX_NAME_LEN);
            }
        }
        if let Some(name_fn) = spec.name_fn {
            if let Some(name) = name_fn(node, text) {
                return name;
            }
        }
    }
    let mut parent = node.parent();
    for _ in 0..4 {
        match parent {
            Some(p) if p.kind() == "variable_declarator" => {
                return field_or_first(p, "name", text);
            }
            Some(p) if p.kind() == "pair" => return field_or_first(p, "key", text),
            Some(p) if p.kind() == "assignment_expression" => {
                return field_or_first(p, "left", text);
            }
            Some(p)
                if p.kind() == "property_signature" || p.kind() == "public_field_definition" =>
            {
                return field_or_first(p, "name", text);
            }
            Some(p) => parent = p.parent(),
            None => break,
        }
    }
    "<anonymous>".to_owned()
}

fn field_or_first(node: Node, field: &str, text: &str) -> String {
    let target = node
        .child_by_field_name(field)
        .or_else(|| node.named_child(0));
    target
        .and_then(|n| n.utf8_text(text.as_bytes()).ok())
        .map(|t| clamp_text(t, MAX_NAME_LEN))
        .unwrap_or_else(|| "<anonymous>".to_owned())
}

fn find_params<'a>(spec: &'static LangSpec, node: Node<'a>) -> Option<Node<'a>> {
    for field in spec.parameter_fields {
        if let Some(n) = node.child_by_field_name(field) {
            return Some(n);
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        for field in spec.parameter_fields {
            if let Some(n) = child.child_by_field_name(field) {
                return Some(n);
            }
        }
    }
    None
}

const PARAMETER_CONTAINERS: &[&str] = &[
    "parameters",
    "formal_parameters",
    "parameter_list",
    "value_parameters",
    "formal_parameter_list",
];

fn params_of(container: Node, text: &str, index: &LineIndex) -> (Vec<String>, Vec<SourceRange>) {
    let bytes = text.as_bytes();
    let mut cursor = container.walk();
    let nodes: Vec<_> = if PARAMETER_CONTAINERS.contains(&container.kind()) {
        container.named_children(&mut cursor).collect()
    } else {
        vec![container]
    };
    nodes
        .into_iter()
        .filter_map(|node| {
            let target = ["name", "pattern", "left"]
                .into_iter()
                .find_map(|field| node.child_by_field_name(field))
                .unwrap_or(node);
            let name = clamp_text(target.utf8_text(bytes).unwrap_or(""), 60);
            (!name.is_empty() && name != ",").then(|| (name, range_of(target, index)))
        })
        .unzip()
}

fn extract_class(spec: &'static LangSpec, node: Node, text: &str) -> ClassInfo {
    let bytes = text.as_bytes();
    let name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(bytes).ok())
        .map(|t| clamp_text(t, MAX_NAME_LEN))
        .unwrap_or_else(|| "<anonymous>".to_owned());
    let bases = spec.bases_fn.map(|f| f(node, text)).unwrap_or_default();
    let (method_count, property_count) = match class_body(node) {
        Some(body) => {
            let mut methods = 0u32;
            let mut properties = 0u32;
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                let child_kind = child.kind();
                if child_kind == "decorated_definition" {
                    let mut inner = child.walk();
                    for wrapped in child.children(&mut inner) {
                        if spec.is_function_kind(wrapped.kind()) {
                            methods += 1;
                        }
                    }
                } else if spec.is_function_kind(child_kind) {
                    methods += 1;
                } else {
                    // Python wraps class-level assignments in expression statements.
                    let effective_kind = if child_kind == "expression_statement" {
                        child.named_child(0).map(|n| n.kind()).unwrap_or(child_kind)
                    } else {
                        child_kind
                    };
                    if spec.property_kinds.contains(&effective_kind) {
                        properties += 1;
                    }
                }
            }
            (Some(methods), Some(properties))
        }
        None => (None, None),
    };
    ClassInfo {
        name,
        range: range_of(node, &LineIndex::new(text)),
        bases,
        method_count,
        property_count,
    }
}

fn class_body<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if let Some(body) = node.child_by_field_name("body") {
        return Some(body);
    }
    // Go type_spec nests the body under the type field.
    if let Some(ty) = node.child_by_field_name("type") {
        if let Some(body) = ty.child_by_field_name("body") {
            return Some(body);
        }
        if ty.kind() == "struct_type" || ty.kind() == "interface_type" {
            return ty.named_child(0);
        }
    }
    None
}

fn jsx_component_target(node: Node, bytes: &[u8]) -> Option<String> {
    let name_node = if node.kind() == "jsx_self_closing_element" {
        node.child_by_field_name("name")
    } else {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| c.kind() == "jsx_opening_element")
            .and_then(|opening| opening.child_by_field_name("name"))
    }?;
    let raw = name_node.utf8_text(bytes).ok()?;
    let first = raw.chars().next()?;
    if first.is_uppercase() {
        Some(clamp_text(raw, MAX_NAME_LEN))
    } else {
        None
    }
}

/// Builds the full ParseResult for a grammar-backed language.
pub fn parse_with_spec(spec: &'static LangSpec, text: &str) -> ParseResult {
    let index = LineIndex::new(text);
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = (spec.grammar)().into();
    let tree = match parser.set_language(&language) {
        Ok(()) => parser.parse(text.as_bytes(), None),
        Err(_) => None,
    };

    let Some(tree) = tree else {
        return ParseResult {
            coverage: CoverageKind::Lexical,
            diagnostics: vec![ParseDiagnostic {
                message: "语法解析器未能生成语法树".to_owned(),
                range: None,
                kind: "parser".to_owned(),
            }],
            ..empty_result(spec.grammar_version)
        };
    };

    let root = tree.root_node();
    let mut diagnostics = Vec::new();
    collect_syntax_errors(root, text.as_bytes(), &index, &mut diagnostics);

    let mut out = walk_root(spec, root, text, &index);

    if spec.docstring {
        collect_docstrings(root, text, &index, &mut out);
    }

    if let Some(post) = spec.post_fn {
        post(&mut out.classes, &out.functions);
    }

    let coverage = if diagnostics.is_empty() {
        CoverageKind::Ast
    } else {
        CoverageKind::Structural
    };
    let python_quality = if spec.language == "Python" && coverage == CoverageKind::Ast {
        super::python_quality::extract(root, text, &index)
    } else {
        Default::default()
    };

    aggregate_function_metrics(&mut out);

    let (code_lines, comment_lines) = line_classification(text, &out.comments);

    ParseResult {
        parser_name: "tree-sitter".to_owned(),
        parser_version: spec.grammar_version.to_owned(),
        coverage,
        diagnostics,
        functions: out.functions,
        classes: out.classes,
        imports: out.imports,
        calls: out.calls,
        comments: out.comments,
        identifiers: out.identifiers,
        control_flow: out.control_flow,
        inheritance: out.inheritance,
        component_references: out.component_references,
        python_quality,
        metrics: FileMetrics {
            line_count: index.line_count(),
            code_lines,
            comment_lines,
            function_count: Some(0),
            class_count: Some(0),
            import_count: Some(0),
        },
    }
}

/// Derives length/branch/nesting/cyclomatic/cognitive per function from the
/// collected control records and function ranges.
fn aggregate_function_metrics(out: &mut WalkOutput) {
    for i in 0..out.functions.len() {
        let mut metrics = FunctionMetrics::default();
        let (length, params) = {
            let f = &out.functions[i];
            let length = f
                .range
                .end_line
                .saturating_sub(f.range.start_line)
                .saturating_add(1);
            (length, f.parameters.len() as u32)
        };
        metrics.length = Some(length);
        metrics.parameter_count = Some(params);
        for &(fn_index, depth, _) in &out.control_records {
            if fn_index == i {
                metrics.branch_count = Some(metrics.branch_count.unwrap_or(0) + 1);
                metrics.nesting_depth = Some(metrics.nesting_depth.unwrap_or(0).max(depth));
                metrics.cognitive_complexity =
                    Some(metrics.cognitive_complexity.unwrap_or(0) + depth);
            }
        }
        metrics.cyclomatic_complexity = Some(metrics.branch_count.unwrap_or(0) + 1);
        out.functions[i].metrics = metrics;
    }
}

fn empty_result(version: &str) -> ParseResult {
    ParseResult {
        parser_name: "tree-sitter".to_owned(),
        parser_version: version.to_owned(),
        coverage: CoverageKind::Lexical,
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
        metrics: FileMetrics::default(),
    }
}

fn collect_syntax_errors(
    root: Node,
    bytes: &[u8],
    index: &LineIndex,
    out: &mut Vec<ParseDiagnostic>,
) {
    if !root.has_error() {
        return;
    }
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            let (line, column) = index.line_col(node.byte_range().start);
            let raw = node.utf8_text(bytes).unwrap_or("");
            out.push(ParseDiagnostic {
                message: format!(
                    "语法解析错误（第 {line} 行第 {column} 列附近）：{}",
                    clamp_text(raw, 60)
                ),
                range: Some(range_of(node, index)),
                kind: "syntax".to_owned(),
            });
            if out.len() >= MAX_SYNTAX_DIAGNOSTICS {
                return;
            }
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        for child in children {
            stack.push(child);
        }
    }
}

/// Collects Python-style docstrings (first string statement of module, class,
/// and function bodies) as documentation comments.
fn collect_docstrings(root: Node, text: &str, index: &LineIndex, out: &mut WalkOutput) {
    let bytes = text.as_bytes();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let is_body_scope = matches!(node.kind(), "module" | "block" | "class_body");
        if is_body_scope {
            let mut cursor = node.walk();
            let first = node.children(&mut cursor).find(|c| c.is_named());
            if let Some(statement) = first {
                if statement.kind() == "expression_statement" {
                    let mut inner = statement.walk();
                    let string = statement
                        .children(&mut inner)
                        .find(|c| c.kind() == "string");
                    if let Some(string) = string {
                        let raw = string.utf8_text(bytes).unwrap_or("");
                        let content = raw.trim_matches(|c: char| c == '"' || c == '\'');
                        if !content.trim().is_empty() {
                            out.comments.push(CommentInfo {
                                text: clamp_text(content, 500),
                                range: range_of(string, index),
                                kind: "doc".to_owned(),
                            });
                        }
                    }
                }
            }
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        for child in children {
            stack.push(child);
        }
    }
}

/// Counts code vs. comment lines: comment lines come from collected comment
/// spans, code lines are the remaining non-blank lines.
fn line_classification(text: &str, comments: &[CommentInfo]) -> (u32, u32) {
    let non_blank = text.lines().filter(|l| !l.trim().is_empty()).count() as u32;
    let mut comment_line_set: HashSet<u32> = HashSet::new();
    for comment in comments {
        let last = comment.range.end_line.max(comment.range.start_line);
        for line in comment.range.start_line..=last {
            comment_line_set.insert(line);
        }
    }
    let comment_lines = comment_line_set.len() as u32;
    (non_blank.saturating_sub(comment_lines), comment_lines)
}

/// Remaps every range in a parse result from block-local bytes to the original
/// composite file, using the original line index.
fn remap_result(result: &mut ParseResult, shift: usize, index: &LineIndex) {
    fn fix(r: &mut SourceRange, shift: usize, index: &LineIndex) {
        let start = r.start_byte + shift;
        let end = r.end_byte + shift;
        let (start_line, start_column) = index.line_col(start);
        let (end_line, end_column) = index.line_col(end);
        r.start_byte = start;
        r.end_byte = end;
        r.start_line = start_line;
        r.start_column = start_column;
        r.end_line = end_line;
        r.end_column = end_column;
    }
    for d in &mut result.diagnostics {
        if let Some(range) = &mut d.range {
            fix(range, shift, index);
        }
    }
    for f in &mut result.functions {
        fix(&mut f.range, shift, index);
        if let Some(body) = &mut f.body_range {
            fix(body, shift, index);
        }
        for parameter in &mut f.parameter_ranges {
            fix(parameter, shift, index);
        }
    }
    for c in &mut result.classes {
        fix(&mut c.range, shift, index);
    }
    for i in &mut result.imports {
        fix(&mut i.range, shift, index);
        if let Some(binding) = &mut i.binding_range {
            fix(binding, shift, index);
        }
    }
    for c in &mut result.calls {
        fix(&mut c.range, shift, index);
    }
    for c in &mut result.comments {
        fix(&mut c.range, shift, index);
    }
    for i in &mut result.identifiers {
        fix(&mut i.range, shift, index);
    }
    for c in &mut result.control_flow {
        fix(&mut c.range, shift, index);
    }
    for r in &mut result.inheritance {
        fix(&mut r.range, shift, index);
    }
    for r in &mut result.component_references {
        fix(&mut r.range, shift, index);
    }
}

struct ScriptBlock {
    content_start: usize,
    content_end: usize,
    lang: &'static str,
}

/// Finds inline `<script>` blocks in a composite file (Vue/Svelte/HTML).
fn script_blocks(text: &str) -> Vec<ScriptBlock> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("<script") {
        let open_tag_start = cursor + rel;
        let after = open_tag_start + "<script".len();
        let boundary =
            after < bytes.len() && matches!(bytes[after], b' ' | b'>' | b'\t' | b'\n' | b'\r');
        if !boundary {
            cursor = open_tag_start + 7;
            continue;
        }
        let Some(open_end_rel) = lower[open_tag_start..].find('>') else {
            break;
        };
        let open_end = open_tag_start + open_end_rel;
        let attrs = &lower[open_tag_start..open_end];
        let content_start = open_end + 1;
        let Some(close_rel) = lower[content_start..].find("</script") else {
            break;
        };
        let content_end = content_start + close_rel;
        cursor = content_end + 1;

        if attrs.contains("src=") {
            continue;
        }
        let lang = if attrs.contains("lang=\"tsx\"") || attrs.contains("lang='tsx'") {
            "tsx"
        } else if attrs.contains("lang=\"ts\"")
            || attrs.contains("lang='ts'")
            || attrs.contains("lang=\"typescript\"")
            || attrs.contains("type=\"text/typescript\"")
        {
            "ts"
        } else {
            "js"
        };
        blocks.push(ScriptBlock {
            content_start,
            content_end,
            lang,
        });
    }
    blocks
}

/// Maps real opening parentheses to their final source line for privacy spans.
/// Grammar tokens, not characters inside strings/comments, determine the close.
pub(crate) fn parenthesized_ends(
    language: &str,
    text: &str,
) -> Option<std::collections::HashMap<usize, u32>> {
    let index = LineIndex::new(text);
    let blocks = if super::languages::is_composite(language) {
        script_blocks(text)
    } else {
        let spec = spec_for(language)?;
        vec![ScriptBlock {
            content_start: 0,
            content_end: text.len(),
            lang: spec.language,
        }]
    };
    let mut ends = std::collections::HashMap::new();
    for block in blocks {
        let language = match block.lang {
            "tsx" => "TSX",
            "ts" => "TypeScript",
            "js" => "JavaScript",
            other => other,
        };
        let Some(spec) = spec_for(language) else {
            continue;
        };
        let mut parser = ::tree_sitter::Parser::new();
        if parser.set_language(&(spec.grammar)()).is_err() {
            continue;
        }
        let content = &text[block.content_start..block.content_end];
        let Some(tree) = parser.parse(content, None) else {
            continue;
        };
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if !node.has_error()
                && node.child(0).is_some_and(|child| child.kind() == "(")
                && node
                    .child(node.child_count().saturating_sub(1))
                    .is_some_and(|child| child.kind() == ")" && !child.is_missing())
            {
                let start = block.content_start + node.start_byte();
                let end = block.content_start + node.end_byte();
                ends.insert(start, index.line_col(end).0);
            }
            let mut cursor = node.walk();
            stack.extend(node.named_children(&mut cursor));
        }
    }
    Some(ends)
}

/// Parses a composite file by partitioning its script blocks.
pub fn parse_composite(text: &str) -> ParseResult {
    let index = LineIndex::new(text);
    let mut merged = empty_result("1");
    merged.parser_name = "composite-script".to_owned();
    merged.coverage = CoverageKind::Structural;
    merged.diagnostics.push(ParseDiagnostic {
        message: "复合文件（Vue/Svelte/HTML）按嵌入 script 块分区解析；模板与样式未做完整语法解析"
            .to_owned(),
        range: None,
        kind: "coverage".to_owned(),
    });

    let (lexical_comment_lines, _) = lexical_comment_scan(text, "HTML");
    let non_blank = text.lines().filter(|l| !l.trim().is_empty()).count() as u32;
    merged.metrics.line_count = index.line_count();
    merged.metrics.code_lines = non_blank.saturating_sub(lexical_comment_lines);
    merged.metrics.comment_lines = lexical_comment_lines;

    let mut functions = 0u32;
    let mut classes = 0u32;
    let mut imports = 0u32;

    for block in script_blocks(text) {
        let content = &text[block.content_start..block.content_end];
        if content.trim().is_empty() {
            continue;
        }
        let spec = match block.lang {
            "tsx" => spec_for("TSX"),
            "ts" => spec_for("TypeScript"),
            _ => spec_for("JavaScript"),
        };
        let Some(spec) = spec else { continue };
        let mut inner = parse_with_spec(spec, content);
        remap_result(&mut inner, block.content_start, &index);
        functions += inner.functions.len() as u32;
        classes += inner.classes.len() as u32;
        imports += inner.imports.len() as u32;
        merged.functions.append(&mut inner.functions);
        merged.classes.append(&mut inner.classes);
        merged.imports.append(&mut inner.imports);
        merged.calls.append(&mut inner.calls);
        merged.comments.append(&mut inner.comments);
        merged.identifiers.append(&mut inner.identifiers);
        merged.control_flow.append(&mut inner.control_flow);
        merged
            .component_references
            .append(&mut inner.component_references);
        merged.diagnostics.append(&mut inner.diagnostics);
    }

    merged.metrics.function_count = Some(functions);
    merged.metrics.class_count = Some(classes);
    merged.metrics.import_count = Some(imports);
    merged
}
