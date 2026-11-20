//! Internal rule engine. Every rule produces evidence-backed drafts that the
//! runner converts into the shared Issue contract with source `codedoctor`.
//! Pattern-only findings carry lower confidence and are always worded as
//! potential risks needing human confirmation.
pub mod basic;
pub mod comments;
pub mod complexity;
pub mod config;
mod local_quality;
pub mod runner;
pub mod security;
pub(crate) mod source_redaction;

#[cfg(test)]
mod fixture_tests;

use crate::domain::types::{CoverageKind, Issue, IssueFingerprint, Severity};
use crate::parse::ParseResult;
use config::RuleConfig;

/// Inputs one rule check may read.
pub struct RuleContext<'a> {
    pub file: &'a crate::domain::types::FileRecord,
    pub text: &'a str,
    pub parse: Option<&'a ParseResult>,
    pub config: &'a RuleConfig,
}

impl RuleContext<'_> {
    /// Returns the 1-based line's text without the trailing newline.
    pub fn line_text(&self, line: u32) -> &str {
        self.text
            .lines()
            .nth(line.saturating_sub(1) as usize)
            .unwrap_or("")
    }

    pub fn language(&self) -> &str {
        self.file.language.as_str()
    }
}

/// Intermediate finding before the shared Issue contract is applied.
#[derive(Debug, Clone)]
pub struct IssueDraft {
    pub rule_id: String,
    pub category: String,
    pub severity: Severity,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub title: String,
    pub explanation: String,
    pub evidence: String,
    pub impact: String,
    pub recommendation: String,
    pub confidence: f32,
}

impl IssueDraft {
    pub fn new(
        rule_id: &str,
        category: &str,
        severity: Severity,
        start_line: u32,
        end_line: u32,
    ) -> Self {
        Self {
            rule_id: rule_id.to_owned(),
            category: category.to_owned(),
            severity,
            start_line: start_line.max(1),
            start_column: 1,
            end_line: end_line.max(start_line.max(1)),
            end_column: 1,
            title: String::new(),
            explanation: String::new(),
            evidence: String::new(),
            impact: String::new(),
            recommendation: String::new(),
            confidence: 0.7,
        }
    }

    pub fn evidence(mut self, text: impl Into<String>) -> Self {
        self.evidence = clamp(text.into(), 500);
        self
    }

    pub fn title(mut self, text: impl Into<String>) -> Self {
        self.title = clamp(text.into(), 200);
        self
    }

    pub fn explanation(mut self, text: impl Into<String>) -> Self {
        self.explanation = clamp(text.into(), 2000);
        self
    }

    pub fn impact(mut self, text: impl Into<String>) -> Self {
        self.impact = clamp(text.into(), 500);
        self
    }

    pub fn recommendation(mut self, text: impl Into<String>) -> Self {
        self.recommendation = clamp(text.into(), 500);
        self
    }

    pub fn confidence(mut self, value: f32) -> Self {
        self.confidence = value.clamp(0.0, 1.0);
        self
    }

    pub fn columns(mut self, start: u32, end: u32) -> Self {
        self.start_column = start.max(1);
        self.end_column = end.max(self.start_column);
        self
    }
}

pub fn clamp(value: impl AsRef<str>, max_chars: usize) -> String {
    let value = value.as_ref();
    if value.chars().count() <= max_chars {
        value.to_owned()
    } else {
        let mut cut: String = value.chars().take(max_chars).collect();
        cut.push('…');
        cut
    }
}

pub(crate) fn safe_issue_evidence<'a>(rule_id: &str, evidence: &'a str) -> &'a str {
    if rule_id == security::HARDCODED_SECRET_RULE_ID
        || evidence.lines().any(security::may_expose_credential)
        || evidence.contains("PRIVATE KEY-----")
    {
        "疑似凭据已隐藏"
    } else {
        evidence
    }
}

/// One checkable rule. Implementations must stay stateless.
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn category(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    /// Language/coverage gating; default: all languages, any coverage.
    fn supports(&self, language: &str, coverage: CoverageKind) -> bool {
        let _ = (language, coverage);
        true
    }
    fn check(&self, ctx: &RuleContext) -> Vec<IssueDraft>;
}

/// Converts a draft into the shared Issue contract with a stable fingerprint.
pub fn draft_to_issue(draft: &IssueDraft, file_path: &str, coverage: CoverageKind) -> Issue {
    let mut issue = Issue {
        id: String::new(),
        rule_id: draft.rule_id.clone(),
        category: draft.category.clone(),
        severity: draft.severity,
        file_path: file_path.to_owned(),
        start_line: draft.start_line,
        start_column: draft.start_column,
        end_line: draft.end_line,
        end_column: draft.end_column,
        title: draft.title.clone(),
        explanation: draft.explanation.clone(),
        evidence: safe_issue_evidence(&draft.rule_id, &draft.evidence).to_owned(),
        impact: draft.impact.clone(),
        recommendation: draft.recommendation.clone(),
        source: "codedoctor".to_owned(),
        coverage,
        confidence: draft.confidence,
        fingerprint: String::new(),
    };
    issue.fingerprint = IssueFingerprint::for_issue(&issue);
    issue.id = format!("rule-{}", &issue.fingerprint[..20]);
    issue
}

/// Merges drafts that share rule ID, range, and normalized evidence.
pub fn dedup_drafts(drafts: Vec<IssueDraft>) -> Vec<IssueDraft> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let key = (
            draft.rule_id.clone(),
            draft.start_line,
            draft.start_column,
            draft.end_line,
            draft.end_column,
            draft
                .evidence
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
        if seen.insert(key) {
            out.push(draft);
        }
    }
    out
}

/// Languages whose files are plain data/markup: pattern rules that expect
/// executable code skip them to avoid noise.
pub fn is_code_language(language: &str) -> bool {
    !matches!(
        language,
        "JSON" | "YAML" | "XML" | "CSS" | "SCSS" | "Markdown" | "Dockerfile"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_to_issue_builds_stable_fingerprint_and_id() {
        let draft = IssueDraft::new("quality.debug-output", "quality", Severity::Low, 3, 3)
            .title("调试输出")
            .evidence("print('x')")
            .confidence(0.8);
        let a = draft_to_issue(&draft, "a.py", CoverageKind::Ast);
        let b = draft_to_issue(&draft, "a.py", CoverageKind::Ast);
        assert_eq!(a.id, b.id);
        assert!(a.id.starts_with("rule-"));
        assert_eq!(a.source, "codedoctor");
    }

    #[test]
    fn dedup_keeps_first_of_identical_drafts() {
        let make = || {
            IssueDraft::new("quality.long-line", "quality", Severity::Low, 4, 4).evidence("aaaa")
        };
        let out = dedup_drafts(vec![make(), make()]);
        assert_eq!(out.len(), 1);
    }
}
