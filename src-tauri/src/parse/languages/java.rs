//! Java grammar facts (tree-sitter-java).

use super::common::clamp_text;
use super::{LangSpec, Node, field_text};
use crate::parse::{ImportBinding, ImportInfo};

fn java_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let raw = node.utf8_text(text.as_bytes()).unwrap_or("");
    let raw = raw.strip_prefix("import ").unwrap_or(raw).trim();
    let raw = raw.strip_suffix(';').unwrap_or(raw).trim();
    let raw = raw.strip_prefix("static ").unwrap_or(raw).trim();
    if raw.is_empty() || raw == "*" {
        return vec![];
    }
    let (module, name) = match raw.strip_suffix(".*") {
        Some(prefix) => (prefix.to_owned(), String::new()),
        None => match raw.rsplit_once('.') {
            Some((m, n)) => (m.to_owned(), n.to_owned()),
            None => (String::new(), raw.to_owned()),
        },
    };
    let names = if name.is_empty() {
        vec![]
    } else {
        vec![ImportBinding { name, alias: None }]
    };
    vec![ImportInfo {
        module,
        names,
        range: Default::default(),
        dynamic: false,
    }]
}

fn java_bases(node: Node, text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut bases = Vec::new();
    // `superclass` field wraps an `extends Base` clause node; the type is its
    // named child. `interfaces` wraps a `super_interfaces` list.
    if let Some(superclass) = node.child_by_field_name("superclass") {
        let mut cursor = superclass.walk();
        for child in superclass.children(&mut cursor) {
            if child.is_named() {
                let raw = child.utf8_text(bytes).unwrap_or("");
                let base = raw.split('<').next().unwrap_or(raw).trim();
                bases.push(clamp_text(base, 80));
            }
        }
    }
    if let Some(interfaces) = node.child_by_field_name("interfaces") {
        let mut cursor = interfaces.walk();
        for child in interfaces.children(&mut cursor) {
            if child.is_named() {
                let raw = child.utf8_text(bytes).unwrap_or("");
                let base = raw.split('<').next().unwrap_or(raw).trim();
                bases.push(clamp_text(base, 80));
            }
        }
    }
    bases
}

fn java_callee(node: Node, text: &str) -> String {
    if node.kind() == "object_creation_expression" {
        let ty = field_text(node, "type", text);
        return format!("new {ty}");
    }
    let name = field_text(node, "name", text);
    if name.is_empty() {
        return String::new();
    }
    if let Some(object) = node.child_by_field_name("object") {
        let raw = object.utf8_text(text.as_bytes()).unwrap_or("");
        let prefix = clamp_text(raw, 60);
        if !prefix.is_empty() {
            return format!("{prefix}.{name}");
        }
    }
    name
}

fn java_exported(node: Node, _text: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            return child
                .utf8_text(_text.as_bytes())
                .map(|t| t.split_whitespace().any(|m| m == "public"))
                .unwrap_or(false);
        }
    }
    false
}

pub static SPEC: LangSpec = LangSpec {
    language: "Java",
    grammar: || tree_sitter_java::LANGUAGE.into(),
    grammar_version: "0.23",
    function_kinds: &[
        "method_declaration",
        "constructor_declaration",
        "compact_constructor_declaration",
    ],
    arrow_kinds: &[],
    class_kinds: &[
        "class_declaration",
        "interface_declaration",
        "enum_declaration",
        "record_declaration",
    ],
    impl_kinds: &[],
    property_kinds: &["field_declaration"],
    import_kinds: &["import_declaration"],
    call_kinds: &["method_invocation", "object_creation_expression"],
    comment_kinds: &["line_comment", "block_comment"],
    identifier_kinds: &["identifier", "type_identifier"],
    control_flow_kinds: &[
        "if_statement",
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
        "switch_block_statement_group",
        "catch_clause",
        "ternary_expression",
    ],
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &[],
    no_descend_kinds: &["string_literal"],
    parameter_fields: &["parameters"],
    dynamic_calls: &[
        "forName",
        "getMethod",
        "invoke",
        "newInstance",
        "getDeclaredMethod",
    ],
    docstring: false,
    import_fn: java_imports,
    callee_fn: java_callee,
    name_fn: None,
    parent_class_fn: None,
    bases_fn: Some(java_bases),
    exported_fn: java_exported,
    post_fn: None,
};
