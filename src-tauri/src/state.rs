//! Shared application state managed by Tauri: the on-disk storage handle,
//! the imported-project registry, and active scan job controls.

use crate::error::AppError;
use crate::scan::job::JobHandle;
use crate::scan::{is_managed_scan_id, now_millis};
use crate::storage::Storage;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub storage: Arc<Mutex<Storage>>,
    /// project_id → canonical root path (directory or .zip archive)
    pub paths: Mutex<HashMap<String, PathBuf>>,
    pub jobs: Arc<Mutex<HashMap<String, Arc<JobHandle>>>>,
    pub scan_work_root: Option<PathBuf>,
    _database_lock: Option<File>,
}

impl AppState {
    /// Production state backed by a SQLite database file.
    pub fn open(db_path: &str) -> Result<Self, AppError> {
        let database_path = Path::new(db_path);
        if let Some(parent) = database_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        // Recovery shares the data directory with the live worker. Hold an
        // exclusive OS lock so a second app process cannot recover its jobs.
        let database_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(database_path.with_extension("app-lock"))?;
        database_lock
            .try_lock()
            .map_err(|_| AppError::Validation("the local database is already in use".to_owned()))?;
        let scan_work_root = database_path.with_extension("scan-work");
        fs::create_dir_all(&scan_work_root)?;
        if !fs::symlink_metadata(&scan_work_root)?.file_type().is_dir() {
            return Err(AppError::Validation("scan workspace is invalid".to_owned()));
        }
        for entry in fs::read_dir(&scan_work_root)? {
            let entry = entry?;
            if is_managed_scan_id(&entry.file_name().to_string_lossy())
                && entry.file_type()?.is_dir()
            {
                // A crash can leave temporary files. Failed removals remain
                // eligible for cleanup on subsequent launches.
                let _ = fs::remove_dir_all(entry.path());
            }
        }
        let mut storage = Storage::open(db_path)?;
        storage.recover_interrupted_scans(&now_millis().to_string())?;
        let paths = storage
            .list_projects()?
            .into_iter()
            .filter_map(|project| {
                let path = PathBuf::from(project.normalized_path);
                path.exists().then_some((project.id, path))
            })
            .collect();
        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
            paths: Mutex::new(paths),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            scan_work_root: Some(scan_work_root),
            _database_lock: Some(database_lock),
        })
    }

    /// In-memory state for tests.
    pub fn in_memory() -> Result<Self, AppError> {
        Ok(Self {
            storage: Arc::new(Mutex::new(Storage::in_memory()?)),
            paths: Mutex::new(HashMap::new()),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            scan_work_root: None,
            _database_lock: None,
        })
    }

    pub fn storage(&self) -> Result<std::sync::MutexGuard<'_, Storage>, AppError> {
        self.storage
            .lock()
            .map_err(|_| AppError::Validation("local storage is unavailable".to_owned()))
    }

    pub fn project_path(&self, project_id: &str) -> Result<PathBuf, AppError> {
        self.paths
            .lock()
            .map_err(|_| AppError::Validation("project state is unavailable".to_owned()))?
            .get(project_id)
            .cloned()
            .ok_or_else(|| AppError::Validation("project is not imported".to_owned()))
    }
}
