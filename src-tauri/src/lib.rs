pub mod analyzers;
pub mod clones;
pub mod commands;
pub mod domain;
pub mod error;
pub mod graph;
pub mod history;
pub mod import;
pub mod index;
pub mod parse;
pub mod prompt;
pub mod report;
pub mod rules;
pub mod scan;
pub mod score;
pub mod state;
pub mod storage;

use state::AppState;

/// Liveness probe for the Rust core.
///
/// The frontend calls this on startup to prove the command bridge works before
/// any scanning feature depends on it.
#[tauri::command]
fn health() -> &'static str {
    "ok"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app data dir unavailable: {e}"))?;
            let db_path = data_dir.join("codedoctor.db");
            let state = AppState::open(&db_path.to_string_lossy())
                .map_err(|e| format!("local storage unavailable: {e}"))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            commands::project::import_project,
            commands::project::list_projects,
            commands::project::restore_project,
            commands::project::index_project,
            commands::scan::start_scan,
            commands::scan::pause_scan,
            commands::scan::resume_scan,
            commands::scan::cancel_scan,
            commands::scan::get_scan,
            commands::scan::list_issues,
            commands::scan::list_analyzer_runs,
            commands::source::read_issue_source,
            commands::source::open_issue_source,
            commands::report::calculate_report,
            commands::report::generate_governance_prompt,
            commands::report::export_governance_prompt,
            commands::report::export_issue_list,
            commands::report::export_report,
            commands::architecture::get_architecture_graph,
            commands::history::compare_scan_history,
            commands::history::list_scan_runs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
