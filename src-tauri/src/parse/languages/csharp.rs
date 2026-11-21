//! C# grammar facts (tree-sitter-c-sharp).

use super::{LangSpec, Node, field_text};
use crate::parse::{ImportBinding, ImportInfo};

fn csharp_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let bytes = text.as_bytes();
    let module = field_text(node, "name", text);
    let module = if module.is_empty() {
        // Some tree-sitter-c-sharp builds expose the target as a bare
        // identifier/qualified_name child instead of a `name` field.
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| matches!(c.kind(), "identifier" | "qualified_name"))
            .and_then(|n| n.utf8_text(bytes).ok())
            .map(|t| t.to_owned())
            .unwrap_or_default()
    } else {
        module
    };
    if module.is_empty() {
        return vec![];
    }
    let (module, name) = match module.rsplit_once('.') {
        Some((m, n)) if n.chars().next().is_some_and(|c| c.is_uppercase()) => {
            (m.to_owned(), n.to_owned())
        }
        _ => (module, String::new()),
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
        binding_range: None,
        dynamic: false,
    }]
}

fn csharp_bases(node: Node, text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut bases = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "base_list" {
            let mut inner = child.walk();
            for base in child.children(&mut inner) {
                match base.kind() {
                    // Runtime nodes are plain identifier/qualified_name forms.
                    "identifier" | "qualified_name" | "generic_name" => {
                        let raw = base.utf8_text(bytes).unwrap_or("");
                        let clean = raw.split('<').next().unwrap_or(raw).trim();
                        bases.push(clean.trim_start_matches(':').trim().to_owned());
                    }
                    "primary_constructor_base_type" => {
                        if let Some(ty) = base.child_by_field_name("type") {
                            let raw = ty.utf8_text(bytes).unwrap_or("");
                            let clean = raw.split('<').next().unwrap_or(raw).trim();
                            bases.push(clean.to_owned());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    bases.retain(|b| !b.is_empty());
    bases
}

fn csharp_callee(node: Node, text: &str) -> String {
    if node.kind() == "object_creation_expression" {
        let ty = field_text(node, "type", text);
        return format!("new {ty}");
    }
    // invocation_expression exposes its target through the `function` field.
    let via_function = field_text(node, "function", text);
    if !via_function.is_empty() {
        return via_function;
    }
    field_text(node, "expression", text)
}

fn csharp_exported(node: Node, _text: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifier" {
            if child
                .utf8_text(_text.as_bytes())
                .map(|t| t == "public")
                .unwrap_or(false)
            {
                return true;
            }
        } else if child.kind() == "modifiers" {
            return child
                .utf8_text(_text.as_bytes())
                .map(|t| t.split_whitespace().any(|m| m == "public"))
                .unwrap_or(false);
        }
    }
    false
}

pub static SPEC: LangSpec = LangSpec {
    language: "C#",
    grammar: || tree_sitter_c_sharp::LANGUAGE.into(),
    grammar_version: "0.23",
    function_kinds: &[
        "method_declaration",
        "constructor_declaration",
        "local_function_statement",
    ],
    arrow_kinds: &[],
    class_kinds: &[
        "class_declaration",
        "interface_declaration",
        "struct_declaration",
        "record_declaration",
        "enum_declaration",
    ],
    impl_kinds: &[],
    property_kinds: &["property_declaration", "field_declaration"],
    import_kinds: &["using_directive"],
    call_kinds: &["invocation_expression", "object_creation_expression"],
    comment_kinds: &["comment"],
    identifier_kinds: &["identifier"],
    control_flow_kinds: &[
        "if_statement",
        "for_statement",
        "for_each_statement",
        "while_statement",
        "do_statement",
        "switch_section",
        "catch_clause",
        "conditional_expression",
    ],
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &[],
    no_descend_kinds: &["string_literal"],
    parameter_fields: &["parameters"],
    dynamic_calls: &["GetType", "CreateInstance", "Invoke", "GetMethod"],
    docstring: false,
    import_fn: csharp_imports,
    callee_fn: csharp_callee,
    name_fn: None,
    parent_class_fn: None,
    bases_fn: Some(csharp_bases),
    exported_fn: csharp_exported,
    post_fn: None,
};
