//! Basic quality rules: naming, unused imports/params, long lines, magic
//! numbers, empty exception handling, debug output residue.

use super::{IssueDraft, Rule, RuleContext, clamp, is_code_language};
use crate::domain::types::{CoverageKind, Severity};
use crate::parse::ImportInfo;
use std::collections::HashSet;

/// Single-character names accepted by convention (loop indices, coords).
const ALLOWED_SINGLE_CHARS: &[char] = &['i', 'j', 'k', 'n', 'x', 'y'];
const MEANINGLESS_NAMES: &[&str] = &[
    "foo", "bar", "baz", "asdf", "qwerty", "tmp1", "temp1", "data1", "list1", "arr1", "obj1",
    "str1", "test1", "thing1", "aaaa",
];
const MAGIC_NUMBER_WHITELIST: &[&str] = &["0", "1", "2", "100", "1000"];

/// Languages where unused-import reporting is reliable. Go unused imports are
/// compile errors, wildcard/side-effect imports cannot be judged.
fn import_binding_language(language: &str) -> bool {
    matches!(
        language,
        "Python" | "JavaScript" | "TypeScript" | "TSX" | "JSX" | "Java" | "Rust" | "C#" | "PHP"
    )
}

/// Resolves the local binding name introduced by one import entry, or `None`
/// for side-effect/wildcard imports that cannot be checked.
fn import_binding<'a>(language: &str, import: &'a ImportInfo) -> Option<String> {
    if import.dynamic {
        return None;
    }
    if let Some(first) = import.names.first() {
        return Some(first.alias.clone().unwrap_or_else(|| first.name.clone()));
    }
    match language {
        "Python" => module_tail(&import.module, '.').map(|s| s.to_owned()),
        "C#" | "PHP" => module_tail(&import.module, if language == "PHP" { '\\' } else { '.' })
            .map(|s| s.to_owned()),
        "Java" | "Rust" | "Go" => None,
        _ => None,
    }
}

fn module_tail(module: &str, sep: char) -> Option<&str> {
    module.rsplit(sep).next().filter(|s| !s.is_empty())
}

/// 判定函数参数是否应忽略（约定前缀、self/cls）。
fn param_is_ignored(language: &str, param: &str) -> bool {
    let p = param.trim();
    p.starts_with('_')
        || matches!(p, "self" | "cls" | "this")
        || (language == "Rust"
            && (p == "self"
                || p == "&self"
                || p == "&mut self"
                || p == "&'a self"
                || p == "&mut self: Self"))
}

pub struct NamingRule;

impl Rule for NamingRule {
    fn id(&self) -> &'static str {
        "basic.naming"
    }
    fn category(&self) -> &'static str {
        "quality"
    }
    fn default_severity(&self) -> Severity {
        Severity::Low
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        coverage == CoverageKind::Ast
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let mut drafts = vec![];
        let mut reported = HashSet::new();
        for identifier in &parse.identifiers {
            let name = &identifier.name;
            let bad = (name.chars().count() == 1
                && !ALLOWED_SINGLE_CHARS.contains(&name.chars().next().unwrap_or(' ')))
                || MEANINGLESS_NAMES.contains(&name.as_str())
                || is_positional_name(name);
            if !bad || !reported.insert(name.clone()) {
                continue;
            }
            let line = identifier.range.start_line;
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    line,
                    identifier.range.end_line,
                )
                .title("命名不清晰")
                .evidence(format!(
                    "标识符 `{name}`（{}:{}）",
                    identifier.range.start_line, identifier.range.start_column
                ))
                .explanation(format!(
                    "标识符 `{name}` 命名过短或无语义，阅读者无法从名称推断其用途。"
                ))
                .impact("降低可读性，后续维护者需要额外追查变量含义。")
                .recommendation("改用描述业务含义的名称；循环内公认的 i/j/k 等惯例名可保留。")
                .confidence(0.7),
            );
            if drafts.len() >= 15 {
                break;
            }
        }
        drafts
    }
}

fn is_positional_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if !(2..=4).contains(&bytes.len()) {
        return false;
    }
    let letters = bytes.iter().take_while(|b| b.is_ascii_lowercase()).count();
    let all_alnum = bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    (1..=2).contains(&letters) && letters < bytes.len() && all_alnum
}

pub struct UnusedImportRule;

impl Rule for UnusedImportRule {
    fn id(&self) -> &'static str {
        "quality.unused-import"
    }
    fn category(&self) -> &'static str {
        "quality"
    }
    fn default_severity(&self) -> Severity {
        Severity::Low
    }
    fn supports(&self, language: &str, coverage: CoverageKind) -> bool {
        import_binding_language(language) && coverage == CoverageKind::Ast
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let language = ctx.language();
        let mut drafts = vec![];
        for import in &parse.imports {
            let Some(binding) = import_binding(language, import) else {
                continue;
            };
            let used = parse.identifiers.iter().any(|id| {
                id.name == binding
                    && !parse.imports.iter().any(|imp| {
                        inside(
                            imp.range.start_byte,
                            imp.range.end_byte,
                            id.range.start_byte,
                        )
                    })
            });
            if used {
                continue;
            }
            drafts.push(
                IssueDraft::new(self.id(), self.category(), self.default_severity(), import.range.start_line, import.range.end_line)
                    .title("疑似未使用的导入")
                    .evidence(format!("导入 `{binding}`（{}）在文件其余部分未出现", import.module))
                    .explanation(format!(
                        "导入的 `{binding}` 没有在本文件其他位置被引用。动态调用、反射、插件注册或模板引擎可能隐式使用导入，无法确认时保留。"
                    ))
                    .impact("无用导入会增加阅读噪音，并可能引入不必要的初始化开销。")
                    .recommendation("确认无隐式引用后移除；保留有业务含义的副作用导入。")
                    .confidence(0.85),
            );
            if drafts.len() >= 20 {
                break;
            }
        }
        drafts
    }
}

fn inside(start: usize, end: usize, offset: usize) -> bool {
    offset >= start && offset < end
}

pub struct UnusedParameterRule;

impl Rule for UnusedParameterRule {
    fn id(&self) -> &'static str {
        "quality.unused-parameter"
    }
    fn category(&self) -> &'static str {
        "quality"
    }
    fn default_severity(&self) -> Severity {
        Severity::Low
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        coverage == CoverageKind::Ast
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let language = ctx.language();
        let mut drafts = vec![];
        for function in &parse.functions {
            let Some(body) = &function.body_range else {
                continue;
            };
            for param in &function.parameters {
                let name = param.trim();
                if param_is_ignored(language, name) || name.is_empty() {
                    continue;
                }
                let used = parse.identifiers.iter().any(|id| {
                    id.name == name
                        && id.range.start_byte >= body.start_byte
                        && id.range.start_byte < body.end_byte
                });
                if used {
                    continue;
                }
                drafts.push(
                    IssueDraft::new(
                        self.id(),
                        self.category(),
                        self.default_severity(),
                        function.range.start_line,
                        function.range.end_line,
                    )
                    .title("疑似未使用的函数参数")
                    .evidence(format!(
                        "函数 `{}` 的参数 `{name}` 在函数体内未出现",
                        function.name
                    ))
                    .explanation("参数未被函数体引用，可能是残留的旧接口参数或占位参数。")
                    .impact("误导调用者以为该参数有作用，增加接口理解成本。")
                    .recommendation(
                        "确认后移除参数，或在参数名前加 `_` 标记有意忽略（保留接口兼容时）。",
                    )
                    .confidence(0.75),
                );
                if drafts.len() >= 20 {
                    break;
                }
            }
            if drafts.len() >= 20 {
                break;
            }
        }
        drafts
    }
}

pub struct LongLineRule;

impl Rule for LongLineRule {
    fn id(&self) -> &'static str {
        "quality.long-line"
    }
    fn category(&self) -> &'static str {
        "quality"
    }
    fn default_severity(&self) -> Severity {
        Severity::Low
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let limit = ctx.config.max_line_length;
        let mut drafts = vec![];
        for (index, line) in ctx.text.lines().enumerate() {
            let length = line.chars().count();
            if length <= limit {
                continue;
            }
            let line_no = index as u32 + 1;
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    line_no,
                    line_no,
                )
                .evidence(clamp(line, 200))
                .title("超长代码行")
                .explanation(format!(
                    "第 {line_no} 行共 {length} 个字符，超过阈值 {limit}。"
                ))
                .impact("横向阅读困难，diff 审查与并排对比体验差。")
                .recommendation("拆分表达式或提取变量/函数；表格数据可考虑单独的数据文件。")
                .confidence(0.95),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct MagicNumberRule;

impl Rule for MagicNumberRule {
    fn id(&self) -> &'static str {
        "quality.magic-number"
    }
    fn category(&self) -> &'static str {
        "quality"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn supports(&self, language: &str, _coverage: CoverageKind) -> bool {
        is_code_language(language)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        let mut reported = HashSet::new();
        for (index, raw_line) in ctx.text.lines().enumerate() {
            let trimmed = raw_line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
                continue;
            }
            let lower = raw_line.to_lowercase();
            if ["const ", "final ", "static ", "#define", "let me", " = ["]
                .iter()
                .any(|k| lower.contains(k))
                || is_constant_declaration(&lower)
            {
                continue;
            }
            for token in
                raw_line.split(|c: char| !(c.is_ascii_digit() || c.is_alphabetic() || c == '_'))
            {
                let token = token.trim_matches(|c: char| !c.is_ascii_digit());
                if token.is_empty()
                    || MAGIC_NUMBER_WHITELIST.contains(&token)
                    || token.len() < 2
                    || !token.chars().next().is_some_and(|c| c.is_ascii_digit())
                {
                    continue;
                }
                let is_version_like = raw_line.contains("version") || raw_line.contains("Version");
                if is_version_like {
                    continue;
                }
                if !reported.insert(token.to_owned()) {
                    continue;
                }
                let line_no = index as u32 + 1;
                drafts.push(
                    IssueDraft::new(
                        self.id(),
                        self.category(),
                        self.default_severity(),
                        line_no,
                        line_no,
                    )
                    .evidence(clamp(raw_line, 200))
                    .title("疑似魔法数字")
                    .explanation(format!(
                        "数值字面量 `{token}` 缺少命名或常量定义，含义需读者推断。"
                    ))
                    .impact("业务含义散落各处，调整时容易遗漏。")
                    .recommendation("提取为有名字的常量或配置项；确认该数值的业务含义后再命名。")
                    .confidence(0.6),
                );
                if drafts.len() >= 5 {
                    return drafts;
                }
            }
        }
        drafts
    }
}

fn is_constant_declaration(lower_line: &str) -> bool {
    // UPPER_CASE = 123 (python) or enum-ish declarations.
    let before_digit = lower_line
        .split(|c: char| c.is_ascii_digit())
        .next()
        .unwrap_or("");
    before_digit.contains(&before_digit.to_uppercase())
        && before_digit
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .count()
            >= 2
        && before_digit.chars().any(|c| c.is_ascii_uppercase())
}

pub struct EmptyCatchRule;

impl Rule for EmptyCatchRule {
    fn id(&self) -> &'static str {
        "quality.empty-catch"
    }
    fn category(&self) -> &'static str {
        "quality"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        coverage == CoverageKind::Ast || coverage == CoverageKind::Structural
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let mut drafts = vec![];
        for control in &parse.control_flow {
            if control.kind != "catch" {
                continue;
            }
            let start = control.range.start_byte.min(ctx.text.len());
            let end = control.range.end_byte.min(ctx.text.len()).max(start);
            let body = &ctx.text[start..end];
            if !is_empty_exception_body(body, ctx.language()) {
                continue;
            }
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    control.range.start_line,
                    control.range.end_line,
                )
                .evidence(clamp(body, 300))
                .title("空的异常处理")
                .explanation("异常被捕获后既未处理也未记录，错误会被静默吞掉。")
                .impact("故障难以定位，业务异常可能无声失败。")
                .recommendation("至少记录日志或添加注释说明为何可安全忽略；必要时向上传递。")
                .confidence(0.8),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

fn is_empty_exception_body(body: &str, language: &str) -> bool {
    let cleaned: String = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| {
            !(l.starts_with("//")
                || l.starts_with('#')
                || l.starts_with("--")
                || l.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let cleaned = cleaned.trim();
    if language == "Python" {
        let without_header: String = cleaned
            .lines()
            .filter(|l| !l.starts_with("except"))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();
        let bare = without_header.trim_end_matches(':').trim();
        matches!(bare, "" | "pass" | "...")
    } else {
        match (cleaned.find('{'), cleaned.rfind('}')) {
            (Some(open), Some(close)) if close > open => cleaned[open + 1..close].trim().is_empty(),
            _ => cleaned.ends_with("{}") || cleaned.ends_with("{ }"),
        }
    }
}

pub struct DebugOutputRule;

const DEBUG_CALLEES: &[(&str, &[&str])] = &[
    ("Python", &["print"]),
    ("JavaScript", &["log", "debug", "info"]),
    ("TypeScript", &["log", "debug", "info"]),
    ("TSX", &["log", "debug", "info"]),
    ("JSX", &["log", "debug", "info"]),
    (
        "Java",
        &[
            "System.out.println",
            "System.out.print",
            "System.err.println",
        ],
    ),
    ("Rust", &["println!", "print!", "dbg!", "eprintln!"]),
    ("Go", &["Println", "Printf"]),
    ("PHP", &["var_dump", "print_r"]),
    ("C#", &["Console.WriteLine", "Console.Write"]),
];

impl Rule for DebugOutputRule {
    fn id(&self) -> &'static str {
        "quality.debug-output"
    }
    fn category(&self) -> &'static str {
        "quality"
    }
    fn default_severity(&self) -> Severity {
        Severity::Low
    }
    fn supports(&self, _language: &str, coverage: CoverageKind) -> bool {
        coverage == CoverageKind::Ast || coverage == CoverageKind::Structural
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let language = ctx.language();
        let Some(patterns) = DEBUG_CALLEES.iter().find(|(l, _)| *l == language) else {
            return vec![];
        };
        let is_test_file = ["test", "spec", "__tests__"]
            .iter()
            .any(|m| ctx.file.relative_path.to_lowercase().contains(m));
        let mut drafts = vec![];
        for call in &parse.calls {
            let matches = patterns.1.iter().any(|pattern| {
                if pattern.ends_with('!') || pattern.contains('.') {
                    call.callee.ends_with(pattern)
                } else {
                    call.callee.rsplit(['.', ':']).next() == Some(*pattern)
                }
            });
            if !matches {
                continue;
            }
            let severity = if is_test_file {
                Severity::Info
            } else {
                self.default_severity()
            };
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    severity,
                    call.range.start_line,
                    call.range.end_line,
                )
                .evidence(clamp(&call.callee, 120))
                .title("调试输出残留")
                .explanation(format!(
                    "调用 `{}` 属于调试/过程输出；{}测试文件中的输出通常可忽略。",
                    call.callee,
                    if is_test_file {
                        "该文件疑似测试文件，"
                    } else {
                        ""
                    }
                ))
                .impact("生产环境输出噪音，可能泄露内部数据结构。")
                .recommendation("移除或改用受控的日志框架与级别。")
                .confidence(0.8),
            );
            if drafts.len() >= 15 {
                break;
            }
        }
        drafts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_name_detection() {
        assert!(is_positional_name("a1"));
        assert!(is_positional_name("ab12"));
        assert!(!is_positional_name("abc1"));
        assert!(!is_positional_name("x"));
    }
}
