use super::*;
use crate::domain::types::{CoverageKind, Issue, ProjectRecord, ScanRun, Severity};
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

const RUN: &str = "run-source";
const ISSUE: &str = "issue-source";
const PROJECT: &str = "project-source";

struct Fixture(PathBuf);

impl Fixture {
    fn create() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "codedoctor-source-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn directory_state(&self, relative: &str, bytes: &[u8], line: u32, end: u32) -> AppState {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        snapshot(&self.0, relative, bytes, line, end)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn snapshot(root: &Path, relative: &str, bytes: &[u8], line: u32, end: u32) -> AppState {
    let state = AppState::in_memory().unwrap();
    state
        .paths
        .lock()
        .unwrap()
        .insert(PROJECT.into(), root.to_path_buf());
    let mut store = state.storage().unwrap();
    store
        .insert_project(&ProjectRecord {
            id: PROJECT.into(),
            name: "Fixture".into(),
            normalized_path: root.to_string_lossy().into_owned(),
            created_at: "0".into(),
        })
        .unwrap();
    store
        .create_scan_run(&ScanRun {
            id: RUN.into(),
            project_id: PROJECT.into(),
            status: ScanStatus::Completed,
            started_at: Some("1".into()),
            completed_at: Some("2".into()),
            failure_message: None,
            config: json!({}),
            rule_version: "test".into(),
            analyzer_versions: json!({}),
            total_files: 1,
            scanned_files: 1,
            total_issues: 1,
        })
        .unwrap();
    store
        .upsert_file(&FileRecord {
            id: "file-source".into(),
            scan_run_id: RUN.into(),
            project_id: PROJECT.into(),
            relative_path: relative.into(),
            absolute_path: "ignored-legacy-path".into(),
            encoding: decode_bytes(bytes).unwrap().encoding,
            size_bytes: bytes.len() as u64,
            content_hash: format!("{:x}", Sha256::digest(bytes)),
            language: "Python".into(),
            parse_capability: CoverageKind::Ast,
            excluded_reason: None,
        })
        .unwrap();
    store
        .insert_issue(
            RUN,
            &Issue {
                id: ISSUE.into(),
                rule_id: "fixture.rule".into(),
                category: "style".into(),
                severity: Severity::Low,
                file_path: relative.into(),
                start_line: line,
                start_column: 1,
                end_line: end,
                end_column: 1,
                title: String::new(),
                explanation: String::new(),
                evidence: "persisted evidence must stay out of source previews".into(),
                impact: String::new(),
                recommendation: String::new(),
                source: "codedoctor".into(),
                coverage: CoverageKind::Ast,
                confidence: 1.0,
                fingerprint: String::new(),
            },
        )
        .unwrap();
    drop(store);
    state
}

fn read(state: &AppState) -> IssueSourceContext {
    read_issue_source_state(state, RUN, ISSUE).unwrap().unwrap()
}

#[test]
fn reads_numbered_utf16_context_for_a_completed_scan() {
    let fixture = Fixture::create();
    let text = (1..=23)
        .map(|line| format!("源码第 {line} 行\r\n"))
        .collect::<String>();
    let bytes: Vec<_> = [0xff, 0xfe]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let state = fixture.directory_state("src/中文.py", &bytes, 10, 11);
    let context = read(&state);
    assert_eq!((context.start_line, context.end_line), (4, 17));
    assert_eq!(context.text.lines().nth(6), Some("源码第 10 行"));
    assert!(!context.stale);
    assert!(!context.redacted);
    assert!(!context.truncated);
    assert!(!context.text.contains("persisted evidence"));
    assert_eq!(fs::read(fixture.0.join("src/中文.py")).unwrap(), bytes);
    #[cfg(windows)]
    assert_eq!(
        context.can_open_natively,
        native_editor::available_for(&fixture.0.join("src/中文.py"))
    );
}

#[test]
fn rejects_unknown_issue_run_pair_and_active_run() {
    let fixture = Fixture::create();
    let state = fixture.directory_state("example.py", b"print(1)\n", 1, 1);
    assert!(read_issue_source_state(&state, RUN, "different-issue").is_err());
    assert!(read_issue_source_state(&state, "different-run", ISSUE).is_err());
    let mut active = state.storage().unwrap().get_scan_run(RUN).unwrap().unwrap();
    active.id = "active-run".into();
    active.status = ScanStatus::Running;
    active.completed_at = None;
    state.storage().unwrap().create_scan_run(&active).unwrap();
    assert_eq!(
        read_issue_source_state(&state, &active.id, ISSUE)
            .unwrap_err()
            .to_string(),
        "source context requires a completed scan run"
    );
}

#[test]
fn does_not_return_same_size_changed_source() {
    let fixture = Fixture::create();
    let state = fixture.directory_state("example.py", b"print(1)\n", 1, 1);
    fs::write(fixture.0.join("example.py"), b"print(2)\n").unwrap();
    let context = read(&state);
    assert!(context.stale);
    assert!(context.text.is_empty());
    assert!(!context.can_open_natively);
}

#[test]
fn verifies_legacy_scan_hashes_and_rejects_changed_source() {
    let fixture = Fixture::create();
    let state = fixture.directory_state("example.py", b"print(1)\n", 1, 1);
    {
        let mut store = state.storage().unwrap();
        let mut file = store.get_file_for_run(RUN, "example.py").unwrap().unwrap();
        file.content_hash.truncate(32);
        store.upsert_file(&file).unwrap();
    }
    let context = read(&state);
    assert!(!context.stale);
    assert_eq!(context.text, "print(1)");

    fs::write(fixture.0.join("example.py"), b"print(2)\n").unwrap();
    let context = read(&state);
    assert!(context.stale);
    assert!(context.text.is_empty());
}

#[test]
fn previews_a_finding_created_by_the_scan_job() {
    use crate::rules::config::RuleConfig;
    use crate::scan::{JobHandle, ScanParams, events::EventSink, job::run_scan_job};

    let fixture = Fixture::create();
    let text = format!("value = \"{}\"\n", "x".repeat(140));
    let state = fixture.directory_state("example.py", text.as_bytes(), 1, 1);
    let mut run = state.storage().unwrap().get_scan_run(RUN).unwrap().unwrap();
    run.id = "real-scan".into();
    run.status = ScanStatus::Running;
    run.completed_at = None;
    state.storage().unwrap().create_scan_run(&run).unwrap();
    let params = ScanParams {
        run_id: run.id.clone(),
        project_id: PROJECT.into(),
        root: fixture.0.clone(),
        config: RuleConfig::default(),
        enabled_analyzers: Vec::new(),
        workspace: None,
    };
    let sink: EventSink = Box::new(|_| {});
    run_scan_job(&params, &JobHandle::new(), &sink, &state.storage).unwrap();

    let issue = {
        let store = state.storage().unwrap();
        let file = store
            .get_file_for_run(&run.id, "example.py")
            .unwrap()
            .unwrap();
        assert_eq!(
            file.content_hash,
            format!("{:x}", Sha256::digest(text.as_bytes()))
        );
        store
            .list_issues(&run.id)
            .unwrap()
            .into_iter()
            .find(|issue| issue.rule_id == "quality.long-line")
            .unwrap()
    };
    let context = read_issue_source_state(&state, &run.id, &issue.id)
        .unwrap()
        .unwrap();
    assert!(!context.stale);
    assert_eq!(context.text, text.trim_end());
}

#[test]
fn refuses_unsafe_paths_and_reparse_points() {
    let root = Fixture::create();
    let outside = Fixture::create();
    fs::write(outside.0.join("out.py"), b"print(1)\n").unwrap();
    for path in [
        "../out.py",
        "nested/../out.py",
        "/root.py",
        "C:out.py",
        "out.py:stream",
        "x\\out.py",
    ] {
        let state = snapshot(&root.0, path, b"print(1)\n", 1, 1);
        assert!(matches!(
            read_issue_source_state(&state, RUN, ISSUE),
            Err(AppError::Validation(_))
        ));
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside.0, root.0.join("link")).unwrap();
        let state = snapshot(&root.0, "link/out.py", b"print(1)\n", 1, 1);
        assert!(matches!(
            read_issue_source_state(&state, RUN, ISSUE),
            Err(AppError::Validation(_))
        ));
    }
}

#[test]
fn masks_credential_lines_and_multiline_private_keys_in_other_findings() {
    let fixture = Fixture::create();
    let bytes = b"before\npassword = fake-fixture-only\n-----BEGIN PRIVATE KEY-----\nplaceholder-fixture-key\n-----END PRIVATE KEY-----\nafter\n";
    let state = fixture.directory_state("example.py", bytes, 1, 1);
    let context = read(&state);
    assert!(context.redacted);
    assert!(!context.can_open_natively);
    assert_eq!(
        context
            .text
            .lines()
            .filter(|line| line.contains("已隐藏"))
            .count(),
        4
    );
    assert!(context.text.starts_with("before\n"));
    assert!(context.text.ends_with("\nafter"));
    assert!(!context.text.contains("fake-fixture-only"));
    assert!(!context.text.contains("placeholder-fixture-key"));
    assert!(open_issue_source_state(&state, RUN, ISSUE).is_err());
}

#[test]
fn redacts_multiline_credential_values_when_excerpt_starts_after_the_key() {
    let fixture = Fixture::create();
    let cases = [
        (
            "triple-quoted Python",
            "password = \"\"\"\n",
            "  synthetic-python-secret-marker\n",
            "\"\"\"\n",
        ),
        (
            "JavaScript template literal",
            "api_key = `\n",
            "  synthetic-backtick-secret-marker\n",
            "`\n",
        ),
        (
            "TypeScript annotated template literal",
            "const api_key: string = `\n",
            "  synthetic-typescript-secret-marker\n",
            "`\n",
        ),
        (
            "TypeScript annotation containing an equals sign",
            "const api_key: { format: \"x=y\" } = `\n",
            "  synthetic-typescript-type-secret-marker\n",
            "`\n",
        ),
        (
            "Python annotated triple-quoted string",
            "password: str = \"\"\"\n",
            "  synthetic-python-annotated-secret-marker\n",
            "\"\"\"\n",
        ),
        (
            "Python annotated single-quoted string",
            "password: str = '''\n",
            "  synthetic-python-single-secret-marker\n",
            "'''\n",
        ),
        (
            "Go short declaration raw string",
            "api_key := `\n",
            "  synthetic-go-secret-marker\n",
            "`\n",
        ),
        (
            "YAML block scalar",
            "  secret: |\n",
            "    synthetic-yaml-secret-marker\n",
            "  ordinary: value\n",
        ),
        (
            "YAML block scalar with an equals sign in its comment",
            "  secret: | # setting=value\n",
            "    synthetic-yaml-comment-secret-marker\n",
            "  ordinary: value\n",
        ),
        (
            "YAML folded scalar with an equals sign in its comment",
            "  secret: >- # setting=value\n",
            "    synthetic-yaml-folded-secret-marker\n",
            "  ordinary: value\n",
        ),
    ];

    for (name, opener, secret, closer) in cases {
        let text = format!(
            "{}{opener}{secret}{closer}{}",
            "preamble\n".repeat(8),
            (1..=20)
                .map(|line| format!("ordinary code {line}\n"))
                .collect::<String>()
        );
        let state = fixture.directory_state("example.py", text.as_bytes(), 16, 16);
        let context = read(&state);
        assert!(context.redacted, "{name}");
        assert!(
            context.text.lines().any(|line| line == "[疑似凭据已隐藏]"),
            "{name} continuation should be masked"
        );
        assert!(!context.text.contains(secret.trim()), "{name}");
        assert!(context.text.contains("ordinary code 10"), "{name}");
        assert!(
            open_issue_source_state(&state, RUN, ISSUE).is_err(),
            "{name}"
        );
    }
}

#[test]
fn redacts_python_prefixed_multiline_credentials() {
    let fixture = Fixture::create();
    for prefix in [
        "r", "R", "u", "U", "b", "B", "f", "F", "t", "T", "fr", "rf", "br", "rb", "tr", "rt", "Fr",
        "rF", "BR", "RB", "Tr", "rT",
    ] {
        for quote in ["\"\"\"", "'''"] {
            let text = format!(
                "password: str = {prefix}{quote}\nsynthetic-prefixed-credential-marker\n{quote}\nprint('done')\n"
            );
            let state = fixture.directory_state("example.py", text.as_bytes(), 2, 2);
            let context = read(&state);
            assert!(context.redacted, "{prefix}{quote}");
            assert!(!context.can_open_natively, "{prefix}{quote}");
            assert!(
                !context
                    .text
                    .contains("synthetic-prefixed-credential-marker"),
                "{prefix}{quote}"
            );
            assert!(context.text.contains("print('done')"), "{prefix}{quote}");
            assert!(
                open_issue_source_state(&state, RUN, ISSUE).is_err(),
                "{prefix}{quote}"
            );
        }
    }
}

#[test]
fn redacts_parenthesized_multiline_credentials() {
    let fixture = Fixture::create();
    for opener in [
        "password: str = (\n    r\"\"\"",
        "password = (f\"\"\"",
        "password = (\n    # a comment\n    \n    fr\"\"\"",
    ] {
        let text = format!(
            "{opener}\nsynthetic-parenthesized-credential-marker\n\"\"\"\n)\nprint('done')\n"
        );
        let state = fixture.directory_state("example.py", text.as_bytes(), 3, 3);
        let context = read(&state);
        assert!(context.redacted);
        assert!(
            !context
                .text
                .contains("synthetic-parenthesized-credential-marker")
        );
        assert!(context.text.contains("print('done')"));
        assert!(!context.can_open_natively);
    }
}

#[test]
fn redacts_concatenated_credential_when_excerpt_starts_after_the_assignment() {
    let fixture = Fixture::create();
    let text = format!(
        "password = (\n{}    \"synthetic-concatenated-credential\"\n)\nprint('done')\n",
        "    \"part )\"\n".repeat(12)
    );
    let state = fixture.directory_state("example.py", text.as_bytes(), 14, 14);
    let context = read(&state);
    assert!(context.start_line > 1);
    assert!(context.redacted);
    assert!(!context.text.contains("synthetic-concatenated-credential"));
    assert!(!context.text.contains("part )"));
    assert!(context.text.contains("print('done')"));
    assert!(!context.can_open_natively);
    assert!(open_issue_source_state(&state, RUN, ISSUE).is_err());
}

#[test]
fn keeps_unrelated_multiline_code_visible() {
    let fixture = Fixture::create();
    let text = "message = \"\"\"\nordinary synthetic text marker\n\"\"\"\nprint('done')\n";
    let state = fixture.directory_state("example.py", text.as_bytes(), 2, 2);
    let context = read(&state);
    assert!(!context.redacted);
    assert!(context.text.contains("ordinary synthetic text marker"));
    assert!(context.text.contains("print('done')"));
}

#[test]
fn caps_excerpt_width_and_height_at_utf8_boundaries() {
    let fixture = Fixture::create();
    let line = "界".repeat(MAX_LINE_CHARS + 1);
    let text = format!("{line}\n").repeat(80);
    let state = fixture.directory_state("example.py", text.as_bytes(), 10, 80);
    let context = read(&state);
    assert!(context.truncated);
    assert_eq!(context.start_line, 4);
    assert_eq!(context.end_line - context.start_line + 1, MAX_EXCERPT_LINES);
    assert_eq!(
        context.text.lines().next().unwrap().chars().count(),
        MAX_LINE_CHARS + 1
    );
    assert!(context.text.lines().all(|line| line.ends_with('…')));
}

#[test]
fn previews_zip_member_without_exposing_native_opener() {
    use ::zip::{ZipWriter, write::SimpleFileOptions};

    let fixture = Fixture::create();
    let path = fixture.0.join("source.zip");
    let bytes = b"example source\n";
    let mut archive = ZipWriter::new(File::create(&path).unwrap());
    archive
        .start_file("src/main.py", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(bytes).unwrap();
    drop(archive.finish().unwrap());
    let zip_before = fs::read(&path).unwrap();
    let state = snapshot(&path.canonicalize().unwrap(), "src/main.py", bytes, 1, 1);
    let context = read(&state);
    assert_eq!(context.text, "example source");
    assert!(!context.can_open_natively);
    assert!(open_issue_source_state(&state, RUN, ISSUE).is_err());
    assert_eq!(fs::read(&path).unwrap(), zip_before);
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);

    let mut changed = ZipWriter::new(File::create(&path).unwrap());
    changed
        .start_file("src/main.py", SimpleFileOptions::default())
        .unwrap();
    changed.write_all(b"updated source\n").unwrap();
    drop(changed.finish().unwrap());
    let stale = read(&state);
    assert!(stale.stale);
    assert!(stale.text.is_empty());
}
