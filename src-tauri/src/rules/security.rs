//! Security risk rules. Findings are always potential risks with evidence
//! and confidence — never asserted as confirmed vulnerabilities. The scanner
//! performs no network validation.

use super::{IssueDraft, Rule, RuleContext, clamp, is_code_language};
use crate::domain::types::{CoverageKind, Severity};

const SECRET_KEYWORDS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "secret",
    "secret_key",
    "api_key",
    "apikey",
    "access_token",
    "auth_token",
    "private_key",
    "token",
];

pub(crate) const HARDCODED_SECRET_RULE_ID: &str = "security.hardcoded-secret";

/// Source previews hide likely credential values, including unquoted literals.
pub(crate) fn may_expose_credential(line: &str) -> bool {
    let Some((_, value)) = line.split_once(['=', ':']) else {
        return false;
    };
    if value.trim().is_empty() {
        return false;
    }
    let lower = line.to_ascii_lowercase();
    SECRET_KEYWORDS
        .iter()
        .any(|keyword| lower.contains(keyword))
}

const SENSITIVE_KEYWORDS: &[&str] = &[
    "token", "password", "passwd", "pwd", "secret", "otp", "captcha", "verify", "session",
];

fn confidence_for(coverage: CoverageKind, ast_value: f32, pattern_value: f32) -> f32 {
    if coverage == CoverageKind::Ast {
        ast_value
    } else {
        pattern_value
    }
}

/// Scans line by line, yields (line_no, line, is_comment_only_line).
fn lines_with_marks(text: &str) -> Vec<(u32, &str, bool)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| {
            let t = l.trim_start();
            let comment = t.starts_with("//")
                || t.starts_with('#')
                || t.starts_with("--")
                || t.starts_with('*');
            (i as u32 + 1, l, comment)
        })
        .collect()
}

pub struct HardcodedSecretRule;

impl Rule for HardcodedSecretRule {
    fn id(&self) -> &'static str {
        HARDCODED_SECRET_RULE_ID
    }
    fn category(&self) -> &'static str {
        "security"
    }
    fn default_severity(&self) -> Severity {
        Severity::High
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        for (line_no, line, is_comment) in lines_with_marks(ctx.text) {
            let lower = line.to_lowercase();
            if !SECRET_KEYWORDS.iter().any(|k| lower.contains(k)) {
                continue;
            }
            if lower.contains("getenv")
                || lower.contains("process.env")
                || lower.contains("os.environ")
                || lower.contains(".environment")
            {
                continue;
            }
            // Only quoted literal values count as hardcoded credentials;
            // expressions like `token = str(random())` are skipped.
            let value_part = line
                .split(['=', ':'])
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_end_matches([';', ','])
                .trim();
            let quoted = value_part.len() > 1
                && ((value_part.starts_with('"') && value_part.ends_with('"'))
                    || (value_part.starts_with('\'') && value_part.ends_with('\''))
                    || (value_part.starts_with('`') && value_part.ends_with('`')));
            let inner = value_part
                .trim_start_matches(['"', '\'', '`'])
                .trim_end_matches(['"', '\'', '`'])
                .trim();
            if !quoted || inner.chars().count() < 6 {
                continue;
            }
            if inner.contains("${") || inner.contains('<') || inner.contains('{') {
                continue;
            }
            let real_line = ctx.line_text(line_no);
            drafts.push(
                IssueDraft::new(self.id(), self.category(), self.default_severity(), line_no, line_no)
                    .evidence(clamp(real_line, 200))
                    .title("疑似硬编码密钥/口令")
                    .explanation(format!(
                        "第 {line_no} 行的键值疑似明文凭据（模式匹配，{}；待人工确认，本地不联网校验）。",
                        if is_comment { "出现在注释中" } else { "出现在代码中" }
                    ))
                    .impact("凭据泄露风险；进入版本历史后难以彻底移除。")
                    .recommendation("迁移到环境变量或密钥管理；若已泄露请轮换凭据。")
                    .confidence(0.5),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct SqlConcatRule;

impl Rule for SqlConcatRule {
    fn id(&self) -> &'static str {
        "security.sql-concat"
    }
    fn category(&self) -> &'static str {
        "security"
    }
    fn default_severity(&self) -> Severity {
        Severity::High
    }
    fn supports(&self, language: &str, _coverage: CoverageKind) -> bool {
        is_code_language(language) && language != "HTML"
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        for (line_no, _, _) in lines_with_marks(ctx.text) {
            let line = ctx.line_text(line_no);
            let lower = line.to_lowercase();
            let is_sql = [
                "select ",
                "insert into",
                "update ",
                "delete from",
                "drop table",
            ]
            .iter()
            .any(|k| lower.contains(k));
            if !is_sql {
                continue;
            }
            let concatenated = lower.contains("%s")
                || lower.contains("+ ")
                || lower.contains("${")
                || lower.contains("{")
                || lower.contains(".format(")
                || lower.matches('$').any(|_| true) && lower.contains("$");
            let parameterized = lower.contains("?")
                || lower.contains("$1")
                || lower.contains(":name")
                || lower.contains("prepare")
                || lower.contains("execute(");
            if !concatenated || parameterized {
                continue;
            }
            drafts.push(
                IssueDraft::new(self.id(), self.category(), self.default_severity(), line_no, line_no)
                    .evidence(clamp(line, 200))
                    .title("潜在 SQL 拼接风险")
                    .explanation(format!("第 {line_no} 行的 SQL 语句疑似拼接了变量（模式匹配，待人工确认）；未验证是否真的可注入。"))
                    .impact("若拼接内容来自外部输入，存在 SQL 注入风险。")
                    .recommendation("改用参数化查询/预编译语句；确认拼接来源均为可信常量时可忽略。")
                    .confidence(0.55),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct DangerousExecRule;

const EXEC_CALLS: &[&str] = &[
    "eval",
    "exec",
    "execfile",
    "system",
    "popen",
    "shell_exec",
    "passthru",
    "proc_open",
    "execSync",
    "assert",
];

impl Rule for DangerousExecRule {
    fn id(&self) -> &'static str {
        "security.dangerous-exec"
    }
    fn category(&self) -> &'static str {
        "security"
    }
    fn default_severity(&self) -> Severity {
        Severity::High
    }
    fn supports(&self, language: &str, _coverage: CoverageKind) -> bool {
        is_code_language(language)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        let coverage = ctx
            .parse
            .map(|p| p.coverage)
            .unwrap_or(CoverageKind::Lexical);
        if let Some(parse) = ctx.parse {
            for call in &parse.calls {
                let normalized = call.callee.replace("::", ".");
                let last = normalized.rsplit('.').next().unwrap_or("");
                let hit = EXEC_CALLS.contains(&last)
                    || call.callee.contains("Runtime.getRuntime().exec")
                    || call.callee.contains("child_process")
                    || call.callee.contains("subprocess");
                if !hit {
                    continue;
                }
                let display_name = if EXEC_CALLS.contains(&last) {
                    last
                } else {
                    "命令执行 API"
                };
                drafts.push(
                    IssueDraft::new(self.id(), self.category(), self.default_severity(), call.range.start_line, call.range.end_line)
                        .columns(call.range.start_column, call.range.end_column)
                        .evidence(clamp(&call.callee, 200))
                        .title("潜在不安全命令/动态执行")
                        .explanation(format!(
                            "调用 `{}`（语法级证据，待人工确认）：若执行内容包含外部输入，存在命令注入/任意代码执行风险。",
                            display_name
                        ))
                        .impact("外部输入进入执行通道时可能导致任意代码执行。")
                        .recommendation("避免拼接命令字符串；使用白名单参数与数组形式 API；确认输入来源可信时可忽略。")
                        .confidence(confidence_for(coverage, 0.7, 0.5)),
                );
                if drafts.len() >= 10 {
                    return drafts;
                }
            }
        }
        // lexical fallback for languages without calls
        if coverage == CoverageKind::Lexical {
            for (line_no, _, _) in lines_with_marks(ctx.text) {
                let line = ctx.line_text(line_no);
                let lower = line.to_lowercase();
                let hit = [
                    "os.system",
                    "subprocess.",
                    "shell=true",
                    "eval(",
                    "exec(",
                    "shell_exec",
                    "proc_open",
                    "runtime.getruntime",
                ]
                .iter()
                .any(|k| lower.contains(k));
                if !hit {
                    continue;
                }
                drafts.push(
                    IssueDraft::new(
                        self.id(),
                        self.category(),
                        self.default_severity(),
                        line_no,
                        line_no,
                    )
                    .evidence(clamp(line, 200))
                    .title("潜在不安全命令/动态执行")
                    .explanation("该行疑似调用命令执行/动态求值（词法模式匹配，待人工确认）。")
                    .impact("外部输入进入执行通道时可能导致任意代码执行。")
                    .recommendation("避免拼接命令字符串；使用白名单与数组形式 API。")
                    .confidence(0.5),
                );
                if drafts.len() >= 10 {
                    break;
                }
            }
        }
        drafts
    }
}

pub struct InsecureDeserializationRule;

impl Rule for InsecureDeserializationRule {
    fn id(&self) -> &'static str {
        "security.insecure-deserialization"
    }
    fn category(&self) -> &'static str {
        "security"
    }
    fn default_severity(&self) -> Severity {
        Severity::High
    }
    fn supports(&self, language: &str, _coverage: CoverageKind) -> bool {
        is_code_language(language)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        for (line_no, line, _) in lines_with_marks(ctx.text) {
            let lower = line.to_lowercase();
            let hit = [
                "pickle.loads",
                "pickle.load(",
                "yaml.load(",
                "marshal.loads",
                "unserialize(",
                "readobject(",
                "binaryformatter",
                "deserializ",
            ]
            .iter()
            .any(|k| lower.contains(k));
            if !hit {
                continue;
            }
            // Safe-loader usage (yaml.load(..., Loader=SafeLoader)) cannot be
            // verified here, so findings always stay "待人工确认".
            let real = ctx.line_text(line_no);
            drafts.push(
                IssueDraft::new(self.id(), self.category(), self.default_severity(), line_no, line_no)
                    .evidence(clamp(real, 200))
                    .title("潜在不安全反序列化")
                    .explanation("该行疑似使用存在反序列化风险的 API（模式匹配，待人工确认；如 yaml.load 请确认 Loader 参数）。")
                    .impact("不可信数据反序列化可能导致任意代码执行。")
                    .recommendation("使用安全 API（如 yaml.safe_load、JSON 替代）；确认数据来源可信时可忽略。")
                    .confidence(0.6),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct WeakRandomRule;

impl Rule for WeakRandomRule {
    fn id(&self) -> &'static str {
        "security.weak-random"
    }
    fn category(&self) -> &'static str {
        "security"
    }
    fn default_severity(&self) -> Severity {
        Severity::High
    }
    fn supports(&self, language: &str, _coverage: CoverageKind) -> bool {
        is_code_language(language)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        let lines = lines_with_marks(ctx.text);
        for (line_no, _, _) in &lines {
            let line = ctx.line_text(*line_no).to_lowercase();
            let weak = [
                "math.random",
                "random.random",
                "rand(",
                "mathrandom",
                "mt_rand",
                "threadlocalrandom",
            ]
            .iter()
            .any(|k| line.contains(k));
            if !weak {
                continue;
            }
            let context_sensitive = lines
                .iter()
                .filter(|(l, _, _)| {
                    let delta = (i64::from(*l) - i64::from(*line_no)).abs();
                    delta <= 3
                })
                .any(|(_, l, _)| {
                    let lower = l.to_lowercase();
                    SENSITIVE_KEYWORDS.iter().any(|k| lower.contains(k))
                });
            if !context_sensitive {
                continue;
            }
            drafts.push(
                IssueDraft::new(self.id(), self.category(), self.default_severity(), *line_no, *line_no)
                    .evidence(clamp(ctx.line_text(*line_no), 200))
                    .title("疑似弱随机数用于安全场景")
                    .explanation("附近代码同时出现弱随机源与安全相关关键词（token/密码等），待人工确认用途。")
                    .impact("可预测的随机数会削弱令牌/验证码强度。")
                    .recommendation("安全用途改用密码学安全随机（如 secrets、crypto.getRandomValues、SecureRandom）。")
                    .confidence(0.5),
            );
            if drafts.len() >= 5 {
                break;
            }
        }
        drafts
    }
}

pub struct XssRiskRule;

impl Rule for XssRiskRule {
    fn id(&self) -> &'static str {
        "security.xss-risk"
    }
    fn category(&self) -> &'static str {
        "security"
    }
    fn default_severity(&self) -> Severity {
        Severity::High
    }
    fn supports(&self, language: &str, _coverage: CoverageKind) -> bool {
        matches!(
            language,
            "JavaScript"
                | "TypeScript"
                | "TSX"
                | "JSX"
                | "PHP"
                | "Python"
                | "Vue"
                | "Svelte"
                | "HTML"
        )
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        for (line_no, _, _) in lines_with_marks(ctx.text) {
            let line = ctx.line_text(line_no);
            let lower = line.to_lowercase();
            let sink = lower.contains(".innerhtml")
                || lower.contains("document.write")
                || lower.contains("dangerouslysetinnerhtml")
                || lower.contains("v-html")
                || (lower.contains("echo") && lower.contains('$'));
            let dynamic = lower.contains('+') || lower.contains("${") || lower.contains('{');
            if sink && dynamic && !lower.contains("sanitiz") {
                drafts.push(
                    IssueDraft::new(self.id(), self.category(), self.default_severity(), line_no, line_no)
                        .evidence(clamp(line, 200))
                        .title("潜在 XSS（动态 HTML 注入）")
                        .explanation("该行将动态内容写入 HTML 渲染通道（模式匹配，待人工确认）。")
                        .impact("未净化的外部内容可注入脚本，窃取会话或执行操作。")
                        .recommendation("对动态内容做转义/净化（DOMPurify、htmlspecialchars 等）；确认内容可信时可忽略。")
                        .confidence(0.55),
                );
                if drafts.len() >= 8 {
                    break;
                }
            }
        }
        drafts
    }
}

pub struct PathTraversalRiskRule;

impl Rule for PathTraversalRiskRule {
    fn id(&self) -> &'static str {
        "security.path-traversal"
    }
    fn category(&self) -> &'static str {
        "security"
    }
    fn default_severity(&self) -> Severity {
        Severity::Medium
    }
    fn supports(&self, language: &str, _coverage: CoverageKind) -> bool {
        is_code_language(language)
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let mut drafts = vec![];
        for (line_no, _, _) in lines_with_marks(ctx.text) {
            let line = ctx.line_text(line_no);
            let lower = line.to_lowercase();
            let sink = lower.contains("open(")
                || lower.contains("fs.")
                || lower.contains("new file(")
                || lower.contains("file_get_contents");
            let external = lower.contains("input(")
                || lower.contains("sys.argv")
                || lower.contains("req.")
                || lower.contains("request.")
                || lower.contains("params")
                || lower.contains("query")
                || lower.contains("process.argv");
            if sink && external {
                drafts.push(
                    IssueDraft::new(
                        self.id(),
                        self.category(),
                        self.default_severity(),
                        line_no,
                        line_no,
                    )
                    .evidence(clamp(line, 200))
                    .title("潜在路径拼接风险")
                    .explanation("文件访问疑似直接拼接外部输入路径（模式匹配，待人工确认）。")
                    .impact("路径穿越可能读写任意文件。")
                    .recommendation("校验并规范化路径，限定在预期根目录内；确认输入可信时可忽略。")
                    .confidence(0.5),
                );
                if drafts.len() >= 8 {
                    break;
                }
            }
        }
        drafts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_rule_skips_placeholders_and_env() {
        let cfg = crate::rules::config::RuleConfig::default();
        let file = test_file("Python", "config.py");
        let ctx = RuleContext {
            file: &file,
            text: "db_password = os.environ['DB']\napi_key = '${from_env}'\ntoken = 'ajskdqwk123hjasd'\n",
            parse: None,
            config: &cfg,
        };
        let drafts = HardcodedSecretRule.check(&ctx);
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].start_line, 3);
    }

    pub fn test_file(language: &str, path: &str) -> crate::domain::types::FileRecord {
        use crate::domain::types::{CoverageKind, FileRecord};
        FileRecord {
            id: "t".into(),
            scan_run_id: "t".into(),
            project_id: "p".into(),
            relative_path: path.into(),
            absolute_path: path.into(),
            encoding: "utf-8".into(),
            size_bytes: 0,
            content_hash: String::new(),
            language: language.into(),
            parse_capability: CoverageKind::Lexical,
            excluded_reason: None,
        }
    }
}
