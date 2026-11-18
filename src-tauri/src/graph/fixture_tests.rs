//! Fixture-backed integration tests for the dependency graph.

use super::build;
use crate::domain::types::{CoverageKind, FileRecord};
use crate::parse::ParsedFile;

fn parsed(language: &str, path: &str, text: &str) -> ParsedFile {
    let file = FileRecord {
        id: path.into(),
        scan_run_id: "run".into(),
        project_id: "p".into(),
        relative_path: path.into(),
        absolute_path: path.into(),
        encoding: "utf-8".into(),
        size_bytes: 0,
        content_hash: String::new(),
        language: language.into(),
        parse_capability: CoverageKind::Ast,
        excluded_reason: None,
    };
    let parse = crate::parse::ParserRegistry::parse(&file, text);
    ParsedFile {
        file,
        text: text.to_owned(),
        parse,
    }
}

#[test]
fn python_import_cycle_is_detected_as_one_scc() {
    let files = vec![
        parsed(
            "Python",
            "pkg/a.py",
            include_str!("../../../fixtures/architecture/a.py"),
        ),
        parsed(
            "Python",
            "pkg/b.py",
            include_str!("../../../fixtures/architecture/b.py"),
        ),
        parsed(
            "Python",
            "pkg/c.py",
            include_str!("../../../fixtures/architecture/c.py"),
        ),
    ];
    let snapshot = build(&files);
    assert_eq!(snapshot.cycles().len(), 1, "{:?}", snapshot.cycles);
    assert_eq!(snapshot.cycles()[0].paths.len(), 3);
    let issues = snapshot.issues();
    assert!(
        issues
            .iter()
            .any(|entry| entry.draft.rule_id == "architecture.cycle"
                && entry.path == "pkg/a.py"
                && entry.draft.start_line == 1
                && entry.draft.evidence.contains("pkg/a.py"))
    );
}

#[test]
fn unresolved_dynamic_import_flags_incomplete_coverage() {
    let files = vec![
        parsed(
            "JavaScript",
            "src/dynamic.js",
            include_str!("../../../fixtures/architecture/dynamic.js"),
        ),
        parsed(
            "JavaScript",
            "src/present.js",
            include_str!("../../../fixtures/architecture/present.js"),
        ),
    ];
    let snapshot = build(&files);
    assert!(snapshot.incomplete_coverage);
    assert!(
        snapshot
            .unresolved_imports
            .iter()
            .any(|u| u.contains("missing.js"))
    );
    assert!(
        snapshot
            .edges
            .iter()
            .any(|e| e.target == "src/present.js" && e.source == "src/dynamic.js")
    );
}

#[test]
fn high_fan_in_hotspot_is_reported() {
    let mut files = vec![parsed(
        "Python",
        "pkg/hub.py",
        include_str!("../../../fixtures/architecture/hub.py"),
    )];
    for i in 0..13 {
        let name = format!("pkg/user_{i}.py");
        let text = format!("import hub\n\ndef use_{i}():\n    return hub.shared_value() + {i}\n");
        files.push(parsed("Python", &name, &text));
    }
    let snapshot = build(&files);
    let hub = snapshot
        .nodes
        .iter()
        .find(|n| n.path == "pkg/hub.py")
        .unwrap();
    assert_eq!(hub.fan_in, 13);
    assert!(
        snapshot
            .issues()
            .iter()
            .any(|entry| entry.draft.rule_id == "architecture.hotspot"
                && entry.path == "pkg/user_0.py"
                && entry.draft.evidence.contains("pkg/hub.py"))
    );
}

#[test]
fn high_fan_out_module_is_reported() {
    let mut files = Vec::new();
    for i in 0..13 {
        files.push(parsed(
            "Python",
            &format!("pkg/lib_{i}.py"),
            "def f():\n    return 1\n",
        ));
    }
    let mut importer_text = String::new();
    for i in 0..13 {
        importer_text.push_str(&format!("import lib_{i}\n"));
    }
    importer_text.push_str("def run():\n    return 1\n");
    files.push(parsed("Python", "pkg/importer.py", &importer_text));
    let snapshot = build(&files);
    let importer = snapshot
        .nodes
        .iter()
        .find(|n| n.path == "pkg/importer.py")
        .unwrap();
    assert_eq!(importer.fan_out, 13);
    assert!(
        snapshot
            .issues()
            .iter()
            .any(|entry| entry.draft.rule_id == "architecture.high-fan-out"
                && entry.path == "pkg/importer.py")
    );
}

#[test]
fn acyclic_pair_has_no_cycle_and_complete_coverage() {
    let files = vec![
        parsed("Python", "pkg/base.py", "def base():\n    return 1\n"),
        parsed(
            "Python",
            "pkg/app.py",
            "import base\n\ndef app():\n    return base.base()\n",
        ),
    ];
    let snapshot = build(&files);
    assert!(snapshot.cycles().is_empty());
    assert!(!snapshot.incomplete_coverage);
    assert!(
        snapshot
            .issues()
            .iter()
            .all(|entry| entry.draft.rule_id != "architecture.cycle")
    );
}

#[test]
fn tsx_references_need_import_bindings_and_preserve_kinds_and_ranges() {
    let files = [
        parsed(
            "TSX",
            "src/view/View.tsx",
            "import { Base as Parent, Widget, helper } from '../lib/source';\n\
             import { OnlyImported } from '../lib/OnlyImported';\n\
             export class Page extends Parent {}\n\
             export function view() { helper(); return <Widget />; }\n\
             const label = 'OnlyImported()';",
        ),
        parsed(
            "TypeScript",
            "src/lib/source.ts",
            "export class Base {}\nexport function Widget() {}\nexport function helper() {}",
        ),
        parsed(
            "TypeScript",
            "src/lib/OnlyImported.ts",
            "export class OnlyImported {}",
        ),
    ];

    let graph = build(&files);
    let saved = graph
        .edges
        .iter()
        .map(|edge| {
            (
                edge.target.as_str(),
                edge.kind.as_str(),
                edge.range.start_line,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        saved,
        [
            ("src/lib/OnlyImported.ts", "import", 2),
            ("src/lib/source.ts", "call", 4),
            ("src/lib/source.ts", "component", 4),
            ("src/lib/source.ts", "import", 1),
            ("src/lib/source.ts", "inheritance", 3),
        ]
    );
    assert_eq!(graph.nodes[0].fan_out, 2);
    assert_eq!(graph.nodes[1].fan_in, 1);
}

#[test]
fn relative_imports_resolve_from_the_actual_directory() {
    let files = [
        parsed(
            "JavaScript",
            "src/view/main.js",
            "import '../shared/util.js';\nimport './util.js';\nimport 'Widget';",
        ),
        parsed(
            "JavaScript",
            "src/shared/util.js",
            "export const value = 1;",
        ),
        parsed(
            "JavaScript",
            "src/shared/Widget.js",
            "export class Widget {}",
        ),
        parsed("JavaScript", "index.js", "import './src/shared/util.js';"),
    ];
    let graph = build(&files);
    assert_eq!(graph.edges.len(), 2);
    assert!(
        graph
            .edges
            .iter()
            .all(|edge| edge.target == "src/shared/util.js")
    );
    assert_eq!(graph.unresolved_imports, ["src/view/main.js → ./util.js"]);
    assert!(graph.incomplete_coverage);
}

#[test]
fn python_class_and_call_evidence_uses_static_binding_not_matching_file_name() {
    let files = [
        parsed(
            "Python",
            "pkg/student.py",
            "from .library import Base as Parent, work\n\nclass Student(Parent):\n    pass\n\nwork()\n",
        ),
        parsed(
            "Python",
            "pkg/library.py",
            "class Base:\n    pass\n\ndef work():\n    return 1\n",
        ),
        parsed("Python", "pkg/Parent.py", "class Parent:\n    pass\n"),
    ];
    let graph = build(&files);
    assert_eq!(graph.edges.len(), 3);
    assert!(
        graph
            .edges
            .iter()
            .all(|edge| edge.target == "pkg/library.py")
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == "inheritance" && edge.range.start_line == 3)
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == "call" && edge.range.start_line == 6)
    );
    assert_eq!(graph.nodes[0].fan_out, 1);
}

#[test]
fn shadowed_import_parameter_keeps_import_evidence_without_call_edge() {
    let files = [
        parsed(
            "TypeScript",
            "src/main.ts",
            "import { work } from './helpers';\nfunction use(work: () => void) { work(); }",
        ),
        parsed("TypeScript", "src/helpers.ts", "export function work() {}"),
    ];
    let graph = build(&files);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].kind, "import");
}
