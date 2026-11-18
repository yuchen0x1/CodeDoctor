use crate::domain::types::{Issue, PromptArtifact, PromptMode, ScanRun, ScanStatus, ScoreSnapshot};
use crate::error::AppError;
use crate::history::compare_scans;
use crate::prompt::{PromptProjectContext, generate_prompt};
use crate::report::{ReportFormat, render_with_score};
use crate::score::{ScanSnapshot, calculate_score};
use crate::state::AppState;
use crate::storage::Storage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tauri::State;

fn snapshot(state: &AppState, run_id: &str) -> Result<ScanSnapshot, AppError> {
    let store = state.storage()?;
    let run = completed_run(&store, run_id)?;
    ScanSnapshot::load(&store, run)
}

fn completed_run(store: &Storage, run_id: &str) -> Result<ScanRun, AppError> {
    let run = store
        .get_scan_run(run_id)?
        .ok_or_else(|| AppError::Validation("scan run does not exist".into()))?;
    if run.status != ScanStatus::Completed || run.rule_version == "index-1" {
        return Err(AppError::Validation(
            "this operation requires a completed scan run".into(),
        ));
    }
    Ok(run)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    pub score: ScoreSnapshot,
    pub content: String,
}

#[tauri::command]
pub fn calculate_report(state: State<AppState>, run_id: String) -> Result<ReportResult, AppError> {
    let snap = snapshot(&state, &run_id)?;
    let score = calculate_score(&snap);
    state.storage()?.save_score(&run_id, &score)?;
    Ok(ReportResult {
        content: render_with_score(&snap, &score, ReportFormat::Json)?,
        score,
    })
}

#[tauri::command]
pub fn generate_governance_prompt(
    state: State<AppState>,
    run_id: String,
    mode: PromptMode,
    selected_issue_ids: Vec<String>,
    base_run_id: Option<String>,
) -> Result<PromptArtifact, AppError> {
    build_governance_prompt(
        &state,
        &run_id,
        mode,
        &selected_issue_ids,
        base_run_id.as_deref(),
    )
}

#[tauri::command]
pub fn export_governance_prompt(
    state: State<AppState>,
    run_id: String,
    prompt_id: String,
    destination: String,
) -> Result<(), AppError> {
    export_stored_prompt(&state, &run_id, &prompt_id, &destination)
}

fn build_governance_prompt(
    state: &AppState,
    run_id: &str,
    mode: PromptMode,
    selected_issue_ids: &[String],
    base_run_id: Option<&str>,
) -> Result<PromptArtifact, AppError> {
    let (target, baseline, project_context) = {
        let store = state.storage()?;
        let target = completed_run(&store, run_id)?;
        let project = store
            .get_project(&target.project_id)?
            .ok_or_else(|| AppError::Validation("the scan project does not exist".into()))?;
        let project_context =
            PromptProjectContext::from_index(project, store.list_files_for_run(run_id)?);
        let baseline = base_run_id
            .map(|base_run_id| baseline_snapshot(&store, &target, base_run_id))
            .transpose()?;
        (
            ScanSnapshot::load(&store, target)?,
            baseline,
            project_context,
        )
    };
    if mode == PromptMode::Targeted {
        let known: HashSet<&str> = target
            .issues
            .iter()
            .map(|issue| issue.id.as_str())
            .collect();
        if selected_issue_ids
            .iter()
            .any(|id| !known.contains(id.as_str()))
        {
            return Err(AppError::Validation(
                "one or more selected issues are not in this scan".into(),
            ));
        }
    }
    let score = calculate_score(&target);
    let diff = baseline
        .as_ref()
        .map(|base| compare_scans(base, &target, None, Some(&score)));
    let mut prompt = generate_prompt(
        &target,
        &score,
        &project_context,
        mode,
        selected_issue_ids,
        diff.as_ref(),
    );
    let mut store = state.storage()?;
    store.with_phase(|phase| {
        phase.save_score(run_id, &score)?;
        phase.save_prompt(run_id, &prompt)
    })?;
    (prompt.content, prompt.created_at) = store
        .get_prompt_content(run_id, &prompt.id)?
        .ok_or_else(|| AppError::Database("prompt preview was not stored".into()))?;
    Ok(prompt)
}

fn baseline_snapshot(
    store: &Storage,
    target: &ScanRun,
    base_run_id: &str,
) -> Result<ScanSnapshot, AppError> {
    if base_run_id == target.id {
        return Err(AppError::Validation(
            "baseline and target scans must be different".into(),
        ));
    }
    let baseline = completed_run(store, base_run_id)?;
    if baseline.project_id != target.project_id {
        return Err(AppError::Validation(
            "incremental prompts require scans from the same project".into(),
        ));
    }
    ScanSnapshot::load(store, baseline)
}

fn export_stored_prompt(
    state: &AppState,
    run_id: &str,
    prompt_id: &str,
    destination: &str,
) -> Result<(), AppError> {
    let destination = validate_destination(destination, &["md", "txt"])?;
    let (content, _) = {
        let store = state.storage()?;
        completed_run(&store, run_id)?;
        store
            .get_prompt_content(run_id, prompt_id)?
            .ok_or_else(|| {
                AppError::Validation("prompt preview does not exist for this scan".into())
            })?
    };
    write_export(destination, &content)
}

#[tauri::command]
pub fn export_issue_list(
    state: State<AppState>,
    run_id: String,
    issue_ids: Vec<String>,
    destination: String,
) -> Result<(), AppError> {
    let destination = validate_destination(&destination, &["json"])?;
    let snap = snapshot(&state, &run_id)?;
    let issues = select_export_issues(&snap.issues, &issue_ids)?;
    let content = serde_json::to_string_pretty(&issues)
        .map_err(|_| AppError::Validation("issue list could not be serialized".into()))?;
    write_export(destination, &content)
}

#[tauri::command]
pub fn export_report(
    state: State<AppState>,
    run_id: String,
    format: ReportFormat,
    destination: String,
) -> Result<(), AppError> {
    let destination = validate_destination(&destination, &[report_extension(format)])?;
    let snap = snapshot(&state, &run_id)?;
    let score = calculate_score(&snap);
    state.storage()?.save_score(&run_id, &score)?;
    let content = render_with_score(&snap, &score, format)?;
    write_export(destination, &content)
}

fn report_extension(format: ReportFormat) -> &'static str {
    match format {
        ReportFormat::Json => "json",
        ReportFormat::Markdown => "md",
        ReportFormat::Txt => "txt",
        ReportFormat::Html => "html",
    }
}

fn validate_destination<'a>(
    destination: &'a str,
    extensions: &[&str],
) -> Result<&'a Path, AppError> {
    let path = Path::new(destination);
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extensions
                    .iter()
                    .any(|allowed| extension.eq_ignore_ascii_case(allowed))
            })
    {
        return Err(AppError::Validation("export destination is invalid".into()));
    }
    Ok(path)
}

fn write_export(destination: &Path, content: &str) -> Result<(), AppError> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(destination, content)
        .map_err(|_| AppError::Io("export file could not be written".into()))
}

fn select_export_issues(issues: &[Issue], selected_ids: &[String]) -> Result<Vec<Issue>, AppError> {
    if selected_ids.is_empty() {
        return Ok(issues.to_vec());
    }
    let selected: HashSet<&str> = selected_ids.iter().map(String::as_str).collect();
    let output: Vec<_> = issues
        .iter()
        .filter(|issue| selected.contains(issue.id.as_str()))
        .cloned()
        .collect();
    if output.len() != selected.len() {
        return Err(AppError::Validation(
            "one or more selected issues are not in this scan".into(),
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::ProjectRecord;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn state_with_runs() -> AppState {
        let state = AppState::in_memory().unwrap();
        {
            let mut store = state.storage().unwrap();
            for id in ["project-a", "project-b"] {
                store
                    .insert_project(&ProjectRecord {
                        id: id.into(),
                        name: id.into(),
                        normalized_path: format!("fixture/{id}"),
                        created_at: "1".into(),
                    })
                    .unwrap();
            }
            for (id, project, status, rules) in [
                ("target", "project-a", ScanStatus::Completed, "rules-1"),
                ("baseline", "project-a", ScanStatus::Completed, "rules-1"),
                (
                    "other-project",
                    "project-b",
                    ScanStatus::Completed,
                    "rules-1",
                ),
                ("queued", "project-a", ScanStatus::Queued, "rules-1"),
                (
                    "legacy-index",
                    "project-a",
                    ScanStatus::Completed,
                    "index-1",
                ),
            ] {
                store
                    .create_scan_run(&ScanRun {
                        id: id.into(),
                        project_id: project.into(),
                        status,
                        started_at: None,
                        completed_at: None,
                        config: serde_json::json!({}),
                        rule_version: rules.into(),
                        analyzer_versions: serde_json::json!({}),
                        total_files: 0,
                        scanned_files: 0,
                        total_issues: 0,
                    })
                    .unwrap();
            }
        }
        state
    }

    #[test]
    fn incremental_prompt_requires_a_distinct_completed_scan_in_the_same_project() {
        let state = state_with_runs();
        for baseline in [
            "target",
            "other-project",
            "queued",
            "legacy-index",
            "missing",
        ] {
            let result = build_governance_prompt(
                &state,
                "target",
                PromptMode::Comprehensive,
                &[],
                Some(baseline),
            );
            assert!(matches!(result, Err(AppError::Validation(_))), "{baseline}");
        }
        let result = build_governance_prompt(
            &state,
            "target",
            PromptMode::Targeted,
            &["unknown".into()],
            None,
        );
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn exports_the_exact_persisted_preview_after_generating_a_different_scope() {
        let state = state_with_runs();
        let full = build_governance_prompt(&state, "target", PromptMode::Comprehensive, &[], None)
            .unwrap();
        let delta = build_governance_prompt(
            &state,
            "target",
            PromptMode::Comprehensive,
            &[],
            Some("baseline"),
        )
        .unwrap();
        assert_ne!(full.id, delta.id);
        assert_eq!(
            build_governance_prompt(&state, "target", PromptMode::Comprehensive, &[], None)
                .unwrap(),
            full,
        );

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        for suffix in ["md", "txt"] {
            let path = std::env::temp_dir().join(format!(
                "codedoctor-prompt-{}-{stamp}.{suffix}",
                std::process::id()
            ));
            let destination = path.to_str().unwrap();
            assert!(export_stored_prompt(&state, "baseline", &full.id, destination).is_err());
            export_stored_prompt(&state, "target", &full.id, destination).unwrap();
            let exported = std::fs::read(&path).unwrap();
            std::fs::remove_file(path).unwrap();
            assert_eq!(exported, full.content.as_bytes());
        }
    }

    #[test]
    fn report_destination_extension_must_match_the_requested_format() {
        for (format, suffix) in [
            (ReportFormat::Json, "JSON"),
            (ReportFormat::Markdown, "md"),
            (ReportFormat::Txt, "txt"),
            (ReportFormat::Html, "html"),
        ] {
            let file = std::env::temp_dir().join(format!("report.{suffix}"));
            let path = file.to_str().unwrap();
            assert_eq!(
                validate_destination(path, &[report_extension(format)]).unwrap(),
                file
            );
            let other_format = if format == ReportFormat::Json {
                ReportFormat::Txt
            } else {
                ReportFormat::Json
            };
            assert!(validate_destination(path, &[report_extension(other_format)]).is_err());
        }
        assert!(validate_destination("report.json", &["json"]).is_err());
    }
}
