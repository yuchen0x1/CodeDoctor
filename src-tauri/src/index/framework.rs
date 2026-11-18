#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn tempdir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "codedoctor-framework-test-{}-{}",
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
    fn detects_framework_markers_from_manifests() {
        let root = tempdir();
        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"react":"1"}}"#,
        )
        .unwrap();
        let frameworks = detect_frameworks(&root);
        assert!(frameworks.iter().any(|name| name == "React"));
    }

    #[test]
    fn ignores_framework_names_in_unrelated_package_metadata() {
        let root = tempdir();
        fs::write(
            root.join("package.json"),
            r#"{"name":"reactive-dashboard","description":"uses react concepts"}"#,
        )
        .unwrap();
        let frameworks = detect_frameworks(&root);
        assert!(!frameworks.iter().any(|name| name == "React"));
    }

    #[test]
    fn ignores_pyproject_dependency_prefixes() {
        let root = tempdir();
        fs::write(
            root.join("pyproject.toml"),
            "dependencies = [\"djangoish\", \"pytest-extra\"]\n",
        )
        .unwrap();
        let frameworks = detect_frameworks(&root);
        assert!(!frameworks.iter().any(|name| name == "Django"));
        assert!(!frameworks.iter().any(|name| name == "Pytest"));
    }

    #[test]
    fn detects_frameworks_from_indexed_nested_manifests() {
        let files = [
            ("apps/web/package.json", r#"{"dependencies":{"react":"1"}}"#),
            ("apps/api/pyproject.toml", "dependencies = [\"fastapi\"]\n"),
            ("apps/desktop/Cargo.toml", "[package]\n"),
        ];
        assert_eq!(
            detect_indexed_frameworks(files.into_iter()),
            ["FastAPI", "Python", "React", "Rust/Cargo"]
        );
    }
}
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

fn read_manifest(path: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    let mut contents = String::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut contents)
        .ok()?;
    (contents.len() as u64 <= MAX_MANIFEST_BYTES).then_some(contents)
}

pub fn detect_frameworks(root: &Path) -> Vec<String> {
    detect_markers(
        |name| read_manifest(&root.join(name)).map(Cow::Owned),
        |name| root.join(name).exists(),
        fs::read_dir(root).is_ok_and(|entries| {
            entries
                .flatten()
                .any(|entry| is_dotnet_marker(&entry.path()))
        }),
    )
}

pub(crate) fn detect_indexed_frameworks<'a>(
    files: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let mut directories = BTreeMap::<&str, BTreeMap<&str, &str>>::new();
    for (path, text) in files {
        let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
        directories.entry(directory).or_default().insert(name, text);
    }
    let mut found = Vec::new();
    for files in directories.into_values() {
        found.extend(detect_markers(
            |name| {
                files
                    .get(name)
                    .filter(|text| text.len() as u64 <= MAX_MANIFEST_BYTES)
                    .map(|text| Cow::Borrowed(*text))
            },
            |name| files.contains_key(name),
            files.keys().any(|name| is_dotnet_marker(Path::new(name))),
        ));
    }
    found.sort();
    found.dedup();
    found
}

fn is_dotnet_marker(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("sln") || extension.eq_ignore_ascii_case("csproj")
    })
}

fn detect_markers<'a>(
    manifest: impl Fn(&str) -> Option<Cow<'a, str>>,
    exists: impl Fn(&str) -> bool,
    dotnet: bool,
) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(contents) = manifest("package.json") {
        let mut dependency_names = Vec::new();
        if let Ok(value) = serde_json::from_str::<Value>(&contents) {
            for section in [
                "dependencies",
                "devDependencies",
                "peerDependencies",
                "optionalDependencies",
            ] {
                if let Some(object) = value.get(section).and_then(Value::as_object) {
                    dependency_names.extend(object.keys().map(|key| key.to_ascii_lowercase()));
                }
            }
        }
        for (needle, framework) in [
            ("react", "React"),
            ("next", "Next.js"),
            ("vue", "Vue"),
            ("nuxt", "Nuxt"),
            ("svelte", "Svelte"),
            ("angular", "Angular"),
            ("express", "Express"),
            ("electron", "Electron"),
        ] {
            if dependency_names.iter().any(|name| {
                name == needle
                    || name.starts_with(&format!("{needle}-"))
                    || name.starts_with(&format!("@{needle}/"))
            }) {
                found.push(framework.to_owned());
            }
        }
    }
    if let Some(contents) = manifest("pyproject.toml") {
        for (needle, framework) in [
            ("django", "Django"),
            ("flask", "Flask"),
            ("fastapi", "FastAPI"),
            ("pytest", "Pytest"),
        ] {
            if contents
                .lines()
                .any(|line| dependency_token_in_manifest_line(line, needle))
            {
                found.push(framework.to_owned());
            }
        }
    }
    if exists("Cargo.toml") {
        found.push("Rust/Cargo".to_owned());
    }
    if exists("go.mod") {
        found.push("Go Modules".to_owned());
    }
    if exists("package-lock.json") {
        found.push("npm".to_owned());
    } else if exists("pnpm-lock.yaml") {
        found.push("pnpm".to_owned());
    } else if exists("yarn.lock") {
        found.push("Yarn".to_owned());
    }
    if exists("pyproject.toml") || exists("requirements.txt") {
        found.push("Python".to_owned());
    }
    if dotnet {
        found.push(".NET".to_owned());
    }
    found.sort();
    found.dedup();
    found
}

fn dependency_token_in_manifest_line(line: &str, needle: &str) -> bool {
    let line = line
        .split('#')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(relative) = line[search_from..].find(needle) {
        let start = search_from + relative;
        let end = start + needle.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        let identifier = |character: Option<char>| {
            character
                .is_some_and(|value| value.is_ascii_alphanumeric() || value == '_' || value == '-')
        };
        if !identifier(before) && !identifier(after) {
            return true;
        }
        search_from = end;
        if search_from >= line.len() {
            break;
        }
    }
    false
}
