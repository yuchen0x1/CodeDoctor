use super::cache::rule_cache_key;
use super::events::EventSink;
use super::job::{JobHandle, PauseGate, ScanParams, run_scan_job};
use super::{new_run_id, now_millis};
use crate::commands::scan::{cancel_scan_state, get_scan_state, pause_scan_state};
use crate::domain::types::{AnalyzerStatus, CoverageKind, ProjectRecord, ScanRun, ScanStatus};
use crate::rules::config::{RULE_VERSION, RuleConfig};
use crate::state::AppState;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[test]
fn cache_key_changes_when_file_or_configuration_changes() {
    let first = rule_cache_key("project", "main.py", "hash-a", "rules-1", "{}");
    let changed_file = rule_cache_key("project", "main.py", "hash-b", "rules-1", "{}");
    let changed_config = rule_cache_key("project", "main.py", "hash-a", "rules-1", "{\"x\":1}");
    assert_ne!(first, changed_file);
    assert_ne!(first, changed_config);
}

#[test]
fn pause_gate_blocks_until_resume_and_cancel_flag_is_observable() {
    let gate = Arc::new(PauseGate::default());
    gate.pause();
    let entered = Arc::new(AtomicBool::new(false));
    let waiting_gate = Arc::clone(&gate);
    let waiting_entered = Arc::clone(&entered);
    let thread = thread::spawn(move || {
        waiting_gate.wait_if_paused();
        waiting_entered.store(true, Ordering::SeqCst);
    });
    thread::sleep(Duration::from_millis(10));
    assert!(!entered.load(Ordering::SeqCst));
    gate.resume();
    thread.join().unwrap();
    assert!(entered.load(Ordering::SeqCst));
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("codedoctor-test-{}", new_run_id(0)));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn register_project(state: &AppState, path: &Path) {
    state
        .storage()
        .unwrap()
        .insert_project(&ProjectRecord {
            id: "fixture".into(),
            normalized_path: path.to_string_lossy().into_owned(),
            name: "fixture".into(),
            created_at: now_millis().to_string(),
        })
        .unwrap();
}

fn run_record(run_id: &str, status: ScanStatus) -> ScanRun {
    ScanRun {
        id: run_id.to_owned(),
        project_id: "fixture".into(),
        status,
        started_at: Some((now_millis() - 1500).to_string()),
        completed_at: None,
        config: json!({"maxLineLength": 80}),
        rule_version: RULE_VERSION.into(),
        analyzer_versions: json!({}),
        total_files: 0,
        scanned_files: 0,
        total_issues: 0,
    }
}

fn seed_job(state: &AppState, run_id: &str, root: &Path) -> ScanParams {
    state
        .storage()
        .unwrap()
        .create_scan_run(&run_record(run_id, ScanStatus::Running))
        .unwrap();
    ScanParams {
        run_id: run_id.into(),
        project_id: "fixture".into(),
        root: root.to_path_buf(),
        config: RuleConfig {
            max_line_length: 80,
            excluded_directories: vec!["Generated".into()],
            ..RuleConfig::default()
        },
        enabled_analyzers: Vec::new(),
        workspace: None,
    }
}

#[test]
fn repeated_scan_preserves_findings_with_distinct_ids_and_actual_coverage() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("gEnErAtEd")).unwrap();
    fs::write(
        fixture.0.join("gEnErAtEd").join("ignored.py"),
        "import os\n",
    )
    .unwrap();
    fs::write(
        fixture.0.join("main.py"),
        format!("text = \"{}\"\n", "x".repeat(140)),
    )
    .unwrap();
    let state = AppState::in_memory().unwrap();
    register_project(&state, &fixture.0);
    let sink: EventSink = Box::new(|_| {});
    for id in ["first", "second"] {
        let params = seed_job(&state, id, &fixture.0);
        run_scan_job(&params, &JobHandle::new(), &sink, &state.storage).unwrap();
        let summary = get_scan_state(&state, id).unwrap();
        assert_eq!(summary.run.status, ScanStatus::Completed);
        assert_eq!(summary.run.total_files, 1);
        assert_eq!(summary.run.scanned_files, 1);
        assert_eq!(
            summary.run.config["indexedProjectContext"]["sourceLineCount"],
            1
        );
        assert!(summary.elapsed_ms >= 1500);
        let store = state.storage().unwrap();
        let file = store.get_file_for_run(id, "main.py").unwrap().unwrap();
        assert_eq!(file.parse_capability, CoverageKind::Ast);
        assert!(
            store
                .get_file_for_run(id, "gEnErAtEd/ignored.py")
                .unwrap()
                .is_none()
        );
        let analyzers = store.list_analyzer_runs_for_run(id).unwrap();
        assert!(!analyzers.is_empty());
        assert!(analyzers.iter().all(|run| matches!(
            run.status,
            AnalyzerStatus::Skipped | AnalyzerStatus::Unavailable
        )));
    }
    let store = state.storage().unwrap();
    let first = store.list_issues("first").unwrap();
    let second = store.list_issues("second").unwrap();
    assert!(!first.is_empty());
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second) {
        assert_ne!(a.id, b.id);
        assert_eq!(a.fingerprint, b.fingerprint);
    }
}

#[test]
fn paused_rules_can_be_canceled_and_terminal_status_cannot_be_overwritten() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("main.py"), "value = 1\n").unwrap();
    let state = Arc::new(AppState::in_memory().unwrap());
    register_project(&state, &fixture.0);
    let params = seed_job(&state, "active", &fixture.0);
    let handle = JobHandle::new();
    state
        .jobs
        .lock()
        .unwrap()
        .insert(params.run_id.clone(), handle.clone());
    let worker_state = state.clone();
    let (paused_tx, paused_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let control = worker_state.clone();
        let sink: EventSink = Box::new(move |event| {
            if event.stage == "rules" && event.current_path.is_none() {
                pause_scan_state(&control, "active").unwrap();
                paused_tx.send(()).unwrap();
            }
        });
        run_scan_job(&params, &handle, &sink, &worker_state.storage)
    });
    paused_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        get_scan_state(&state, "active").unwrap().run.status,
        ScanStatus::Paused
    );
    cancel_scan_state(&state, "active").unwrap();
    assert!(worker.join().unwrap().is_err());
    assert_eq!(
        get_scan_state(&state, "active").unwrap().run.status,
        ScanStatus::Canceled
    );
    pause_scan_state(&state, "active").unwrap();
    cancel_scan_state(&state, "active").unwrap();
    assert_eq!(
        get_scan_state(&state, "active").unwrap().run.status,
        ScanStatus::Canceled
    );
}

#[test]
fn canceled_zip_scan_removes_worker_workspace() {
    let fixture = Fixture::new();
    let zip_path = fixture.0.join("fixture.zip");
    let mut archive = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
    archive
        .start_file("main.py", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut archive, b"value = 1\n").unwrap();
    archive.finish().unwrap();
    let state = AppState::in_memory().unwrap();
    register_project(&state, &zip_path);
    let mut params = seed_job(&state, "archive", &zip_path);
    let workspace = fixture.0.join("workspace");
    params.workspace = Some(workspace.clone());
    let handle = JobHandle::new();
    let control = handle.clone();
    let work = workspace.clone();
    let sink: EventSink = Box::new(move |event| {
        if event.stage == "index" && work.join("main.py").is_file() {
            control.cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(run_scan_job(&params, &handle, &sink, &state.storage).is_err());
    assert_eq!(
        get_scan_state(&state, "archive").unwrap().run.status,
        ScanStatus::Canceled
    );
    assert!(zip_path.is_file());
    assert!(!workspace.exists());
}

#[test]
fn optional_ruff_uses_indexed_paths_when_installed() {
    if !crate::analyzers::discovery::discover()
        .into_iter()
        .any(|tool| tool.id == "ruff" && tool.executable.is_some())
    {
        return;
    }
    let fixture = Fixture::new();
    let source_dir = fixture.0.join("中文示例");
    fs::create_dir(&source_dir).unwrap();
    fs::write(source_dir.join("main.py"), "import os\n").unwrap();
    let ignored = fixture.0.join("Generated");
    fs::create_dir(&ignored).unwrap();
    fs::write(ignored.join("skip.py"), "import sys\n").unwrap();
    let state = AppState::in_memory().unwrap();
    register_project(&state, &fixture.0);
    let mut params = seed_job(&state, "ruff-fixture", &fixture.0);
    params.enabled_analyzers = vec!["ruff".into()];
    let sink: EventSink = Box::new(|_| {});
    run_scan_job(&params, &JobHandle::new(), &sink, &state.storage).unwrap();
    let store = state.storage().unwrap();
    let tools = store.list_analyzer_runs_for_run(&params.run_id).unwrap();
    let ruff = tools.iter().find(|tool| tool.analyzer == "ruff").unwrap();
    assert_eq!(ruff.status, AnalyzerStatus::Executed, "{:?}", ruff.message);
    let run = store.get_scan_run(&params.run_id).unwrap().unwrap();
    assert_eq!(
        run.analyzer_versions["ruff"].as_str(),
        ruff.version.as_deref()
    );
    assert!(ruff.version.is_some());
    let issues = store.list_issues(&params.run_id).unwrap();
    let ruff_issues: Vec<_> = issues
        .iter()
        .filter(|issue| issue.source == "ruff")
        .collect();
    assert_eq!(ruff_issues.len(), 1);
    assert_eq!(ruff_issues[0].file_path, "中文示例/main.py");
}

#[test]
fn restart_recovers_interrupted_runs_and_preserves_their_settings() {
    let fixture = Fixture::new();
    let db_path = fixture.0.join("fixture.db");
    let db_path = db_path.to_str().unwrap();
    let abandoned;
    let unrelated;
    {
        let state = AppState::open(db_path).unwrap();
        register_project(&state, &fixture.0);
        state
            .storage()
            .unwrap()
            .create_scan_run(&run_record("running", ScanStatus::Running))
            .unwrap();
        state
            .storage()
            .unwrap()
            .create_scan_run(&run_record("canceling", ScanStatus::Canceling))
            .unwrap();
        let root = state.scan_work_root.as_ref().unwrap();
        abandoned = root.join(new_run_id(1));
        unrelated = root.join("unrelated");
        fs::create_dir(&abandoned).unwrap();
        fs::create_dir(&unrelated).unwrap();
        assert!(AppState::open(db_path).is_err());
        assert_eq!(
            get_scan_state(&state, "running").unwrap().run.status,
            ScanStatus::Running
        );
    }
    let reopened = AppState::open(db_path).unwrap();
    let failed = get_scan_state(&reopened, "running").unwrap();
    assert_eq!(failed.run.status, ScanStatus::Failed);
    assert_eq!(failed.run.config["maxLineLength"], json!(80));
    assert_eq!(failed.run.config["interrupted"], json!(true));
    assert_eq!(
        get_scan_state(&reopened, "canceling").unwrap().run.status,
        ScanStatus::Canceled
    );
    assert!(!abandoned.exists());
    assert!(unrelated.exists());
}
