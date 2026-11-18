//! Comment governance rules. High comment volume alone is never a finding:
//! API docs, copyright headers, and complex-algorithm explanations must stay.
//! Suspected stale content is always marked 待人工确认.

use super::{IssueDraft, Rule, RuleContext, clamp};
use crate::domain::types::Severity;
use std::collections::HashMap;

/// Strips comment markers and normalizes a comment body.
fn comment_body(raw: &str) -> String {
    let mut t = raw.trim();
    for marker in ["///", "//", "#", "--", "/*", "*/", "<!--", "-->"] {
        if t.starts_with(marker) {
            t = t[marker.len()..].trim_start();
        }
    }
    if t.ends_with("*/") {
        t = t[..t.len() - 2].trim_end();
    }
    t.replace('\t', " ").trim().to_owned()
}

fn looks_like_code(body: &str) -> bool {
    let t = body.trim();
    if t.is_empty() {
        return false;
    }
    let code_starts = [
        "return",
        "if ",
        "if(",
        "for ",
        "for(",
        "while ",
        "while(",
        "switch",
        "case ",
        "try ",
        "catch",
        "else",
        "elif",
        "def ",
        "class ",
        "function ",
        "const ",
        "let ",
        "var ",
        "public ",
        "private ",
        "protected ",
        "static ",
        "import ",
        "from ",
        "use ",
        "require",
        "#include",
        "print",
        "console.",
        "System.",
        "echo ",
        "self.",
        "this.",
        "}",
        "fmt.",
    ];
    let ends_like_code =
        (t.ends_with(';') && (t.contains('(') || t.contains('='))) || t.ends_with('{') || t == "}";
    code_starts.iter().any(|p| t.starts_with(p)) || ends_like_code
}

const OBVIOUS_PREFIXES: &[&str] = &[
    "get ",
    "set ",
    "loop ",
    "increment",
    "decrement",
    "end for",
    "end if",
    "end while",
    "constructor",
    "destructor",
    "initialize",
    "iterate",
];
const OBVIOUS_EXACT: &[&str] = &[
    "初始化",
    "递增",
    "递减",
    "遍历",
    "循环",
    "获取数据",
    "设置值",
    "返回结果",
    "打印",
    "输出结果",
    "结束",
    "构造函数",
    "开始",
    "结束循环",
    "循环结束",
    "getter",
    "setter",
    "done",
    "over",
    "end",
];

const STALE_MARKERS: &[&str] = &["TODO", "FIXME", "XXX", "HACK", "临时", "待办", "占位"];

pub struct CommentedOutCodeRule;

impl Rule for CommentedOutCodeRule {
    fn id(&self) -> &'static str {
        "comments.commented-out-code"
    }
    fn category(&self) -> &'static str {
        "comments"
    }
    fn default_severity(&self) -> Severity {
        Severity::Low
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let mut drafts = vec![];
        for comment in &parse.comments {
            if comment.kind == "doc" {
                continue;
            }
            let body = comment_body(&comment.text);
            if !looks_like_code(&body) {
                continue;
            }
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    comment.range.start_line,
                    comment.range.end_line,
                )
                .evidence(clamp(&comment.text, 300))
                .title("疑似被注释掉的代码")
                .explanation("注释内容看起来是被注释掉的代码（待人工确认）：可能是废弃逻辑、临时调试或备份。")
                .impact("废弃代码堆积会误导阅读，掩盖真实逻辑。")
                .recommendation("确认无用后删除；需要保留历史请依赖版本管理而非注释。")
                .confidence(0.7),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct ObviousCommentRule;

impl Rule for ObviousCommentRule {
    fn id(&self) -> &'static str {
        "comments.obvious"
    }
    fn category(&self) -> &'static str {
        "comments"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let mut drafts = vec![];
        for comment in &parse.comments {
            if comment.kind == "doc" {
                continue;
            }
            let body = comment_body(&comment.text);
            if body.chars().count() > 30 || body.is_empty() {
                continue;
            }
            let lower = body.to_lowercase();
            let obvious = OBVIOUS_PREFIXES.iter().any(|p| lower.starts_with(p))
                || OBVIOUS_EXACT.iter().any(|e| lower == *e);
            if !obvious {
                continue;
            }
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    comment.range.start_line,
                    comment.range.end_line,
                )
                .evidence(clamp(&comment.text, 300))
                .title("疑似显而易见的注释")
                .explanation("注释仅复述代码已明显表达的操作，未提供额外信息（疑似，需人工确认）。")
                .impact("增加阅读噪音，且容易与代码演进脱节。")
                .recommendation("确认后删除，或改写为解释“为什么”的注释。")
                .confidence(0.6),
            );
            if drafts.len() >= 10 {
                break;
            }
        }
        drafts
    }
}

pub struct DuplicateCommentRule;

impl Rule for DuplicateCommentRule {
    fn id(&self) -> &'static str {
        "comments.duplicate-explanation"
    }
    fn category(&self) -> &'static str {
        "comments"
    }
    fn default_severity(&self) -> Severity {
        Severity::Low
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let mut groups: HashMap<String, (u32, u32, u32)> = HashMap::new();
        for comment in &parse.comments {
            if comment.kind == "doc" {
                continue;
            }
            let body = comment_body(&comment.text);
            let normalized: String = body
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            if normalized.chars().count() < 6 {
                continue;
            }
            let entry = groups.entry(normalized).or_insert((
                comment.range.start_line,
                comment.range.end_line,
                0,
            ));
            entry.2 += 1;
        }
        let mut drafts = vec![];
        for (_, (start, end, count)) in groups {
            if count < 3 {
                continue;
            }
            drafts.push(
                IssueDraft::new(self.id(), self.category(), self.default_severity(), start, end)
                    .title("重复说明同一段逻辑的注释")
                    .explanation(format!("同一注释内容在本文件重复出现 {count} 次，疑似复制粘贴的解释或生成残留（待人工确认）。"))
                    .impact("重复注释稀释有效信息，维护时容易漏改。")
                    .recommendation("保留一处准确说明，删除重复项。")
                    .confidence(0.7),
            );
            if drafts.len() >= 5 {
                break;
            }
        }
        drafts
    }
}

pub struct StaleMarkerRule;

impl Rule for StaleMarkerRule {
    fn id(&self) -> &'static str {
        "comments.stale-marker"
    }
    fn category(&self) -> &'static str {
        "comments"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return vec![];
        };
        let mut drafts = vec![];
        for comment in &parse.comments {
            let body = comment_body(&comment.text);
            let Some(marker) = STALE_MARKERS.iter().find(|m| body.contains(*m)) else {
                continue;
            };
            drafts.push(
                IssueDraft::new(
                    self.id(),
                    self.category(),
                    self.default_severity(),
                    comment.range.start_line,
                    comment.range.end_line,
                )
                .evidence(clamp(&comment.text, 300))
                .title(format!("疑似过时/临时备注（{marker}）"))
                .explanation("注释包含 TODO/FIXME/临时 标记，可能已失效或被遗忘（待人工确认，不做直接认定）。")
                .impact("未跟踪的遗留备注会让技术债务不可见。")
                .recommendation("确认后转为任务/工单跟踪，或删除已完成的备注。")
                .confidence(0.9),
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
    fn body_stripping_removes_common_markers() {
        assert_eq!(comment_body("// 加载配置"), "加载配置");
        assert_eq!(comment_body("# 读取文件"), "读取文件");
        assert_eq!(comment_body("/* span */"), "span");
        assert_eq!(comment_body("-- init table"), "init table");
    }

    #[test]
    fn code_detection_and_obvious_sets() {
        assert!(looks_like_code("return total;"));
        assert!(looks_like_code("if (x > 0) {"));
        assert!(looks_like_code("console.log('done');"));
        assert!(!looks_like_code("保持该顺序，业务约束"));
        assert!(!looks_like_code("请勿在此排序，依赖后端约定"));
        assert!(OBVIOUS_EXACT.contains(&"初始化"));
    }
}
