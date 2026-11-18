//! Read-only source excerpts tied to completed scan snapshots.

use crate::domain::types::{FileRecord, ScanStatus};
use crate::error::AppError;
use crate::index::{MAX_SOURCE_FILE_BYTES, encoding::decode_bytes};
use crate::rules::security::{HARDCODED_SECRET_RULE_ID, may_expose_credential};
use crate::state::AppState;
use crate::storage::IssueLocation;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(feature = "desktop")]
use tauri::{AppHandle, Manager};

#[cfg(test)]
mod tests;

const CONTEXT_LINES: u32 = 6;
const MAX_EXCERPT_LINES: u32 = 32;
const MAX_LINE_CHARS: usize = 640;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueSourceContext {
    text: String,
    start_line: u32,
    end_line: u32,
    stale: bool,
    redacted: bool,
    truncated: bool,
    can_open_natively: bool,
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn read_issue_source(
    app: AppHandle,
    run_id: String,
    issue_id: String,
) -> Result<Option<IssueSourceContext>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        read_issue_source_state(&app.state::<AppState>(), &run_id, &issue_id)
    })
    .await
    .map_err(|_| AppError::Io("source context could not be read".into()))?
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn open_issue_source(
    app: AppHandle,
    run_id: String,
    issue_id: String,
) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        open_issue_source_state(&app.state::<AppState>(), &run_id, &issue_id)
    })
    .await
    .map_err(|_| AppError::Process("source editor could not be launched".into()))?
}

fn read_issue_source_state(
    state: &AppState,
    run_id: &str,
    issue_id: &str,
) -> Result<Option<IssueSourceContext>, AppError> {
    let Some(source) = IssueSource::resolve(state, run_id, issue_id)? else {
        return Ok(None);
    };
    let (text, local_path) = if source.is_archive() {
        let archive = open_verified_path(&source.project_root, false)?;
        let text = crate::import::zip::read_zip_member(
            archive,
            &source.location.file_path,
            source.file.size_bytes,
        )?
        .map(|bytes| decode_snapshot(&bytes, &source.file))
        .transpose()?
        .flatten();
        (text, None)
    } else {
        let path = contained_source_path(&source.project_root, &source.location.file_path)?;
        let file = open_verified_path(&path, false)?;
        (read_snapshot(&file, &source.file)?, Some(path))
    };
    let Some(text) = text else {
        return Ok(Some(source.stale()));
    };
    let Some(mut excerpt) = source.excerpt(&text) else {
        return Ok(None);
    };
    excerpt.can_open_natively = local_path
        .as_deref()
        .is_some_and(native_editor::available_for)
        && !source.contains_sensitive_content(&text);
    Ok(Some(excerpt))
}

fn open_issue_source_state(state: &AppState, run_id: &str, issue_id: &str) -> Result<(), AppError> {
    let unavailable = || AppError::Validation("native source editor is unavailable".into());
    let source = IssueSource::resolve(state, run_id, issue_id)?.ok_or_else(unavailable)?;
    if source.is_archive() {
        return Err(unavailable());
    }
    let path = contained_source_path(&source.project_root, &source.location.file_path)?;
    if !native_editor::available_for(&path) {
        return Err(unavailable());
    }
    let file = open_verified_path(&path, true)?;
    let text = read_snapshot(&file, &source.file)?
        .ok_or_else(|| AppError::Validation("source file changed since the scan".into()))?;
    if source.contains_sensitive_content(&text) || source.excerpt(&text).is_none() {
        return Err(unavailable());
    }
    native_editor::open(&path)
}

struct IssueSource {
    project_root: PathBuf,
    location: IssueLocation,
    file: FileRecord,
}

impl IssueSource {
    fn resolve(state: &AppState, run_id: &str, issue_id: &str) -> Result<Option<Self>, AppError> {
        let (run, location, file) = {
            let store = state.storage()?;
            let run = store
                .get_scan_run(run_id)?
                .ok_or_else(|| AppError::Validation("scan run does not exist".into()))?;
            if run.status != ScanStatus::Completed {
                return Err(AppError::Validation(
                    "source context requires a completed scan run".into(),
                ));
            }
            let location = store
                .get_issue_location(run_id, issue_id)?
                .ok_or_else(|| AppError::Validation("issue does not belong to this scan".into()))?;
            let Some(file) = store.get_file_for_run(run_id, &location.file_path)? else {
                return Ok(None);
            };
            (run, location, file)
        };
        if file.project_id != run.project_id {
            return Err(invalid_source_path());
        }
        if location.start_line == 0
            || file.excluded_reason.is_some()
            || !matches!(file.encoding.as_str(), "utf-8" | "utf-16le" | "utf-16be")
            || file.size_bytes > MAX_SOURCE_FILE_BYTES
        {
            return Ok(None);
        }
        let project_root = state.project_path(&run.project_id)?;
        safe_metadata(&project_root)?;
        if !project_root.is_absolute() || fs::canonicalize(&project_root)? != project_root {
            return Err(invalid_source_path());
        }
        Ok(Some(Self {
            project_root,
            location,
            file,
        }))
    }

    fn is_archive(&self) -> bool {
        self.project_root.is_file()
            && self
                .project_root
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    }

    fn stale(&self) -> IssueSourceContext {
        IssueSourceContext {
            text: String::new(),
            start_line: self.location.start_line,
            end_line: self.location.end_line,
            stale: true,
            redacted: false,
            truncated: false,
            can_open_natively: false,
        }
    }

    fn excerpt(&self, text: &str) -> Option<IssueSourceContext> {
        let focus = self.location.start_line;
        let first = focus.saturating_sub(CONTEXT_LINES).max(1);
        let line_count = text.lines().count() as u32;
        if focus > line_count {
            return None;
        }
        let desired_last = self
            .location
            .end_line
            .max(focus)
            .saturating_add(CONTEXT_LINES);
        let last = desired_last
            .min(first + MAX_EXCERPT_LINES - 1)
            .min(line_count);
        let mut redacted = false;
        let mut truncated = desired_last.min(line_count) > last;
        let mut lines = Vec::with_capacity((last - first + 1) as usize);
        for (line, sensitive) in self
            .source_lines(text)
            .take(last as usize)
            .skip((first - 1) as usize)
        {
            if sensitive {
                lines.push("[疑似凭据已隐藏]".to_owned());
                redacted = true;
                continue;
            }
            let mut chars = line.chars();
            let mut preview = chars.by_ref().take(MAX_LINE_CHARS).collect::<String>();
            if chars.next().is_some() {
                preview.push('…');
                truncated = true;
            }
            lines.push(preview);
        }
        Some(IssueSourceContext {
            text: lines.join("\n"),
            start_line: first,
            end_line: last,
            stale: false,
            redacted,
            truncated,
            can_open_natively: false,
        })
    }

    fn contains_sensitive_content(&self, text: &str) -> bool {
        self.source_lines(text).any(|(_, sensitive)| sensitive)
    }

    fn source_lines<'a>(&'a self, text: &'a str) -> impl Iterator<Item = (&'a str, bool)> {
        let credential_file = is_credential_path(&self.location.file_path);
        let marked_secret = self.location.rule_id == HARDCODED_SECRET_RULE_ID;
        let mut inside_private_key = false;
        text.lines().enumerate().map(move |(index, line)| {
            if line.contains("-----BEGIN ") && line.contains("PRIVATE KEY-----") {
                inside_private_key = true;
            }
            let line_no = index as u32 + 1;
            let sensitive = credential_file
                || inside_private_key
                || (marked_secret
                    && (self.location.start_line..=self.location.end_line).contains(&line_no))
                || may_expose_credential(line);
            if line.contains("-----END ") && line.contains("PRIVATE KEY-----") {
                inside_private_key = false;
            }
            (line, sensitive)
        })
    }
}

fn is_credential_path(relative: &str) -> bool {
    let name = relative
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || matches!(
            name.as_str(),
            ".npmrc" | ".netrc" | "credentials" | "id_rsa" | "id_ed25519"
        )
        || Path::new(&name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "pem" | "key" | "p12" | "pfx"))
}

fn invalid_source_path() -> AppError {
    AppError::Validation("source path is outside the scanned project or is unsafe".into())
}

fn safe_metadata(path: &Path) -> Result<Metadata, AppError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_symlink() {
        return Err(invalid_source_path());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_source_path());
        }
    }
    Ok(metadata)
}

fn contained_source_path(root: &Path, relative: &str) -> Result<PathBuf, AppError> {
    if !safe_metadata(root)?.is_dir() {
        return Err(invalid_source_path());
    }
    let parts = relative.split('/');
    if parts.clone().any(|part| {
        matches!(part, "" | "." | "..")
            || part.contains(['\\', ':', '\0'])
            || part.ends_with([' ', '.'])
    }) {
        return Err(invalid_source_path());
    }
    let mut target = root.to_path_buf();
    for part in parts {
        target.push(part);
        safe_metadata(&target)?;
    }
    let normalized = target.canonicalize()?;
    if normalized != target
        || !normalized.starts_with(root)
        || !safe_metadata(&normalized)?.is_file()
    {
        return Err(invalid_source_path());
    }
    Ok(normalized)
}

fn open_verified_path(path: &Path, lock_for_editor: bool) -> Result<File, AppError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    if lock_for_editor {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        options.share_mode(FILE_SHARE_READ);
    }
    #[cfg(not(windows))]
    let _ = lock_for_editor;
    let file = options.open(path)?;
    if !file.metadata()?.is_file() || !native_editor::handle_matches_path(&file, path)? {
        return Err(invalid_source_path());
    }
    Ok(file)
}

fn read_snapshot(file: &File, expected: &FileRecord) -> Result<Option<String>, AppError> {
    if file.metadata()?.len() != expected.size_bytes {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(expected.size_bytes.min(MAX_SOURCE_FILE_BYTES) + 1)
        .read_to_end(&mut bytes)?;
    decode_snapshot(&bytes, expected)
}

fn decode_snapshot(bytes: &[u8], expected: &FileRecord) -> Result<Option<String>, AppError> {
    if bytes.len() as u64 != expected.size_bytes {
        return Ok(None);
    }
    let actual_hash = format!("{:x}", Sha256::digest(bytes));
    // Earlier scans persisted only the first 32 SHA-256 hex digits.
    let snapshot_hash = if expected.content_hash.len() == 32 {
        &actual_hash[..32]
    } else {
        actual_hash.as_str()
    };
    if snapshot_hash != expected.content_hash {
        return Ok(None);
    }
    let decoded = decode_bytes(bytes)?;
    Ok((!decoded.is_binary && decoded.encoding == expected.encoding).then_some(decoded.text))
}

#[cfg(windows)]
mod native_editor {
    use super::{AppError, File, Path, PathBuf};
    use std::ffi::{OsStr, OsString, c_void};
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
    use std::process::{Command, Stdio};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
        fn GetFinalPathNameByHandleW(
            file: *mut c_void,
            buffer: *mut u16,
            size: u32,
            flags: u32,
        ) -> u32;
    }

    fn windows_path(call: impl FnOnce(*mut u16, u32) -> u32) -> Option<PathBuf> {
        let mut buffer = vec![0; 32_768];
        let length = call(buffer.as_mut_ptr(), buffer.len() as u32) as usize;
        (length > 0 && length < buffer.len()).then(|| OsString::from_wide(&buffer[..length]).into())
    }

    fn notepad() -> Option<PathBuf> {
        // The OS resolves its protected system directory without environment-variable or PATH lookups.
        let system = windows_path(|buffer, size| unsafe { GetSystemDirectoryW(buffer, size) })?;
        let exe = system.join("notepad.exe");
        (exe.is_absolute() && exe.is_file()).then_some(exe)
    }

    fn editor_path(path: &Path) -> Option<&OsStr> {
        use std::path::{Component, Prefix};
        let Component::Prefix(prefix) = path.components().next()? else {
            return None;
        };
        if !matches!(prefix.kind(), Prefix::VerbatimDisk(_)) {
            return None;
        }
        Some(OsStr::new(path.to_str()?.strip_prefix(r"\\?\")?))
    }

    pub fn handle_matches_path(file: &File, path: &Path) -> Result<bool, AppError> {
        // Query the actual open file before reading; reparse points can change after preflight.
        let actual = windows_path(|buffer, size| unsafe {
            GetFinalPathNameByHandleW(file.as_raw_handle().cast(), buffer, size, 0)
        })
        .ok_or_else(|| AppError::Io("opened source path could not be verified".into()))?;
        Ok(actual == path)
    }

    pub fn available_for(path: &Path) -> bool {
        editor_path(path).is_some() && notepad().is_some()
    }

    pub fn open(path: &Path) -> Result<(), AppError> {
        let unavailable = || AppError::Process("Windows Notepad is unavailable".into());
        let exe = notepad().ok_or_else(unavailable)?;
        let path = editor_path(path).ok_or_else(unavailable)?;
        let system = exe.parent().ok_or_else(unavailable)?;
        Command::new(&exe)
            .arg(path)
            .current_dir(system)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| AppError::Process("Windows Notepad could not be opened".into()))?;
        Ok(())
    }
}

#[cfg(not(windows))]
mod native_editor {
    use super::{AppError, File, Path, fs};

    pub fn handle_matches_path(file: &File, path: &Path) -> Result<bool, AppError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let opened = file.metadata()?;
            let expected = fs::symlink_metadata(path)?;
            Ok(opened.dev() == expected.dev()
                && opened.ino() == expected.ino()
                && path.canonicalize()? == path)
        }
        #[cfg(not(unix))]
        {
            let _ = file;
            Ok(fs::canonicalize(path)? == path)
        }
    }

    pub fn available_for(_: &Path) -> bool {
        false
    }

    pub fn open(_: &Path) -> Result<(), AppError> {
        Err(AppError::Process(
            "native source editor is unavailable".into(),
        ))
    }
}
