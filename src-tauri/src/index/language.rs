#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_source_extensions() {
        assert_eq!(
            language_for_path(std::path::Path::new("main.ts")),
            "TypeScript"
        );
        assert_eq!(language_for_path(std::path::Path::new("main.py")), "Python");
        assert_eq!(
            language_for_path(std::path::Path::new("unknown.bin")),
            "Unknown"
        );
    }
}
use std::path::Path;

pub fn language_for_path(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let language = match extension.as_str() {
        "py" | "pyw" | "pyi" => "Python",
        "js" | "mjs" | "cjs" => "JavaScript",
        "ts" | "mts" | "cts" => "TypeScript",
        "tsx" => "TSX",
        "jsx" => "JSX",
        "java" => "Java",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" => "C++",
        "cs" => "C#",
        "go" => "Go",
        "rs" => "Rust",
        "php" => "PHP",
        "kt" | "kts" => "Kotlin",
        "swift" => "Swift",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "scss" => "SCSS",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "sql" => "SQL",
        "sh" | "bash" => "Shell",
        "ps1" => "PowerShell",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "xml" => "XML",
        _ if name.eq_ignore_ascii_case("dockerfile") => "Dockerfile",
        _ => "Unknown",
    };
    language.to_owned()
}
