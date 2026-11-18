use super::AnalyzerMetadata;
use std::path::PathBuf;
pub fn discover() -> Vec<AnalyzerMetadata> {
    [
        ("ruff", "Ruff", true),
        ("eslint", "ESLint", false),
        ("typescript", "TypeScript", false),
        ("golangci", "golangci-lint", false),
        ("clippy", "Clippy", false),
    ]
    .into_iter()
    .map(|(id, name, safe)| {
        let binary = match id {
            "typescript" => "tsc",
            "golangci" => "golangci-lint",
            "clippy" => "cargo-clippy",
            _ => id,
        };
        AnalyzerMetadata {
            id: id.into(),
            name: name.into(),
            executable: find_executable(binary).map(|p| p.to_string_lossy().into_owned()),
            safe_to_run: safe,
            note: if safe {
                "使用隔离配置，只读扫描。"
            } else {
                "保留输出适配；执行可能加载项目配置、插件或构建脚本，当前只读策略下跳过。"
            }
            .into(),
        }
    })
    .collect()
}

fn find_executable(binary: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        if !dir.is_absolute() {
            continue;
        }
        let file = dir.join(if cfg!(windows) {
            format!("{binary}.exe")
        } else {
            binary.into()
        });
        if file.is_file()
            && let Ok(canonical) = file.canonicalize()
        {
            return Some(canonical);
        }
    }
    None
}
