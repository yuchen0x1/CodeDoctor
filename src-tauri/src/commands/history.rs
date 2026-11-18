use crate::domain::types::ScanRun;
use crate::domain::types::ScanStatus;
use crate::error::AppError;
use crate::history::{HistoryDiff, compare_scans};
use crate::score::{SCORING_VERSION, ScanSnapshot};
use crate::state::AppState;
use serde::Serialize;
#[cfg(feature = "desktop")]
use tauri::State;

fn snapshot(state: &AppState, run_id: &str) -> Result<ScanSnapshot, AppError> {
    let store = state.storage()?;
    let run = store
        .get_scan_run(run_id)?
        .ok_or_else(|| AppError::Validation("scan run does not exist".into()))?;
    if run.status != ScanStatus::Completed || run.rule_version == "index-1" {
        return Err(AppError::Validation(
            "history comparison requires completed scan runs".into(),
        ));
    }
    ScanSnapshot::load(&store, run)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn compare_scan_history(
    state: State<AppState>,
    base_run_id: String,
    target_run_id: String,
) -> Result<HistoryDiff, AppError> {
    if base_run_id == target_run_id {
        return Err(AppError::Validation(
            "baseline and target scans must be different".into(),
        ));
    }
    let base = snapshot(&state, &base_run_id)?;
    let target = snapshot(&state, &target_run_id)?;
    if base.run.project_id != target.run.project_id {
        return Err(AppError::Validation(
            "history comparison requires scans from the same project".into(),
        ));
    }
    let store = state.storage()?;
    let current =
        |run_id| {
            Ok::<_, AppError>(store.get_score(run_id)?.filter(|score| {
                score.dimensions["scoringVersion"].as_str() == Some(SCORING_VERSION)
            }))
        };
    let base_score = current(&base_run_id)?;
    let target_score = current(&target_run_id)?;
    Ok(compare_scans(
        &base,
        &target,
        base_score.as_ref(),
        target_score.as_ref(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanHistoryRow {
    #[serde(flatten)]
    run: ScanRun,
    score_summary: Option<ScanScoreSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanScoreSummary {
    overall_score: f32,
    coverage: f32,
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_scan_runs(
    state: State<AppState>,
    project_id: String,
) -> Result<Vec<ScanHistoryRow>, AppError> {
    let store = state.storage()?;
    let mut runs = store.list_scan_runs(&project_id)?;
    // The original database stored seconds. Modern runs use milliseconds.
    runs.sort_by_key(|run| {
        let timestamp = run
            .started_at
            .as_deref()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0_u64);
        std::cmp::Reverse(if timestamp < 100_000_000_000 {
            timestamp * 1000
        } else {
            timestamp
        })
    });
    runs.into_iter()
        .filter(|run| run.rule_version != "index-1")
        .map(|run| {
            let score_summary = store
                .get_score(&run.id)?
                .filter(|score| {
                    score.dimensions["scoringVersion"].as_str() == Some(SCORING_VERSION)
                })
                .map(|score| ScanScoreSummary {
                    overall_score: score.overall_score,
                    coverage: score.coverage,
                });
            Ok(ScanHistoryRow { run, score_summary })
        })
        .collect()
}
