//! TypeScript/TSX grammar facts (tree-sitter-typescript, TSX language).

use super::common::clamp_text;
use super::javascript::{js_exported, js_imports};
use super::{LangSpec, Node, field_text};

/// Extracts `extends`/`implements`/`interface extends` bases shared by JS
/// class heritage and TS declarations.
pub fn js_bases(node: Node, text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut bases = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_heritage" => {
                let mut inner = child.walk();
                for clause in child.children(&mut inner) {
                    match clause.kind() {
                        "extends_clause" => {
                            if let Some(value) = clause.child_by_field_name("value") {
                                let raw = value.utf8_text(bytes).unwrap_or("");
                                let base = raw.split('<').next().unwrap_or(raw).trim();
                                bases.push(clamp_text(base, 80));
                            }
                        }
                        "implements_clause" => {
                            let mut inner2 = clause.walk();
                            for ty in clause.children(&mut inner2) {
                                if ty.kind() == "type_identifier" {
                                    let raw = ty.utf8_text(bytes).unwrap_or("");
                                    let base = raw.split('<').next().unwrap_or(raw).trim();
                                    bases.push(clamp_text(base, 80));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "extends_type_clause" => {
                // TS interface extends
                if let Some(value) = child.child_by_field_name("value") {
                    let raw = value.utf8_text(bytes).unwrap_or("");
                    bases.push(clamp_text(raw.split('<').next().unwrap_or(raw).trim(), 80));
                }
                let mut inner = child.walk();
                for ty in child.children(&mut inner) {
                    if ty.kind() == "type_identifier" {
                        bases.push(clamp_text(ty.utf8_text(bytes).unwrap_or(""), 80));
                    }
                }
            }
            "extends_clause" => {
                if let Some(value) = child.child_by_field_name("value") {
                    let raw = value.utf8_text(bytes).unwrap_or("");
                    bases.push(clamp_text(raw.split('<').next().unwrap_or(raw).trim(), 80));
                }
            }
            _ => {}
        }
    }
    bases.retain(|b| !b.is_empty());
    bases
}

pub const TS_BASE: LangSpec = LangSpec {
    language: "TypeScript",
    grammar: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    grammar_version: "0.23",
    function_kinds: &[
        "function_declaration",
        "generator_function_declaration",
        "method_definition",
        "function_signature",
        "function",
        "arrow_function",
    ],
    arrow_kinds: &["function", "arrow_function"],
    class_kinds: &[
        "class_declaration",
        "class",
        "interface_declaration",
        "enum_declaration",
    ],
    impl_kinds: &[],
    property_kinds: &[
        "public_field_definition",
        "field_definition",
        "property_signature",
    ],
    import_kinds: &["import_statement", "export_statement", "call_expression"],
    call_kinds: &["call_expression", "new_expression"],
    comment_kinds: &["comment"],
    identifier_kinds: &[
        "identifier",
        "property_identifier",
        "shorthand_property_identifier",
        "type_identifier",
    ],
    control_flow_kinds: &[
        "if_statement",
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
        "switch_case",
        "switch_default",
        "catch_clause",
        "ternary_expression",
    ],
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &["jsx_element", "jsx_self_closing_element"],
    no_descend_kinds: &["string", "template_string"],
    parameter_fields: &["parameters"],
    dynamic_calls: &["eval"],
    docstring: false,
    import_fn: js_imports,
    callee_fn: |node, text| field_text(node, "function", text),
    name_fn: None,
    parent_class_fn: None,
    bases_fn: Some(js_bases),
    exported_fn: js_exported,
    post_fn: None,
};

pub static SPEC: LangSpec = TS_BASE;

/// TSX grammar variant of the TypeScript spec.
const TSX_BASE: LangSpec = LangSpec {
    language: "TSX",
    grammar: || tree_sitter_typescript::LANGUAGE_TSX.into(),
    ..TS_BASE
};

pub static SPEC_TSX: LangSpec = TSX_BASE;
