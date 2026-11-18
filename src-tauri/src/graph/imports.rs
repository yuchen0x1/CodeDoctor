//! Static import and reference evidence between indexed files.

use crate::domain::types::CoverageKind;
use crate::parse::{ImportInfo, ParsedFile, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub dynamic: bool,
    pub confidence: f32,
    pub range: SourceRange,
    pub coverage: CoverageKind,
}

pub struct EdgeCollection {
    pub edges: Vec<GraphEdge>,
    /// Intra-project-intent imports that could not be resolved.
    pub unresolved: Vec<String>,
}

pub fn collect_edges(files: &[ParsedFile]) -> EdgeCollection {
    let by_path: HashMap<String, &ParsedFile> = files
        .iter()
        .map(|file| (file.file.relative_path.replace('\\', "/"), file))
        .collect();
    let mut edges = Vec::new();
    let mut unresolved = Vec::new();
    let mut declarations = HashMap::new();

    for parsed in files {
        let source = parsed.file.relative_path.replace('\\', "/");
        let language = parsed.file.language.as_str();
        let mut bindings = HashMap::new();
        for import in &parsed.parse.imports {
            let module = import.module.trim();
            let declaration = parsed
                .text
                .get(import.range.start_byte..import.range.end_byte)
                .unwrap_or_default();
            // In 'from . import a, b', the imported names may be modules.
            let local_modules: Vec<String> = if language == "Python"
                && module.starts_with('.')
                && module.bytes().all(|b| b == b'.')
                && !import.names.is_empty()
            {
                import
                    .names
                    .iter()
                    .map(|name| format!("{module}{}", name.name))
                    .collect()
            } else {
                vec![module.to_owned()]
            };
            for module in &local_modules {
                match resolve_import(&source, language, module, import, declaration, &by_path) {
                    Resolution::External => {}
                    Resolution::Missing => unresolved.push(format!("{source} → {module}")),
                    Resolution::Found(target, confidence) if target != source => {
                        edges.push(edge(
                            &source,
                            target,
                            "import",
                            &import.range,
                            parsed.parse.coverage,
                            import.dynamic,
                            confidence,
                        ));
                        if !import.dynamic && local_modules.len() == 1 && module == &import.module {
                            record_bindings(
                                &mut bindings,
                                language,
                                import,
                                declaration,
                                target,
                                confidence,
                            );
                        }
                    }
                    Resolution::Found(_, _) => {}
                }
            }
        }
        collect_references(
            parsed,
            &source,
            &by_path,
            &bindings,
            &mut declarations,
            &mut edges,
        );
    }
    edges.sort_unstable_by(|a, b| {
        (&a.source, &a.target, &a.kind, a.dynamic)
            .cmp(&(&b.source, &b.target, &b.kind, b.dynamic))
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.range.start_byte.cmp(&b.range.start_byte))
    });
    edges.dedup_by(|a, b| a.source == b.source && a.target == b.target && a.kind == b.kind);
    unresolved.sort_unstable();
    unresolved.dedup();
    EdgeCollection { edges, unresolved }
}

fn edge(
    source: &str,
    target: &str,
    kind: &str,
    range: &SourceRange,
    coverage: CoverageKind,
    dynamic: bool,
    confidence: f32,
) -> GraphEdge {
    GraphEdge {
        source: source.to_owned(),
        target: target.to_owned(),
        kind: kind.to_owned(),
        dynamic,
        confidence,
        range: range.clone(),
        coverage,
    }
}

enum Resolution<'a> {
    External,
    Found(&'a str, f32),
    Missing,
}

const JS_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "vue", "svelte", "mjs", "cjs", "mts", "cts",
];

fn ecmascript(language: &str) -> bool {
    matches!(
        language,
        "JavaScript" | "TypeScript" | "TSX" | "JSX" | "Vue" | "Svelte" | "HTML"
    )
}

/// Join lexical paths from the scan index, rejecting paths above its root.
fn join(base: &str, relative: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in base.split('/').chain(relative.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

fn resolve_import<'a>(
    source: &str,
    language: &str,
    module: &str,
    import: &ImportInfo,
    declaration: &str,
    by_path: &'a HashMap<String, &ParsedFile>,
) -> Resolution<'a> {
    if module == "<dynamic>" || (module.is_empty() && import.dynamic) {
        return Resolution::Missing;
    }
    if module.is_empty() {
        return Resolution::External;
    }

    let dir = source.rsplit_once('/').map_or("", |(dir, _)| dir);
    let parent_relative = module.starts_with("./") || module.starts_with("../");
    let mut internal = parent_relative || import.dynamic;
    let mut confidence = if import.dynamic { 0.8 } else { 0.9 };
    let mut candidates = Vec::new();

    match language {
        _ if ecmascript(language) => {
            if parent_relative && let Some(base) = join(dir, module) {
                let last = module.rsplit('/').next().unwrap_or_default();
                candidates.push(base.clone());
                if !last.contains('.') {
                    for ext in JS_EXTENSIONS {
                        candidates.push(format!("{base}.{ext}"));
                    }
                    for ext in JS_EXTENSIONS {
                        candidates.push(format!("{base}/index.{ext}"));
                    }
                } else {
                    // ESM paths in TypeScript often use the emitted JS suffix.
                    let ts_exts: &[&str] = match last.rsplit_once('.').map(|(_, ext)| ext) {
                        Some("js") => &["ts", "tsx"],
                        Some("mjs") => &["mts"],
                        Some("cjs") => &["cts"],
                        _ => &[],
                    };
                    if matches!(language, "TypeScript" | "TSX") {
                        let (stem, _) = base.rsplit_once('.').unwrap_or((&base, ""));
                        for ext in ts_exts {
                            candidates.push(format!("{stem}.{ext}"));
                        }
                    }
                }
            }
        }
        "Python" => {
            let name = module.trim_start_matches('.');
            let levels = module.len() - name.len();
            let rel = name.replace('.', "/");
            if levels > 0 {
                internal = true;
                let mut base = dir;
                for _ in 1..levels {
                    if base.is_empty() {
                        return Resolution::Missing;
                    }
                    base = base.rsplit_once('/').map_or("", |(parent, _)| parent);
                }
                if let Some(path) = join(base, &rel) {
                    python_candidates(&mut candidates, &path);
                }
            } else {
                python_candidates(&mut candidates, &rel);
                if let Some(target) = find_path(&candidates, by_path) {
                    return Resolution::Found(target, confidence);
                }
                // A sibling can be on the Python import path when a file is
                // launched as a script. Keep this weaker than root resolution.
                if !dir.is_empty()
                    && let Some(path) = join(dir, &rel)
                {
                    candidates.clear();
                    python_candidates(&mut candidates, &path);
                    confidence = 0.65;
                }
            }
        }
        "Java" | "C#" => {
            if !module.contains('*') {
                let ext = if language == "Java" { "java" } else { "cs" };
                let rel = module.replace('.', "/");
                for root in ["", "src/", "src/main/java/", "src/test/java/"] {
                    candidates.push(format!("{root}{rel}.{ext}"));
                }
            }
        }
        "Rust" => {
            if let Some(rel) = module.strip_prefix("crate::") {
                internal = true;
                let rel = rel.replace("::", "/");
                candidates.extend([format!("src/{rel}.rs"), format!("src/{rel}/mod.rs")]);
            } else if module == "crate" {
                internal = true;
                for name in &import.names {
                    candidates.extend([
                        format!("src/{}.rs", name.name),
                        format!("src/{}/mod.rs", name.name),
                    ]);
                }
                candidates.extend(["src/lib.rs".to_owned(), "src/main.rs".to_owned()]);
            }
        }
        "PHP" => {
            if let Some(name) = import.names.first() {
                let namespace = module.trim_start_matches('\\').replace('\\', "/");
                let rel = format!("{namespace}/{}.php", name.name);
                candidates.extend([rel.clone(), format!("src/{rel}")]);
            }
        }
        "C" | "C++" => {
            let rel = module.replace('\\', "/");
            internal |= declaration.split_once('"').is_some();
            if internal && !rel.starts_with('/') && !rel.contains(':') {
                candidates.extend(join(dir, &rel));
                candidates.extend(join("", &rel));
            }
        }
        _ => {}
    }

    match find_path(&candidates, by_path) {
        Some(path) => Resolution::Found(path, confidence),
        None if internal => Resolution::Missing,
        None => Resolution::External,
    }
}

fn python_candidates(paths: &mut Vec<String>, base: &str) {
    if base.is_empty() {
        paths.push("__init__.py".to_owned());
    } else {
        paths.extend([format!("{base}.py"), format!("{base}/__init__.py")]);
    }
}

fn find_path<'a>(
    candidates: &[String],
    by_path: &'a HashMap<String, &ParsedFile>,
) -> Option<&'a str> {
    candidates
        .iter()
        .find_map(|path| by_path.get_key_value(path).map(|(key, _)| key.as_str()))
}

#[derive(Clone, Copy)]
struct Binding<'a> {
    target: &'a str,
    symbol: Option<&'a str>,
    confidence: f32,
}

fn record_bindings<'a>(
    bindings: &mut HashMap<&'a str, Option<Binding<'a>>>,
    language: &str,
    import: &'a ImportInfo,
    declaration: &str,
    target: &'a str,
    confidence: f32,
) {
    let mut record = |local: &'a str, symbol: Option<&'a str>| {
        if !local.is_empty() {
            bindings
                .entry(local)
                .and_modify(|entry| *entry = None)
                .or_insert(Some(Binding {
                    target,
                    symbol,
                    confidence,
                }));
        }
    };
    match language {
        _ if ecmascript(language) && declaration.starts_with("import") => {
            for name in &import.names {
                record(
                    name.alias.as_deref().unwrap_or(&name.name),
                    Some(&name.name),
                );
            }
        }
        "Python" if import.range.start_column == 1 && declaration.starts_with("from ") => {
            for name in &import.names {
                record(
                    name.alias.as_deref().unwrap_or(&name.name),
                    Some(&name.name),
                );
            }
        }
        "Python" if import.range.start_column == 1 && declaration.starts_with("import ") => {
            let alias = import.names.first().and_then(|name| name.alias.as_deref());
            record(alias.unwrap_or(&import.module), None);
        }
        _ => {}
    }
}

#[derive(Clone, Copy)]
enum Reference {
    Inheritance,
    Component,
    Call,
}

impl Reference {
    fn kind(self) -> &'static str {
        match self {
            Self::Inheritance => "inheritance",
            Self::Component => "component",
            Self::Call => "call",
        }
    }
}

struct Declarations<'a> {
    classes: HashSet<&'a str>,
    functions: HashSet<&'a str>,
}

impl<'a> From<&'a ParsedFile> for Declarations<'a> {
    fn from(target: &'a ParsedFile) -> Self {
        let python = target.file.language == "Python";
        Self {
            classes: target
                .parse
                .classes
                .iter()
                .filter(|class| !python || class.range.start_column == 1)
                .map(|class| class.name.as_str())
                .collect(),
            functions: target
                .parse
                .functions
                .iter()
                .filter(|function| {
                    function.parent_class.is_none()
                        && if python {
                            function.range.start_column == 1
                        } else {
                            function.is_exported
                        }
                })
                .map(|function| function.name.as_str())
                .collect(),
        }
    }
}

fn collect_references<'a>(
    parsed: &ParsedFile,
    source: &str,
    by_path: &HashMap<String, &'a ParsedFile>,
    bindings: &HashMap<&str, Option<Binding<'a>>>,
    declarations: &mut HashMap<&'a str, Declarations<'a>>,
    edges: &mut Vec<GraphEdge>,
) {
    if bindings.is_empty() {
        return;
    }
    let mut parameters = HashMap::<&str, Vec<&SourceRange>>::new();
    for function in &parsed.parse.functions {
        for name in &function.parameters {
            if bindings.contains_key(name.as_str()) {
                parameters.entry(name).or_default().push(&function.range);
            }
        }
    }
    let mut recorded = HashSet::new();
    let mut record = |name: &str, range: &SourceRange, reference: Reference| {
        let (local, qualified) = name
            .rsplit_once('.')
            .map_or((name, None), |(local, name)| (local, Some(name)));
        let Some(Some(binding)) = bindings.get(local) else {
            return;
        };
        let symbol = match (binding.symbol, qualified) {
            (Some(symbol), None) | (None, Some(symbol)) => symbol,
            _ => return,
        };
        let key = (binding.target, reference.kind());
        if recorded.contains(&key)
            || parameters.get(local).is_some_and(|scopes| {
                scopes.iter().any(|scope| {
                    scope.start_byte <= range.start_byte && range.end_byte <= scope.end_byte
                })
            })
        {
            return;
        }
        let declarations = declarations
            .entry(binding.target)
            .or_insert_with(|| by_path[binding.target].into());
        let matches_declaration = match reference {
            Reference::Inheritance => declarations.classes.contains(symbol),
            Reference::Component => {
                declarations.functions.contains(symbol) || declarations.classes.contains(symbol)
            }
            Reference::Call => declarations.functions.contains(symbol),
        };
        if matches_declaration {
            recorded.insert(key);
            edges.push(edge(
                source,
                binding.target,
                reference.kind(),
                range,
                parsed.parse.coverage,
                false,
                binding.confidence.min(0.75),
            ));
        }
    };

    for class in &parsed.parse.classes {
        if parsed.file.language != "Python" || class.range.start_column == 1 {
            for base in &class.bases {
                record(base, &class.range, Reference::Inheritance);
            }
        }
    }
    for component in &parsed.parse.component_references {
        record(&component.target, &component.range, Reference::Component);
    }
    for call in parsed.parse.calls.iter().filter(|call| !call.dynamic) {
        record(&call.callee, &call.range, Reference::Call);
    }
}
