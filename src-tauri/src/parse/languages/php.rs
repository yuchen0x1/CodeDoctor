//! PHP grammar facts (tree-sitter-php).

use super::common::clamp_text;
use super::{LangSpec, Node, field_text};
use crate::parse::{ImportBinding, ImportInfo};

fn php_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // `use App\Loader as L;` → namespace_name or namespace_use_clause.
        let raw = match child.kind() {
            "namespace_name" | "namespace_use_clause" | "namespace_name_as_name" => {
                child.utf8_text(bytes).unwrap_or("").trim().to_owned()
            }
            _ => continue,
        };
        if raw.is_empty() {
            continue;
        }
        let (raw, alias) = match raw.split_once(" as ") {
            Some((path, alias)) => (path.trim().to_owned(), Some(clamp_text(alias.trim(), 80))),
            None => (raw, None),
        };
        let (module, name) = match raw.rsplit_once('\\') {
            Some((m, n)) => (m.to_owned(), n.to_owned()),
            None => (String::new(), raw.clone()),
        };
        out.push(ImportInfo {
            module,
            names: vec![ImportBinding { name, alias }],
            range: Default::default(),
            binding_range: None,
            dynamic: false,
        });
    }
    out
}

fn php_bases(node: Node, text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut bases = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "base_clause" | "class_interface_clause") {
            let mut inner = child.walk();
            for base in child.children(&mut inner) {
                if matches!(base.kind(), "name" | "qualified_name") {
                    let raw = base.utf8_text(bytes).unwrap_or("");
                    bases.push(clamp_text(raw, 80));
                }
            }
        }
    }
    bases
}

fn php_callee(node: Node, text: &str) -> String {
    let function = field_text(node, "function", text);
    if !function.is_empty() {
        return function;
    }
    let name = field_text(node, "name", text);
    if name.is_empty() {
        return String::new();
    }
    let object = node
        .child_by_field_name("object")
        .or_else(|| node.child_by_field_name("scope"))
        .and_then(|n| n.utf8_text(text.as_bytes()).ok())
        .map(|t| clamp_text(t, 60))
        .unwrap_or_default();
    if object.is_empty() {
        name
    } else {
        format!("{object}.{name}")
    }
}

pub static SPEC: LangSpec = LangSpec {
    language: "PHP",
    grammar: || tree_sitter_php::LANGUAGE_PHP.into(),
    grammar_version: "0.23",
    function_kinds: &[
        "function_definition",
        "method_declaration",
        "arrow_function",
        "anonymous_function",
    ],
    arrow_kinds: &["arrow_function", "anonymous_function"],
    class_kinds: &[
        "class_declaration",
        "interface_declaration",
        "trait_declaration",
        "enum_declaration",
    ],
    impl_kinds: &[],
    property_kinds: &["property_declaration"],
    import_kinds: &["namespace_use_declaration"],
    call_kinds: &[
        "function_call_expression",
        "member_call_expression",
        "scoped_call_expression",
        "call_expression",
    ],
    comment_kinds: &["comment"],
    identifier_kinds: &["name", "variable_name"],
    control_flow_kinds: &[
        "if_statement",
        "else_if_clause",
        "for_statement",
        "while_statement",
        "do_statement",
        "case_statement",
        "catch_clause",
        "conditional_expression",
    ],
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &[],
    no_descend_kinds: &["string"],
    parameter_fields: &["formal_parameters", "parameters"],
    dynamic_calls: &[
        "eval",
        "call_user_func",
        "call_user_func_array",
        "create_function",
        "assert",
    ],
    docstring: false,
    import_fn: php_imports,
    callee_fn: php_callee,
    name_fn: None,
    parent_class_fn: None,
    bases_fn: Some(php_bases),
    exported_fn: |_, _| false,
    post_fn: None,
};
