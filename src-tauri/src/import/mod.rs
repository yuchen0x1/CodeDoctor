#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn tempdir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "codedoctor-import-test-{}-{}",
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
    fn import_counts_source_files_and_lines_without_binary_files() {
        let root = tempdir();
        fs::write(root.join("main.py"), "print('a')\nprint('b')\n").unwrap();
        fs::write(root.join("image.bin"), [0_u8, 1, 2, 0, 255]).unwrap();
        let summary = import_directory(&root).unwrap();
        assert_eq!(summary.file_count, 2);
        assert_eq!(summary.source_file_count, 1);
        assert_eq!(summary.line_count, 2);
    }
}
pub mod path;
pub mod zip;

use crate::error::AppError;
use crate::index::{IndexSummary, index_directory};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub use path::{
    DiscoveredFile, ExcludedPath, canonical_project_path, collect_files,
    collect_files_with_exclusions,
};
pub use zip::extract_zip;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project_id: String,
    pub canonical_path: String,
    pub display_name: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub config_files: Vec<String>,
    pub file_count: u64,
    pub source_file_count: u64,
    pub line_count: u64,
    pub max_directory_depth: u32,
}

pub fn import_project_path(path: impl AsRef<Path>) -> Result<ProjectSummary, AppError> {
    let input = path.as_ref();
    if input.is_file()
        && input
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        let archive = canonical_archive_path(input)?;
        let extracted = extract_zip(&archive)?;
        let result = import_directory(&extracted).map(|mut summary| {
            summary.canonical_path = archive.to_string_lossy().into_owned();
            summary.display_name = archive
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "project".to_owned());
            summary.project_id = format!(
                "{:x}",
                sha2::Sha256::digest(archive.to_string_lossy().as_bytes())
            );
            summary
        });
        let _ = std::fs::remove_dir_all(&extracted);
        return result;
    } else {
        return import_directory(input);
    }
}

fn canonical_archive_path(path: &Path) -> Result<std::path::PathBuf, AppError> {
    if !path.exists() {
        return Err(AppError::Validation(
            "project path does not exist".to_owned(),
        ));
    }
    let canonical = std::fs::canonicalize(path)?;
    if !canonical.is_file() {
        return Err(AppError::Validation(
            "project archive must be a file".to_owned(),
        ));
    }
    Ok(canonical)
}

pub fn import_directory(path: impl AsRef<Path>) -> Result<ProjectSummary, AppError> {
    let root = canonical_project_path(path)?;
    let index = index_directory(&root)?;
    Ok(summary_from_index(&root, index))
}

fn summary_from_index(root: &Path, index: IndexSummary) -> ProjectSummary {
    let canonical_path = root.to_string_lossy().into_owned();
    let project_id = format!("{:x}", sha2::Sha256::digest(canonical_path.as_bytes()));
    let mut config_files: Vec<_> = index
        .files
        .iter()
        .filter_map(|file| {
            let name = file.relative_path.rsplit('/').next()?;
            let known = matches!(
                name,
                "package.json"
                    | "pnpm-workspace.yaml"
                    | "pyproject.toml"
                    | "requirements.txt"
                    | "Cargo.toml"
                    | "go.mod"
                    | "go.work"
                    | "pom.xml"
                    | "build.gradle"
                    | "build.gradle.kts"
                    | "CMakeLists.txt"
                    | "composer.json"
                    | "Package.swift"
                    | "tsconfig.json"
                    | "ruff.toml"
                    | ".ruff.toml"
                    | "tauri.conf.json"
            ) || name.ends_with(".csproj")
                || name.ends_with(".sln");
            known.then(|| file.relative_path.clone())
        })
        .collect();
    config_files.sort();
    ProjectSummary {
        project_id,
        canonical_path,
        display_name: root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        languages: index.languages,
        frameworks: index.frameworks,
        config_files,
        file_count: index.file_count,
        source_file_count: index.source_file_count,
        line_count: index.line_count,
        max_directory_depth: index.max_directory_depth,
    }
}

use sha2::Digest;
