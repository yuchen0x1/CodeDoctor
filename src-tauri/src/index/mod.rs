#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn tempdir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "codedoctor-index-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn oversized_files_are_excluded() {
        let root = tempdir();
        let path = root.join("huge.txt");
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_SOURCE_FILE_BYTES + 1).unwrap();
        let summary = index_directory(&root).unwrap();
        assert_eq!(summary.source_file_count, 0);
        assert!(
            summary
                .excluded_files
                .iter()
                .any(|item| item.reason.contains("size"))
        );
    }
}
pub mod encoding;
pub mod framework;
pub mod language;

use crate::error::AppError;
use crate::import::path::{ExcludedPath, collect_files_with_exclusions};
use encoding::decode_bytes;
use framework::detect_frameworks;
pub use language::language_for_path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

pub const MAX_SOURCE_FILE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedFile {
    pub relative_path: String,
    pub absolute_path: String,
    pub encoding: String,
    pub size_bytes: u64,
    pub content_hash: String,
    pub language: String,
    pub line_count: u64,
    pub excluded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedFile {
    pub relative_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexSummary {
    pub file_count: u64,
    pub source_file_count: u64,
    pub line_count: u64,
    pub max_directory_depth: u32,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub files: Vec<IndexedFile>,
    pub excluded_files: Vec<ExcludedFile>,
}

pub fn index_directory(root: impl AsRef<Path>) -> Result<IndexSummary, AppError> {
    let root = crate::import::path::canonical_project_path(root)?;
    let (discovered, mut excluded): (Vec<_>, Vec<ExcludedPath>) =
        collect_files_with_exclusions(&root)?;
    let mut files = Vec::new();
    let mut languages = BTreeSet::new();
    let mut line_count = 0_u64;
    let mut source_file_count = 0_u64;
    let mut max_depth = 0_u32;

    for file in discovered {
        max_depth = max_depth.max(file.relative_path.matches('/').count() as u32 + 1);
        let handle = fs::File::open(&file.absolute_path)?;
        let mut bytes = Vec::new();
        handle
            .take(MAX_SOURCE_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        let language = language_for_path(&file.absolute_path);
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let mut encoding = "binary".to_owned();
        let mut lines = 0_u64;
        let mut reason = None;
        if file.size_bytes > MAX_SOURCE_FILE_BYTES || bytes.len() as u64 > MAX_SOURCE_FILE_BYTES {
            reason = Some("file exceeds maximum source size".to_owned());
        } else if let Ok(decoded) = decode_bytes(&bytes) {
            encoding = decoded.encoding;
            if decoded.is_binary {
                reason = Some("binary file".to_owned());
            } else if language == "Unknown" {
                reason = Some("unknown file type".to_owned());
            } else {
                lines = decoded.text.lines().count() as u64;
                line_count += lines;
                source_file_count += 1;
                languages.insert(language.clone());
            }
        } else {
            reason = Some("file encoding could not be decoded".to_owned());
        }
        if let Some(reason_value) = &reason {
            excluded.push(ExcludedPath {
                relative_path: file.relative_path.clone(),
                reason: reason_value.clone(),
            });
        }
        files.push(IndexedFile {
            relative_path: file.relative_path,
            absolute_path: file.absolute_path.to_string_lossy().into_owned(),
            encoding,
            size_bytes: file.size_bytes,
            content_hash: hash,
            language,
            line_count: lines,
            excluded_reason: reason,
        });
    }
    excluded.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(IndexSummary {
        file_count: files.len() as u64,
        source_file_count,
        line_count,
        max_directory_depth: max_depth,
        languages: languages.into_iter().collect(),
        frameworks: detect_frameworks(&root),
        files,
        excluded_files: excluded
            .into_iter()
            .map(|item| ExcludedFile {
                relative_path: item.relative_path,
                reason: item.reason,
            })
            .collect(),
    })
}

pub fn index_path(path: impl AsRef<Path>) -> Result<IndexSummary, AppError> {
    index_directory(path)
}
