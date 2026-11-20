//! Scan lifecycle commands: start/pause/resume/cancel/query. Progress is
//! delivered through the `scan://progress` event channel only.

#[cfg(feature = "desktop")]
use crate::domain::types::AnalyzerRun;
use crate::domain::types::{Issue, ScanRun, ScanStatus};
use crate::error::AppError;
use crate::rules::config::RULE_VERSION;
use crate::rules::config::RuleConfig;
use crate::scan::events::EventSink;
use crate::scan::job::{JobHandle, run_scan_job};
use crate::scan::{ScanParams, ScanRunSummary, new_run_id, now_millis};
use crate::state::AppState;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "desktop")]
use tauri::{AppHandle, Emitter, State};

static SCAN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn elapsed_of(run: &ScanRun) -> u64 {
    if run
        .config
        .get("interrupted")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        return 0;
    }
    fn epoch_millis(timestamp: &str) -> Option<u64> {
        // Versions before scanEvidenceVersion stored Unix seconds.
        let value = timestamp.parse::<u64>().ok()?;
        Some(if value < 100_000_000_000 {
            value.saturating_mul(1000)
        } else {
            value
        })
    }
    let start = run.started_at.as_deref().and_then(epoch_millis);
    let end = run
        .completed_at
        .as_deref()
        .and_then(epoch_millis)
        .or_else(|| {
            matches!(
                run.status,
                ScanStatus::Running | ScanStatus::Paused | ScanStatus::Canceling
            )
            .then(now_millis)
        });
    match (start, end) {
        (Some(start), Some(end)) => end.saturating_sub(start),
        _ => 0,
    }
}

fn summary_of(run: ScanRun) -> ScanRunSummary {
    ScanRunSummary {
        elapsed_ms: elapsed_of(&run),
        run,
    }
}

fn scan_configuration(
    config: Option<serde_json::Value>,
    analyzers: Option<Vec<String>>,
) -> Result<(RuleConfig, Vec<String>, serde_json::Value), AppError> {
    let mut config: RuleConfig = config
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| AppError::Validation("scan configuration is invalid".to_owned()))?
        .unwrap_or_default();
    let thresholds = [
        (config.max_line_length as u64, 40, 1000),
        (config.max_function_lines.into(), 5, 1000),
        (config.max_cyclomatic.into(), 1, 200),
        (config.max_cognitive.into(), 1, 500),
        (config.max_nesting.into(), 1, 32),
        (config.max_parameters.into(), 1, 100),
        (config.max_class_methods.into(), 1, 500),
        (config.max_class_lines.into(), 20, 10_000),
        (config.max_file_lines.into(), 50, 100_000),
        (config.duplicate_min_lines as u64, 3, 200),
    ];
    if thresholds
        .iter()
        .any(|(value, min, max)| !(min..=max).contains(&value))
    {
        return Err(AppError::Validation(
            "a scan threshold is out of range".to_owned(),
        ));
    }
    config.excluded_directories =
        crate::import::path::normalize_excluded_directories(&config.excluded_directories);
    let analyzers: Vec<String> = analyzers
        .unwrap_or_default()
        .into_iter()
        .find(|id| id == "ruff")
        .into_iter()
        .collect();
    let mut recorded = serde_json::to_value(&config)
        .map_err(|_| AppError::Validation("scan configuration could not be stored".to_owned()))?;
    recorded["scanEvidenceVersion"] = json!(1);
    recorded["enabledAnalyzers"] = json!(analyzers);
    Ok((config, analyzers, recorded))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn start_scan(
    state: State<AppState>,
    app: AppHandle,
    project_id: String,
    config: Option<serde_json::Value>,
    analyzers: Option<Vec<String>>,
) -> Result<ScanRunSummary, AppError> {
    start_scan_state(
        &state,
        project_id,
        config,
        analyzers,
        Box::new(move |event| {
            let _ = app.emit("scan://progress", event);
        }),
    )
}

pub fn start_scan_state(
    state: &AppState,
    project_id: String,
    config: Option<serde_json::Value>,
    analyzers: Option<Vec<String>>,
    sink: EventSink,
) -> Result<ScanRunSummary, AppError> {
    let root = state.project_path(&project_id)?;
    let run_id = new_run_id(SCAN_SEQUENCE.fetch_add(1, Ordering::Relaxed));
    let (rule_config, enabled_analyzers, recorded_config) = scan_configuration(config, analyzers)?;
    let started_at = now_millis().to_string();
    let run = ScanRun {
        id: run_id.clone(),
        project_id: project_id.clone(),
        status: ScanStatus::Running,
        started_at: Some(started_at),
        completed_at: None,
        failure_message: None,
        config: recorded_config,
        rule_version: RULE_VERSION.to_owned(),
        analyzer_versions: json!({}),
        total_files: 0,
        scanned_files: 0,
        total_issues: 0,
    };
    let mut jobs = state.jobs.lock().map_err(|_| scan_state_unavailable())?;
    {
        let mut store = state.storage()?;
        if store.has_active_scan(&project_id)? {
            return Err(AppError::Validation(
                "this project already has an active scan".to_owned(),
            ));
        }
        store.create_scan_run(&run)?;
    }

    let handle = JobHandle::new();
    let storage_arc = state.storage.clone();
    let params = ScanParams {
        run_id: run_id.clone(),
        project_id: project_id.clone(),
        root,
        config: rule_config,
        enabled_analyzers,
        workspace: state.scan_work_root.as_ref().map(|path| path.join(&run_id)),
    };
    jobs.insert(run_id.clone(), handle.clone());
    let active_jobs = Arc::clone(&state.jobs);
    if std::thread::Builder::new()
        .name("codedoctor-scan".to_owned())
        .spawn(move || {
            let _ = run_scan_job(&params, &handle, &sink, &storage_arc);
            if let Ok(mut jobs) = active_jobs.lock() {
                jobs.remove(&params.run_id);
            }
        })
        .is_err()
    {
        jobs.remove(&run_id);
        let error = AppError::Validation("scan worker could not be started".to_owned());
        state.storage()?.finish_scan_run(
            &run_id,
            ScanStatus::Failed,
            &now_millis().to_string(),
            0,
            0,
            Some(&error),
        )?;
        return Err(error);
    }

    Ok(summary_of(run))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn pause_scan(state: State<AppState>, run_id: String) -> Result<(), AppError> {
    pause_scan_state(&state, &run_id)
}

pub fn pause_scan_state(state: &AppState, run_id: &str) -> Result<(), AppError> {
    control_scan(state, run_id, ScanStatus::Paused)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn resume_scan(state: State<AppState>, run_id: String) -> Result<(), AppError> {
    resume_scan_state(&state, &run_id)
}

pub fn resume_scan_state(state: &AppState, run_id: &str) -> Result<(), AppError> {
    control_scan(state, run_id, ScanStatus::Running)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn cancel_scan(state: State<AppState>, run_id: String) -> Result<(), AppError> {
    cancel_scan_state(&state, &run_id)
}

pub fn cancel_scan_state(state: &AppState, run_id: &str) -> Result<(), AppError> {
    control_scan(state, run_id, ScanStatus::Canceling)
}

fn scan_state_unavailable() -> AppError {
    AppError::Validation("scan state is unavailable".to_owned())
}

fn control_scan(state: &AppState, run_id: &str, next: ScanStatus) -> Result<(), AppError> {
    let handle = state
        .jobs
        .lock()
        .map_err(|_| scan_state_unavailable())?
        .get(run_id)
        .cloned();
    let mut store = state.storage()?;
    let status = store
        .get_scan_run(run_id)?
        .ok_or_else(|| AppError::Validation("scan run does not exist".to_owned()))?
        .status;
    if status == next
        || matches!(
            status,
            ScanStatus::Completed | ScanStatus::Failed | ScanStatus::Canceled
        )
    {
        return Ok(());
    }
    let permitted = matches!(
        (status, next),
        (
            ScanStatus::Running,
            ScanStatus::Paused | ScanStatus::Canceling
        ) | (
            ScanStatus::Paused,
            ScanStatus::Running | ScanStatus::Canceling
        )
    );
    let handle = handle
        .filter(|_| permitted)
        .ok_or_else(|| AppError::Validation("scan cannot make this status change".to_owned()))?;
    if !store.transition_scan_status(run_id, status, next)? {
        return Err(AppError::Validation(
            "scan status has already changed".to_owned(),
        ));
    }
    match next {
        ScanStatus::Paused => handle.gate.pause(),
        ScanStatus::Running => handle.gate.resume(),
        ScanStatus::Canceling => {
            handle.cancel.store(true, Ordering::Relaxed);
            handle.gate.resume();
        }
        _ => unreachable!(),
    }
    Ok(())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_scan(state: State<AppState>, run_id: String) -> Result<ScanRunSummary, AppError> {
    get_scan_state(&state, &run_id)
}

pub fn get_scan_state(state: &AppState, run_id: &str) -> Result<ScanRunSummary, AppError> {
    state
        .storage()?
        .get_scan_run(run_id)?
        .map(summary_of)
        .ok_or_else(|| AppError::Validation("scan run does not exist".to_owned()))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_analyzer_runs(
    state: State<AppState>,
    run_id: String,
) -> Result<Vec<AnalyzerRun>, AppError> {
    state.storage()?.list_analyzer_runs_for_run(&run_id)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuePage {
    pub issues: Vec<Issue>,
    pub total: i64,
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_issues(
    state: State<AppState>,
    run_id: String,
    severity: Option<String>,
    category: Option<String>,
    file_path: Option<String>,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<IssuePage, AppError> {
    list_issues_state(
        &state, run_id, severity, category, file_path, search, limit, offset,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn list_issues_state(
    state: &AppState,
    run_id: String,
    severity: Option<String>,
    category: Option<String>,
    file_path: Option<String>,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<IssuePage, AppError> {
    let filter = crate::storage::IssueFilter {
        severity,
        category,
        file_path,
        search,
        limit,
        offset,
    };
    let (issues, total) = state.storage()?.list_issues_filtered(&run_id, &filter)?;
    Ok(IssuePage { issues, total })
}

/// Convenience wrapper returning all issues of a run (exports/history).
pub fn all_issues(state: &AppState, run_id: &str) -> Result<Vec<Issue>, AppError> {
    state.storage()?.list_issues(run_id)
}
