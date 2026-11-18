//! JavaScript grammar facts (tree-sitter-javascript). Also serves JSX and the
//! script blocks embedded in composite files (Vue/Svelte/HTML).

use super::common::{clamp_text, unquote};
use super::{LangSpec, Node, field_text};
use crate::parse::{ImportBinding, ImportInfo};

/// Import/re-export/dynamic-import extraction shared with TypeScript.
pub fn js_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut mk = |module: String, names: Vec<ImportBinding>, dynamic: bool| {
        out.push(ImportInfo {
            module,
            names,
            range: Default::default(),
            dynamic,
        })
    };
    match node.kind() {
        "import_statement" => {
            let module = node
                .child_by_field_name("source")
                .and_then(|n| n.utf8_text(bytes).ok())
                .map(unquote)
                .unwrap_or_default();
            let mut names = Vec::new();
            let mut clause_cursor = node.walk();
            let clause = node
                .children(&mut clause_cursor)
                .find(|c| c.kind() == "import_clause");
            if let Some(clause) = clause {
                let mut cursor = clause.walk();
                for child in clause.children(&mut cursor) {
                    match child.kind() {
                        "identifier" => {
                            let name = child.utf8_text(bytes).unwrap_or("").to_owned();
                            names.push(ImportBinding { name, alias: None });
                        }
                        "named_imports" => {
                            let mut inner = child.walk();
                            for spec in child.children(&mut inner) {
                                if spec.kind() == "import_specifier" {
                                    let name = field_text(spec, "name", text);
                                    let alias = field_text(spec, "alias", text);
                                    names.push(ImportBinding {
                                        name,
                                        alias: if alias.is_empty() { None } else { Some(alias) },
                                    });
                                }
                            }
                        }
                        "namespace_import" => {
                            let name = field_text(child, "name", text);
                            names.push(ImportBinding { name, alias: None });
                        }
                        _ => {}
                    }
                }
            }
            mk(module, names, false);
        }
        "export_statement" => {
            if node.child_by_field_name("source").is_some() {
                let module = node
                    .child_by_field_name("source")
                    .and_then(|n| n.utf8_text(bytes).ok())
                    .map(unquote)
                    .unwrap_or_default();
                let mut names = Vec::new();
                if let Some(clause) = node.child_by_field_name("export_clause") {
                    let mut inner = clause.walk();
                    for spec in clause.children(&mut inner) {
                        if spec.kind() == "export_specifier" {
                            let name = field_text(spec, "name", text);
                            let alias = field_text(spec, "alias", text);
                            names.push(ImportBinding {
                                name,
                                alias: if alias.is_empty() { None } else { Some(alias) },
                            });
                        }
                    }
                }
                mk(module, names, false);
            }
        }
        "call_expression" => {
            let function = node.child_by_field_name("function");
            let kind = function.map(|n| n.kind()).unwrap_or("");
            let callee = function
                .and_then(|n| n.utf8_text(bytes).ok())
                .unwrap_or("")
                .to_owned();
            if kind == "import" || callee == "require" {
                let module = node
                    .child_by_field_name("arguments")
                    .and_then(|args| args.named_child(0))
                    .filter(|n| n.kind() == "string")
                    .and_then(|s| s.utf8_text(bytes).ok())
                    .map(unquote)
                    .map(|m| clamp_text(&m, 200))
                    .unwrap_or_else(|| "<dynamic>".to_owned());
                mk(module, vec![], kind == "import");
            }
        }
        _ => {}
    }
    out
}

pub fn js_exported(node: Node, _text: &str) -> bool {
    let mut parent = node.parent();
    for _ in 0..3 {
        match parent {
            Some(p) if p.kind() == "export_statement" => return true,
            Some(p) => parent = p.parent(),
            None => break,
        }
    }
    false
}

pub static SPEC: LangSpec = LangSpec {
    language: "JavaScript",
    grammar: || tree_sitter_javascript::LANGUAGE.into(),
    grammar_version: "0.23",
    function_kinds: &[
        "function_declaration",
        "generator_function_declaration",
        "method_definition",
        "function",
        "arrow_function",
    ],
    arrow_kinds: &["function", "arrow_function"],
    class_kinds: &["class_declaration", "class"],
    impl_kinds: &[],
    property_kinds: &["field_definition"],
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
    bases_fn: Some(crate::parse::languages::typescript::js_bases),
    exported_fn: js_exported,
    post_fn: None,
};
