#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn tempdir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "codedoctor-test-{}-{}",
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
    fn canonical_project_path_accepts_chinese_directory_and_rejects_file() {
        let root = tempdir();
        let project = root.join("中文项目");
        fs::create_dir(&project).unwrap();
        let canonical = canonical_project_path(&project).unwrap();
        assert_eq!(canonical.file_name().unwrap().to_string_lossy(), "中文项目");

        let file = project.join("main.rs");
        fs::write(&file, "fn main() {}").unwrap();
        assert!(canonical_project_path(&file).is_err());
    }

    #[test]
    fn traversal_is_normalized_and_default_directories_are_excluded() {
        let root = tempdir();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("node_modules").join("ignored.js"), "ignored").unwrap();

        let files = collect_files(&root).unwrap();
        assert!(files.iter().any(|file| file.relative_path == "src/main.rs"));
        assert!(
            !files
                .iter()
                .any(|file| file.relative_path.contains("node_modules"))
        );
    }

    #[test]
    fn user_excluded_directories_are_applied_without_changing_default_exclusions() {
        let root = tempdir();
        fs::create_dir_all(root.join("generated")).unwrap();
        fs::create_dir_all(root.join("src").join("generated")).unwrap();
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("generated").join("out.rs"), "generated").unwrap();
        fs::write(
            root.join("src").join("generated").join("out.rs"),
            "generated",
        )
        .unwrap();
        fs::write(root.join("node_modules").join("dep.js"), "dependency").unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();

        let (files, excluded) =
            collect_files_with_extra_exclusions(&root, &["generated".to_owned()]).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "src/main.rs");
        assert!(
            excluded
                .iter()
                .any(|item| item.relative_path == "generated")
        );
        assert!(
            excluded
                .iter()
                .any(|item| item.relative_path == "src/generated")
        );
        assert!(
            excluded
                .iter()
                .any(|item| item.relative_path == "node_modules")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlink_escape_is_not_followed() {
        use std::os::unix::fs::symlink;
        let root = tempdir();
        let outside =
            std::env::temp_dir().join(format!("codedoctor-outside-{}", std::process::id()));
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.rs"), "secret").unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        let files = collect_files(&root).unwrap();
        assert!(
            !files
                .iter()
                .any(|file| file.relative_path.contains("secret.rs"))
        );
    }
}
use crate::error::AppError;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "build",
    "dist",
    "target",
    "out",
    "coverage",
    ".cache",
    "__pycache__",
    ".venv",
    "venv",
    "env",
    ".idea",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedPath {
    pub relative_path: String,
    pub reason: String,
}

pub fn canonical_project_path(path: impl AsRef<Path>) -> Result<PathBuf, AppError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AppError::Validation(
            "project path does not exist".to_owned(),
        ));
    }
    let canonical = fs::canonicalize(path)?;
    if !canonical.is_dir() {
        return Err(AppError::Validation(
            "project path must be a directory".to_owned(),
        ));
    }
    Ok(canonical)
}

pub fn normalize_relative_path(path: &Path) -> Option<String> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(value) => {
                components.push(value.to_string_lossy().into_owned())
            }
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    return None;
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return None,
        }
    }
    if components.is_empty() {
        None
    } else {
        Some(components.join("/"))
    }
}

pub fn collect_files(root: impl AsRef<Path>) -> Result<Vec<DiscoveredFile>, AppError> {
    Ok(collect_files_with_exclusions(root)?.0)
}

pub fn collect_files_with_exclusions(
    root: impl AsRef<Path>,
) -> Result<(Vec<DiscoveredFile>, Vec<ExcludedPath>), AppError> {
    collect_files_with_extra_exclusions(root, &[])
}

pub(crate) fn normalize_excluded_directories(names: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = names
        .iter()
        .map(|name| name.trim())
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 512
                && name.chars().count() <= 128
                && !matches!(*name, "." | "..")
                && !name.contains(['/', '\\', ':'])
                && !name.chars().any(char::is_control)
        })
        .map(str::to_lowercase)
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

pub fn collect_files_with_extra_exclusions(
    root: impl AsRef<Path>,
    extra_ignored_dirs: &[String],
) -> Result<(Vec<DiscoveredFile>, Vec<ExcludedPath>), AppError> {
    collect_files_checked(root, extra_ignored_dirs, || Ok(()))
}

pub(crate) fn collect_files_checked(
    root: impl AsRef<Path>,
    extra_ignored_dirs: &[String],
    check: impl Fn() -> Result<(), AppError>,
) -> Result<(Vec<DiscoveredFile>, Vec<ExcludedPath>), AppError> {
    let root = canonical_project_path(root)?;
    let mut files = Vec::new();
    let mut excluded = Vec::new();
    let extra = normalize_excluded_directories(extra_ignored_dirs);
    visit_directory(&root, &root, &mut files, &mut excluded, &extra, &check)?;
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    excluded.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok((files, excluded))
}

fn visit_directory(
    root: &Path,
    directory: &Path,
    files: &mut Vec<DiscoveredFile>,
    excluded: &mut Vec<ExcludedPath>,
    extra_ignored_dirs: &[String],
    check: &impl Fn() -> Result<(), AppError>,
) -> Result<(), AppError> {
    check()?;
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        check()?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .ok()
            .and_then(normalize_relative_path);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            excluded.push(ExcludedPath {
                relative_path: relative
                    .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned()),
                reason: "symbolic link is excluded".to_owned(),
            });
            continue;
        }
        if metadata.is_dir() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let default_ignored = DEFAULT_IGNORED_DIRS.iter().any(|ignored| *ignored == name);
            if default_ignored || extra_ignored_dirs.binary_search(&name).is_ok() {
                excluded.push(ExcludedPath {
                    relative_path: relative.unwrap_or_default(),
                    reason: if default_ignored {
                        "default ignored directory"
                    } else {
                        "user excluded directory"
                    }
                    .to_owned(),
                });
                continue;
            }
            visit_directory(root, &path, files, excluded, extra_ignored_dirs, check)?;
        } else if metadata.is_file() {
            let Some(relative_path) = relative else {
                continue;
            };
            files.push(DiscoveredFile {
                relative_path,
                absolute_path: path,
                size_bytes: metadata.len(),
            });
        }
    }
    Ok(())
}
