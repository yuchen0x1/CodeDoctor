//! Project import and indexing commands, backed by the shared AppState.

use crate::domain::types::ProjectRecord;
use crate::error::AppError;
use crate::import::{ProjectSummary, extract_zip, import_project_path};
use crate::index::{IndexSummary, index_directory};
use crate::state::AppState;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

#[tauri::command]
pub async fn import_project(app: AppHandle, path: String) -> Result<ProjectSummary, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        import_project_state(&app.state::<AppState>(), path)
    })
    .await
    .map_err(|_| AppError::Validation("project import worker exited".to_owned()))?
}

pub fn import_project_state(state: &AppState, path: String) -> Result<ProjectSummary, AppError> {
    let summary = import_project_path(&path)?;
    let mut paths = state
        .paths
        .lock()
        .map_err(|_| AppError::Validation("project state is unavailable".to_owned()))?;
    if paths.contains_key(&summary.project_id) {
        return Ok(summary);
    }
    let project_exists = state
        .storage()?
        .list_projects()?
        .iter()
        .any(|project| project.id == summary.project_id);
    if !project_exists {
        state.storage()?.insert_project(&ProjectRecord {
            id: summary.project_id.clone(),
            name: summary.display_name.clone(),
            normalized_path: summary.canonical_path.clone(),
            created_at: now(),
        })?;
    }
    paths.insert(
        summary.project_id.clone(),
        PathBuf::from(&summary.canonical_path),
    );
    Ok(summary)
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<Vec<ProjectRecord>, AppError> {
    state.storage()?.list_projects()
}

#[tauri::command]
pub async fn restore_project(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectSummary, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        restore_project_state(&app.state::<AppState>(), &project_id)
    })
    .await
    .map_err(|_| AppError::Validation("project restore worker exited".to_owned()))?
}

fn restore_project_state(state: &AppState, project_id: &str) -> Result<ProjectSummary, AppError> {
    let project = state
        .storage()?
        .get_project(project_id)?
        .ok_or_else(|| AppError::Validation("project does not exist".into()))?;
    if !std::path::Path::new(&project.normalized_path).try_exists()? {
        return Ok(ProjectSummary {
            project_id: project.id,
            canonical_path: project.normalized_path,
            display_name: project.name,
            source_available: false,
            languages: Vec::new(),
            frameworks: Vec::new(),
            config_files: Vec::new(),
            file_count: 0,
            source_file_count: 0,
            line_count: 0,
            max_directory_depth: 0,
        });
    }
    let summary = import_project_path(&project.normalized_path)?;
    if summary.project_id != project.id {
        return Err(AppError::Validation(
            "stored project path no longer matches its identity".into(),
        ));
    }
    state
        .paths
        .lock()
        .map_err(|_| AppError::Validation("project state is unavailable".into()))?
        .insert(project.id, PathBuf::from(&summary.canonical_path));
    Ok(summary)
}

#[tauri::command]
pub async fn index_project(app: AppHandle, project_id: String) -> Result<IndexSummary, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        index_project_state(&app.state::<AppState>(), project_id)
    })
    .await
    .map_err(|_| AppError::Validation("project index worker exited".to_owned()))?
}

pub fn index_project_state(state: &AppState, project_id: String) -> Result<IndexSummary, AppError> {
    let path = state.project_path(&project_id)?;
    if path.is_file()
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        let extracted = extract_zip(&path)?;
        let indexed = index_directory(&extracted);
        let _ = std::fs::remove_dir_all(&extracted);
        let mut summary = indexed?;
        for file in &mut summary.files {
            file.absolute_path = format!("{}::{}", path.to_string_lossy(), file.relative_path);
        }
        Ok(summary)
    } else {
        index_directory(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "codedoctor-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn directory_named_with_zip_suffix_is_indexed_as_directory() {
        let state = AppState::in_memory().unwrap();
        let root = temp_root("directory-zip-suffix");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
        let summary = import_project_state(&state, root.to_string_lossy().into_owned()).unwrap();
        let indexed = index_project_state(&state, summary.project_id).unwrap();
        assert_eq!(indexed.source_file_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn indexing_is_read_only_and_does_not_create_scan_history() {
        let state = AppState::in_memory().unwrap();
        let root = temp_root("repeat-index");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
        let summary = import_project_state(&state, root.to_string_lossy().into_owned()).unwrap();
        let project_id = summary.project_id;
        assert!(index_project_state(&state, project_id.clone()).is_ok());
        assert!(index_project_state(&state, project_id.clone()).is_ok());
        assert!(
            state
                .storage()
                .unwrap()
                .list_scan_runs(&project_id)
                .unwrap()
                .is_empty()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reimporting_a_persisted_project_after_restart_restores_its_path() {
        let root = temp_root("restore-after-restart");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
        let database = root.with_extension("sqlite");
        let project_path = root.to_string_lossy().into_owned();
        let project_id = {
            let state = AppState::open(&database.to_string_lossy()).unwrap();
            import_project_state(&state, project_path.clone())
                .unwrap()
                .project_id
        };

        let restarted = AppState::open(&database.to_string_lossy()).unwrap();
        let restored = restore_project_state(&restarted, &project_id).unwrap();

        assert_eq!(restored.project_id, project_id);
        assert!(restored.source_available);
        assert!(index_project_state(&restarted, project_id.clone()).is_ok());
        assert!(import_project_state(&restarted, project_path).is_ok());
        let _ = fs::remove_file(database);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn restores_saved_project_identity_when_source_is_missing() {
        let state = AppState::in_memory().unwrap();
        let root = temp_root("missing-source");
        fs::create_dir(&root).unwrap();
        let imported = import_project_state(&state, root.to_string_lossy().into_owned()).unwrap();
        fs::remove_dir(&root).unwrap();

        let restored = restore_project_state(&state, &imported.project_id).unwrap();
        assert_eq!(restored.project_id, imported.project_id);
        assert_eq!(restored.canonical_path, imported.canonical_path);
        assert_eq!(restored.display_name, imported.display_name);
        assert!(!restored.source_available);
    }
}
