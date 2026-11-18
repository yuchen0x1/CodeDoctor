//! Go grammar facts (tree-sitter-go).

use super::common::{clamp_text, unquote};
use super::{LangSpec, Node, field_text};
use crate::parse::{ImportBinding, ImportInfo};

fn go_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for spec in node.children(&mut cursor) {
        if spec.kind() != "import_spec" {
            // import_declaration may contain import_spec_list or comments
            let mut inner = spec.walk();
            for nested in spec.children(&mut inner) {
                if nested.kind() == "import_spec" {
                    if let Some(info) = go_import_spec(nested, bytes, text) {
                        out.push(info);
                    }
                }
            }
            continue;
        }
        if let Some(info) = go_import_spec(spec, bytes, text) {
            out.push(info);
        }
    }
    out
}

fn go_import_spec(spec: Node, bytes: &[u8], _text: &str) -> Option<ImportInfo> {
    let path = spec
        .child_by_field_name("path")
        .and_then(|n| n.utf8_text(bytes).ok())
        .map(unquote)?;
    let module = clamp_text(&path, 200);
    let alias = spec
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(bytes).ok());
    let binding_name = match alias {
        Some(a) if a != "_" => a.to_owned(),
        _ => module.rsplit('/').next().unwrap_or("").to_owned(),
    };
    let names = if binding_name.is_empty() || alias == Some("_") {
        vec![]
    } else {
        vec![ImportBinding {
            name: binding_name,
            alias: None,
        }]
    };
    Some(ImportInfo {
        module,
        names,
        range: Default::default(),
        dynamic: false,
    })
}

/// Go methods declare their owner via a receiver, not an enclosing class.
fn go_parent_class(node: Node, text: &str) -> Option<String> {
    if node.kind() != "method_declaration" {
        return None;
    }
    let receiver = node.child_by_field_name("receiver")?;
    let declaration = receiver.named_child(0)?;
    let type_node = declaration.child_by_field_name("type")?;
    let raw = type_node.utf8_text(text.as_bytes()).unwrap_or("");
    let raw = raw.trim_start_matches(['*', '[', ']', ' ']);
    let raw = raw.split('<').next().unwrap_or(raw).trim();
    if raw.is_empty() {
        None
    } else {
        Some(clamp_text(raw, 80))
    }
}

fn go_exported(node: Node, text: &str) -> bool {
    let name = field_text(node, "name", text);
    name.chars()
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false)
}

fn go_bases(_node: Node, _text: &str) -> Vec<String> {
    Vec::new()
}

pub static SPEC: LangSpec = LangSpec {
    language: "Go",
    grammar: || tree_sitter_go::LANGUAGE.into(),
    grammar_version: "0.23",
    function_kinds: &["function_declaration", "method_declaration"],
    arrow_kinds: &[],
    class_kinds: &["type_spec"],
    impl_kinds: &[],
    property_kinds: &["field_declaration"],
    import_kinds: &["import_declaration"],
    call_kinds: &["call_expression"],
    comment_kinds: &["comment"],
    identifier_kinds: &[
        "identifier",
        "field_identifier",
        "type_identifier",
        "package_identifier",
    ],
    control_flow_kinds: &[
        "if_statement",
        "for_statement",
        "expression_case",
        "default_case",
        "communication_case",
    ],
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &[],
    no_descend_kinds: &["interpreted_string_literal", "raw_string_literal"],
    parameter_fields: &["parameters"],
    dynamic_calls: &[],
    docstring: false,
    import_fn: go_imports,
    callee_fn: |node, text| field_text(node, "function", text),
    name_fn: None,
    parent_class_fn: Some(go_parent_class),
    bases_fn: Some(go_bases),
    exported_fn: go_exported,
    post_fn: None,
};
