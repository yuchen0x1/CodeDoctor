use super::*;

fn file(language: &str, path: &str) -> FileRecord {
    FileRecord {
        id: "f".into(),
        scan_run_id: "run".into(),
        project_id: "p".into(),
        relative_path: path.into(),
        absolute_path: path.into(),
        encoding: "utf-8".into(),
        size_bytes: 0,
        content_hash: String::new(),
        language: language.into(),
        parse_capability: CoverageKind::NotCovered,
        excluded_reason: None,
    }
}

#[test]
fn python_ast_extracts_structures_ranges_and_metrics() {
    let text = include_str!("../../../fixtures/parser/metrics.py");
    let parsed = ParserRegistry::parse(&file("Python", "metrics.py"), text);
    assert_eq!(parsed.coverage, CoverageKind::Ast);
    let function = parsed
        .functions
        .iter()
        .find(|f| f.name == "calculate")
        .unwrap();
    assert_eq!(function.range.start_line, 5);
    assert_eq!(function.range.end_line, 10);
    assert_eq!(function.metrics.length, Some(6));
    assert_eq!(function.metrics.parameter_count, Some(2));
    assert_eq!(function.metrics.branch_count, Some(2));
    assert_eq!(function.metrics.nesting_depth, Some(2));
    assert_eq!(function.metrics.cyclomatic_complexity, Some(3));
    assert_eq!(parsed.imports.len(), 2);
    assert_eq!(parsed.imports[1].module, "pathlib");
    assert_eq!(parsed.imports[1].names[0].alias.as_deref(), Some("P"));
    assert_eq!(parsed.comments[0].range.start_line, 4);
    assert_eq!(parsed.classes[0].bases, vec!["Base"]);
    assert_eq!(parsed.classes[0].method_count, Some(1));
    assert!(parsed.calls.iter().any(|call| call.callee == "P"));
}

#[test]
fn typescript_class_uses_real_grammar_fields() {
    let parsed = ParserRegistry::parse(
        &file("TypeScript", "metrics.ts"),
        include_str!("../../../fixtures/parser/metrics.ts"),
    );
    assert_eq!(parsed.coverage, CoverageKind::Ast);
    assert_eq!(parsed.classes[0].name, "Service");
    assert_eq!(parsed.classes[0].bases, vec!["Base"]);
    assert_eq!(parsed.classes[0].method_count, Some(1));
    assert_eq!(parsed.classes[0].property_count, Some(1));
    let function = &parsed.functions[0];
    assert_eq!(function.name, "run");
    assert_eq!(function.range.start_line, 4);
    assert_eq!(function.metrics.parameter_count, Some(2));
    assert_eq!(function.parent_class.as_deref(), Some("Service"));
    assert_eq!(parsed.imports[0].module, "./store");
    assert_eq!(parsed.imports[0].names[0].alias.as_deref(), Some("persist"));
}

#[test]
fn javascript_import_arrow_and_dynamic_import_are_distinct() {
    let parsed = ParserRegistry::parse(
        &file("JavaScript", "metrics.js"),
        include_str!("../../../fixtures/parser/metrics.js"),
    );
    assert_eq!(parsed.imports[0].module, "node:fs");
    assert_eq!(parsed.imports[0].names[0].name, "fs");
    assert!(
        parsed
            .functions
            .iter()
            .any(|f| f.name == "transform" && f.metrics.parameter_count == Some(2))
    );
    assert!(
        parsed
            .imports
            .iter()
            .any(|i| i.dynamic && i.module == "./lazy.js")
    );
    assert!(
        parsed
            .functions
            .iter()
            .any(|f| f.name == "execute" && f.is_exported)
    );
}

#[test]
fn malformed_source_keeps_diagnostics_and_marks_partial_coverage() {
    let parsed =
        ParserRegistry::parse(&file("Python", "broken.py"), "def broken(:\n    return 1\n");
    assert!(!parsed.diagnostics.is_empty());
    assert_eq!(parsed.coverage, CoverageKind::Structural);
}

#[test]
fn unsupported_language_never_claims_zero_unknown_metrics() {
    // PowerShell has no grammar registered; the Rust fixture language was
    // upgraded to a tree-sitter grammar, so an actually-unsupported language
    // is used to keep the original coverage-honesty contract.
    let parsed = ParserRegistry::parse(&file("PowerShell", "run.ps1"), "function Main {}\n");
    assert_eq!(parsed.coverage, CoverageKind::Lexical);
    assert_eq!(parsed.metrics.function_count, None);
    assert_eq!(parsed.metrics.import_count, None);
    assert!(parsed.diagnostics.iter().any(|d| d.kind == "coverage"));
}

#[test]
fn vue_script_ranges_map_back_to_original_utf8_source() {
    let text = "<template>中文</template>\n<script setup lang=\"ts\">\nconst add = (值: number) => 值 + 1;\n</script>\n<style>.x { color: red; }</style>";
    let parsed = ParserRegistry::parse(&file("Vue", "Example.vue"), text);
    assert_eq!(parsed.coverage, CoverageKind::Structural);
    assert_eq!(parsed.functions[0].name, "add");
    assert_eq!(parsed.functions[0].range.start_line, 3);
    assert_eq!(
        &text[parsed.functions[0].range.start_byte..parsed.functions[0].range.end_byte],
        "(值: number) => 值 + 1"
    );
    assert!(parsed.diagnostics.iter().any(|d| d.kind == "coverage"));
}

#[test]
fn tsx_extracts_component_references_without_string_matches() {
    let parsed = ParserRegistry::parse(
        &file("TSX", "View.tsx"),
        "export function View() { return <Panel><Button /></Panel>; }",
    );
    assert_eq!(parsed.coverage, CoverageKind::Ast);
    assert!(
        parsed
            .component_references
            .iter()
            .any(|r| r.target == "Panel")
    );
    assert!(
        parsed
            .component_references
            .iter()
            .any(|r| r.target == "Button")
    );
}
