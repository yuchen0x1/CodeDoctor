//! C and C++ grammar facts (tree-sitter-c, tree-sitter-cpp).

use super::common::clamp_text;
use super::{LangSpec, Node};
use crate::parse::ImportInfo;

fn c_imports(node: Node, text: &str) -> Vec<ImportInfo> {
    let bytes = text.as_bytes();
    let module = node
        .child_by_field_name("path")
        .and_then(|n| n.utf8_text(bytes).ok())
        .map(|raw| {
            let t = raw.trim();
            let t = t.trim_start_matches('<').trim_start_matches('"');
            let t = t.trim_end_matches('>').trim_end_matches('"');
            clamp_text(t, 200)
        })
        .unwrap_or_default();
    if module.is_empty() {
        return vec![];
    }
    vec![ImportInfo {
        module,
        names: vec![],
        range: Default::default(),
        dynamic: false,
    }]
}

/// C/C++ functions carry their name inside a declarator chain, not a `name`
/// field: `function_definition → declarator(function_declarator(declarator
/// (identifier)))`, possibly wrapped by pointer/array declarators.
fn c_name(node: Node, text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut current = node.child_by_field_name("declarator")?;
    for _ in 0..8 {
        match current.kind() {
            "identifier" | "field_identifier" | "destructor_name" | "operator_name" => {
                return current.utf8_text(bytes).ok().map(|t| clamp_text(t, 80));
            }
            "function_declarator"
            | "pointer_declarator"
            | "reference_declarator"
            | "array_declarator"
            | "parenthesized_declarator"
            | "init_declarator" => {
                let next = current
                    .child_by_field_name("declarator")
                    .or_else(|| current.child_by_field_name("base"));
                match next {
                    Some(n) => current = n,
                    None => return None,
                }
            }
            "template_function" => {
                let next = current.child_by_field_name("name");
                match next {
                    Some(n) => current = n,
                    None => return None,
                }
            }
            _ => return None,
        }
    }
    None
}

fn cpp_bases(node: Node, text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut bases = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "base_class_clause" {
            let mut inner = child.walk();
            for base in child.children(&mut inner) {
                if matches!(
                    base.kind(),
                    "type_identifier" | "qualified_identifier" | "template_type"
                ) {
                    let raw = base.utf8_text(bytes).unwrap_or("");
                    let base = raw.split('<').next().unwrap_or(raw).trim();
                    bases.push(clamp_text(base, 80));
                }
            }
        }
    }
    bases
}

const C_CONTROL: &[&str] = &[
    "if_statement",
    "for_statement",
    "while_statement",
    "do_statement",
    "case_statement",
    "conditional_expression",
];

const C_IDENTIFIERS: &[&str] = &["identifier", "field_identifier", "type_identifier"];

const C_COMMENTS: &[&str] = &["comment"];

const C_NO_DESCEND: &[&str] = &["string_literal", "char_literal"];

pub const C_BASE: LangSpec = LangSpec {
    language: "C",
    grammar: || tree_sitter_c::LANGUAGE.into(),
    grammar_version: "0.23",
    function_kinds: &["function_definition"],
    arrow_kinds: &[],
    class_kinds: &[],
    impl_kinds: &[],
    property_kinds: &[],
    import_kinds: &["preproc_include"],
    call_kinds: &["call_expression"],
    comment_kinds: C_COMMENTS,
    identifier_kinds: C_IDENTIFIERS,
    control_flow_kinds: C_CONTROL,
    case_arm_kinds: &[],
    ternary_kinds: &[],
    jsx_kinds: &[],
    no_descend_kinds: C_NO_DESCEND,
    parameter_fields: &["parameters"],
    dynamic_calls: &super::C_DYNAMIC_CALLS,
    docstring: false,
    import_fn: c_imports,
    callee_fn: |node, text| super::field_text(node, "function", text),
    name_fn: Some(c_name),
    parent_class_fn: None,
    bases_fn: None,
    exported_fn: |_, _| false,
    post_fn: None,
};

pub static C_SPEC: LangSpec = C_BASE;

const CPP_BASE: LangSpec = LangSpec {
    language: "C++",
    grammar: || tree_sitter_cpp::LANGUAGE.into(),
    function_kinds: &["function_definition", "method_definition"],
    class_kinds: &["class_specifier", "struct_specifier"],
    property_kinds: &["field_declaration"],
    call_kinds: &["call_expression"],
    control_flow_kinds: C_CONTROL,
    bases_fn: Some(cpp_bases),
    ..C_BASE
};

pub static CPP_SPEC: LangSpec = CPP_BASE;
