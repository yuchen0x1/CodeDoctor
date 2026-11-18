//! Per-language grammar facts. Every grammar-backed language declares its
//! tree-sitter node kinds and extraction overrides here; the shared engine in
//! `crate::parse::tree_sitter` walks the syntax tree driven by this spec.
//!
//! Coverage policy: a language gets `Ast` only from these grammars. Languages
//! without an entry fall back to lexical parsing and must never report
//! parser-derived metrics.
pub mod c_family;
pub mod common;
pub mod csharp;
pub mod go;
pub mod java;
pub mod javascript;
pub mod php;
pub mod python;
pub mod rust_lang;
pub mod typescript;

use crate::parse::{ClassInfo, FunctionInfo, ImportInfo};

pub type Node<'a> = tree_sitter::Node<'a>;

/// Declarative description of one grammar-backed language.
pub struct LangSpec {
    /// CodeDoctor language name (matches `index::language`).
    pub language: &'static str,
    pub grammar: fn() -> tree_sitter::Language,
    pub grammar_version: &'static str,
    /// Node kinds that create callable units (named or arrow).
    pub function_kinds: &'static [&'static str],
    /// Subset of `function_kinds` whose name must be recovered from the
    /// surrounding assignment/property instead of a `name` field.
    pub arrow_kinds: &'static [&'static str],
    pub class_kinds: &'static [&'static str],
    /// Kinds that add implementation context (e.g. Rust `impl` blocks) whose
    /// members should carry the type name as `parent_class`.
    pub impl_kinds: &'static [&'static str],
    pub property_kinds: &'static [&'static str],
    pub import_kinds: &'static [&'static str],
    pub call_kinds: &'static [&'static str],
    pub comment_kinds: &'static [&'static str],
    pub identifier_kinds: &'static [&'static str],
    pub control_flow_kinds: &'static [&'static str],
    /// Switch/select arm kinds counted as one decision each.
    pub case_arm_kinds: &'static [&'static str],
    /// Ternary-like conditional expression kinds.
    pub ternary_kinds: &'static [&'static str],
    /// JSX-like element kinds producing component references.
    pub jsx_kinds: &'static [&'static str],
    /// Kinds whose subtree is never descended into (strings, macros).
    pub no_descend_kinds: &'static [&'static str],
    /// Field names that may hold the parameter container.
    pub parameter_fields: &'static [&'static str],
    /// Callee names considered runtime-dispatched (reflection/eval family).
    pub dynamic_calls: &'static [&'static str],
    /// Whether the first string statement of a body is a docstring.
    pub docstring: bool,
    pub import_fn: fn(Node, &str) -> Vec<ImportInfo>,
    pub callee_fn: fn(Node, &str) -> String,
    /// Resolves the declared name of a function node (C-style declarators).
    pub name_fn: Option<fn(Node, &str) -> Option<String>>,
    /// Resolves a receiver/owner class for methods (Go style receivers).
    pub parent_class_fn: Option<fn(Node, &str) -> Option<String>>,
    pub bases_fn: Option<fn(Node, &str) -> Vec<String>>,
    pub exported_fn: fn(Node, &str) -> bool,
    /// Optional post-pass adjusting class method counts (Rust impl blocks).
    pub post_fn: Option<fn(&mut [ClassInfo], &[FunctionInfo])>,
}

impl LangSpec {
    pub fn is_control_kind(&self, kind: &str) -> bool {
        self.control_flow_kinds.contains(&kind)
            || self.case_arm_kinds.contains(&kind)
            || self.ternary_kinds.contains(&kind)
    }

    pub fn is_function_kind(&self, kind: &str) -> bool {
        self.function_kinds.contains(&kind)
    }
}

/// Logical operator tokens recognized by the shared engine on
/// `binary_expression`/`binary_operator` nodes.
pub const LOGICAL_OPERATORS: &[&str] = &["&&", "||", "and", "or", "??"];

pub fn is_binary_kind(kind: &str) -> bool {
    kind == "binary_expression" || kind == "binary_operator"
}

pub fn logical_kind(operator: &str) -> Option<&'static str> {
    match operator {
        "&&" | "and" => Some("logical_and"),
        "||" | "or" => Some("logical_or"),
        "??" => Some("null_coalesce"),
        _ => None,
    }
}

/// Returns the grammar spec for a CodeDoctor language name, if covered.
pub fn spec_for(language: &str) -> Option<&'static LangSpec> {
    match language {
        "Python" => Some(&python::SPEC),
        "JavaScript" | "JSX" => Some(&javascript::SPEC),
        "TypeScript" => Some(&typescript::SPEC),
        "TSX" => Some(&typescript::SPEC_TSX),
        "Java" => Some(&java::SPEC),
        "Go" => Some(&go::SPEC),
        "Rust" => Some(&rust_lang::SPEC),
        "C" => Some(&c_family::C_SPEC),
        "C++" => Some(&c_family::CPP_SPEC),
        "C#" => Some(&csharp::SPEC),
        "PHP" => Some(&php::SPEC),
        _ => None,
    }
}

/// Languages parsed by partitioning embedded script blocks.
pub fn is_composite(language: &str) -> bool {
    matches!(language, "Vue" | "Svelte" | "HTML")
}

/// Reads a node field as clamped text.
pub fn field_text(node: Node, field: &str, text: &str) -> String {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(text.as_bytes()).ok())
        .map(|t| common::clamp_text(t, 80))
        .unwrap_or_default()
}

/// C-compatible dynamic dispatch callee names shared by several specs.
pub const C_DYNAMIC_CALLS: &[&str] = &["dlsym", "system", "popen", "execl", "fork"];
