//! Python grammar facts (tree-sitter-python).

use super::common::clamp_text;
use super::{LangSpec, Node, field_text};
use crate::parse::{ImportBinding, ImportInfo};

fn string_content(node: Node, text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string_content" {
            return child.utf8_text(bytes).ok().map(str::to_owned);
        }
    }
    None
}

fn python_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mk = |module: &str, names: Vec<ImportBinding>, dynamic: bool| ImportInfo {
        module: module.to_owned(),
        names,
        range: Default::default(),
        dynamic,
    };
    match node.kind() {
        "import_statement" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "dotted_name" => {
                        let module = child.utf8_text(bytes).unwrap_or("");
                        out.push(mk(module, vec![], false));
                    }
                    "aliased_import" => {
                        let module = field_text(child, "name", text);
                        let alias = field_text(child, "alias", text);
                        let names = if alias.is_empty() {
                            vec![]
                        } else {
                            vec![ImportBinding {
                                name: module.clone(),
                                alias: Some(alias),
                            }]
                        };
                        out.push(mk(&module, names, false));
                    }
                    _ => {}
                }
            }
        }
        "import_from_statement" => {
            let module = node
                .child_by_field_name("module_name")
                .and_then(|n| n.utf8_text(bytes).ok())
                .unwrap_or(".")
                .to_owned();
            let mut names = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "aliased_import" => {
                        let name = field_text(child, "name", text);
                        let alias = field_text(child, "alias", text);
                        names.push(ImportBinding {
                            name,
                            alias: if alias.is_empty() { None } else { Some(alias) },
                        });
                    }
                    "dotted_name" => {
                        let name = child.utf8_text(bytes).unwrap_or("").to_owned();
                        if name != module {
                            names.push(ImportBinding { name, alias: None });
                        }
                    }
                    _ => {}
                }
            }
            out.push(mk(&module, names, false));
        }
        "call_expression" => {
            let callee = field_text(node, "function", text);
            let last = callee.rsplit('.').next().unwrap_or("");
            if last == "__import__" || callee.ends_with("import_module") {
                let module = node
                    .child_by_field_name("arguments")
                    .and_then(|args| args.named_child(0))
                    .filter(|n| n.kind() == "string")
                    .and_then(|s| string_content(s, text))
                    .map(|m| clamp_text(&m, 200))
                    .unwrap_or_else(|| "<dynamic>".to_owned());
                out.push(mk(&module, vec![], true));
            }
        }
        _ => {}
    }
    out
}

fn python_bases(node: Node, text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    node.child_by_field_name("superclasses")
        .map(|list| {
            let mut cursor = list.walk();
            list.children(&mut cursor)
                .filter(|n| matches!(n.kind(), "identifier" | "attribute" | "subscript" | "call"))
                .map(|n| clamp_text(n.utf8_text(bytes).unwrap_or(""), 80))
                .collect()
        })
        .unwrap_or_default()
}

pub static SPEC: LangSpec = LangSpec {
    language: "Python",
    grammar: || tree_sitter_python::LANGUAGE.into(),
    grammar_version: "0.23",
    function_kinds: &["function_definition"],
    arrow_kinds: &[],
    class_kinds: &["class_definition"],
    impl_kinds: &[],
    property_kinds: &["assignment", "annotated_assignment"],
    import_kinds: &[
        "import_statement",
        "import_from_statement",
        "call_expression",
    ],
    call_kinds: &["call"],
    comment_kinds: &["comment"],
    identifier_kinds: &["identifier"],
    control_flow_kinds: &[
        "if_statement",
        "elif_clause",
        "for_statement",
        "async_for_statement",
        "while_statement",
        "except_clause",
        "case_clause",
        "conditional_expression",
    ],
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &[],
    no_descend_kinds: &["string"],
    parameter_fields: &["parameters"],
    dynamic_calls: &[
        "eval",
        "exec",
        "getattr",
        "setattr",
        "__import__",
        "import_module",
    ],
    docstring: true,
    import_fn: python_imports,
    callee_fn: |node, text| field_text(node, "function", text),
    name_fn: None,
    parent_class_fn: None,
    bases_fn: Some(python_bases),
    exported_fn: |_, _| false,
    post_fn: None,
};
