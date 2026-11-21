//! Rust grammar facts (tree-sitter-rust).

use super::common::clamp_text;
use super::{LangSpec, Node, field_text};
use crate::parse::{ClassInfo, FunctionInfo, ImportBinding, ImportInfo};

fn rust_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let argument = match node.child_by_field_name("argument") {
        Some(a) => a,
        None => return vec![],
    };
    let raw = argument.utf8_text(text.as_bytes()).unwrap_or("");
    let raw = raw.trim();
    let mut names = Vec::new();
    let module;
    if let Some(brace) = raw.find('{') {
        module = clamp_text(raw[..brace].trim_end_matches("::"), 120);
        let inner = raw[brace..].trim_start_matches('{').trim_end_matches('}');
        for part in inner.split(',') {
            let part = part.trim();
            if part == "self" {
                names.push(ImportBinding {
                    name: "self".to_owned(),
                    alias: None,
                });
            } else if let Some((name, alias)) = part.split_once(" as ") {
                names.push(ImportBinding {
                    name: clamp_text(name.trim(), 80),
                    alias: Some(clamp_text(alias.trim(), 80)),
                });
            } else if part != "*" && !part.is_empty() {
                names.push(ImportBinding {
                    name: clamp_text(part, 80),
                    alias: None,
                });
            }
        }
    } else if raw.contains(" as ") {
        let (path, alias) = raw.split_once(" as ").unwrap();
        let path = path.trim();
        match path.rsplit_once("::") {
            Some((m, n)) => {
                module = clamp_text(m, 120);
                names.push(ImportBinding {
                    name: clamp_text(n, 80),
                    alias: Some(clamp_text(alias.trim(), 80)),
                });
            }
            None => {
                module = clamp_text(path, 120);
                names.push(ImportBinding {
                    name: clamp_text(alias.trim(), 80),
                    alias: None,
                });
            }
        }
    } else {
        match raw.rsplit_once("::") {
            Some((m, n)) => {
                module = clamp_text(m, 120);
                names.push(ImportBinding {
                    name: clamp_text(n, 80),
                    alias: None,
                });
            }
            None => {
                module = clamp_text(raw, 120);
            }
        }
    }
    vec![ImportInfo {
        module,
        names,
        range: Default::default(),
        binding_range: None,
        dynamic: false,
    }]
}

fn rust_callee(node: Node, text: &str) -> String {
    if node.kind() == "macro_invocation" {
        let name = field_text(node, "macro", text);
        return format!("{name}!");
    }
    field_text(node, "function", text)
}

fn rust_exported(node: Node, _text: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return true;
        }
    }
    false
}

/// Aggregates `impl` block methods onto the matching struct/enum/trait.
fn rust_post(classes: &mut [ClassInfo], functions: &[FunctionInfo]) {
    for class in classes.iter_mut() {
        let count = functions
            .iter()
            .filter(|f| f.parent_class.as_deref() == Some(class.name.as_str()))
            .count();
        if count > 0 {
            class.method_count = Some(count as u32);
        }
    }
}

pub static SPEC: LangSpec = LangSpec {
    language: "Rust",
    grammar: || tree_sitter_rust::LANGUAGE.into(),
    grammar_version: "0.23",
    function_kinds: &["function_item", "function_signature_item"],
    arrow_kinds: &[],
    class_kinds: &["struct_item", "enum_item", "trait_item"],
    impl_kinds: &["impl_item"],
    property_kinds: &["field_declaration", "enum_variant"],
    import_kinds: &["use_declaration"],
    call_kinds: &["call_expression", "macro_invocation"],
    comment_kinds: &["line_comment", "block_comment"],
    identifier_kinds: &["identifier", "field_identifier", "type_identifier"],
    control_flow_kinds: &[
        "if_expression",
        "if_let_expression",
        "while_expression",
        "while_let_expression",
        "for_expression",
        "loop_expression",
        "match_arm",
    ],
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &[],
    no_descend_kinds: &["string_literal", "macro_invocation"],
    parameter_fields: &["parameters"],
    dynamic_calls: &[],
    docstring: false,
    import_fn: rust_imports,
    callee_fn: rust_callee,
    name_fn: None,
    parent_class_fn: None,
    bases_fn: None,
    exported_fn: rust_exported,
    post_fn: Some(rust_post),
};
