//! Rule runner: applies the default rule set over parsed files with the scan
//! configuration, converts drafts to the shared Issue contract, and dedupes.

use super::basic::{
    DebugOutputRule, EmptyCatchRule, LongLineRule, MagicNumberRule, NamingRule, UnusedImportRule,
    UnusedParameterRule,
};
use super::comments::{
    CommentedOutCodeRule, DuplicateCommentRule, ObviousCommentRule, StaleMarkerRule,
};
use super::complexity::{
    DeepNestingRule, FileOverloadRule, GodClassRule, HighCognitiveRule, HighCyclomaticRule,
    LongFunctionRule, TooManyParamsRule,
};
use super::config::RuleConfig;
use super::local_quality::{UnreachableStatementRule, UnusedLocalBindingRule};
use super::security::{
    DangerousExecRule, HardcodedSecretRule, InsecureDeserializationRule, PathTraversalRiskRule,
    SqlConcatRule, WeakRandomRule, XssRiskRule,
};
use super::{Rule, RuleContext, dedup_drafts, draft_to_issue};
use crate::domain::types::Issue;
use crate::parse::ParsedFile;

/// Runs the internal rule set over one scan's parsed files.
pub struct RuleRunner {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for RuleRunner {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

impl RuleRunner {
    pub fn with_default_rules() -> Self {
        Self {
            rules: vec![
                Box::new(NamingRule),
                Box::new(UnusedImportRule),
                Box::new(UnusedParameterRule),
                Box::new(UnusedLocalBindingRule),
                Box::new(UnreachableStatementRule),
                Box::new(LongLineRule),
                Box::new(MagicNumberRule),
                Box::new(EmptyCatchRule),
                Box::new(DebugOutputRule),
                Box::new(LongFunctionRule),
                Box::new(TooManyParamsRule),
                Box::new(DeepNestingRule),
                Box::new(HighCyclomaticRule),
                Box::new(HighCognitiveRule),
                Box::new(GodClassRule),
                Box::new(FileOverloadRule),
                Box::new(CommentedOutCodeRule),
                Box::new(ObviousCommentRule),
                Box::new(DuplicateCommentRule),
                Box::new(StaleMarkerRule),
                Box::new(HardcodedSecretRule),
                Box::new(SqlConcatRule),
                Box::new(DangerousExecRule),
                Box::new(InsecureDeserializationRule),
                Box::new(WeakRandomRule),
                Box::new(XssRiskRule),
                Box::new(PathTraversalRiskRule),
            ],
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn rule_ids(&self) -> Vec<&'static str> {
        self.rules.iter().map(|r| r.id()).collect()
    }

    pub(crate) fn rules(&self) -> impl Iterator<Item = &dyn Rule> {
        self.rules.iter().map(Box::as_ref)
    }

    /// Runs all enabled rules; returns issues sorted by file then position.
    pub fn run(&self, files: &[ParsedFile], config: &RuleConfig) -> Vec<Issue> {
        let mut issues = Vec::new();
        for parsed in files {
            let ctx = RuleContext {
                file: &parsed.file,
                text: &parsed.text,
                parse: Some(&parsed.parse),
                config,
            };
            let coverage = parsed.parse.coverage;
            for rule in &self.rules {
                if !config.is_rule_enabled(rule.id()) {
                    continue;
                }
                if !rule.supports(ctx.language(), coverage) {
                    continue;
                }
                for draft in dedup_drafts(rule.check(&ctx)) {
                    issues.push(draft_to_issue(&draft, &parsed.file.relative_path, coverage));
                }
            }
        }
        issues.sort_by(|a, b| {
            (&a.file_path, a.start_line, a.start_column, &a.rule_id).cmp(&(
                &b.file_path,
                b.start_line,
                b.start_column,
                &b.rule_id,
            ))
        });
        issues
    }
}

/// Convenience for single-file checks used by tests and the scan host.
pub fn run_on_file(
    file: &crate::domain::types::FileRecord,
    text: &str,
    config: &RuleConfig,
) -> Vec<Issue> {
    let parse = crate::parse::ParserRegistry::parse(file, text);
    let parsed = ParsedFile {
        file: file.clone(),
        text: text.to_owned(),
        parse,
    };
    RuleRunner::with_default_rules().run(std::slice::from_ref(&parsed), config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{CoverageKind, FileRecord};

    fn file(language: &str, path: &str) -> FileRecord {
        FileRecord {
            id: "t".into(),
            scan_run_id: "run".into(),
            project_id: "p".into(),
            relative_path: path.into(),
            absolute_path: path.into(),
            encoding: "utf-8".into(),
            size_bytes: 0,
            content_hash: String::new(),
            language: language.into(),
            parse_capability: CoverageKind::NotCovered,
            excluded_reason: None,
        }
    }

    #[test]
    fn default_rule_set_has_expected_families() {
        let runner = RuleRunner::with_default_rules();
        let ids = runner.rule_ids();
        assert!(ids.len() >= 20);
        assert!(ids.contains(&"quality.unused-import"));
        assert!(ids.contains(&"complexity.long-function"));
        assert!(ids.contains(&"comments.commented-out-code"));
        assert!(ids.contains(&"security.hardcoded-secret"));
    }

    #[test]
    fn runner_respects_disabled_rules() {
        let mut config = RuleConfig::default();
        config.disabled_rules.push("quality.long-line".into());
        let f = file("Python", "x.py");
        let text = format!("x = {}\n", "1".repeat(200));
        let issues = run_on_file(&f, &text, &config);
        assert!(issues.iter().all(|i| i.rule_id != "quality.long-line"));

        let issues_all = run_on_file(&f, &text, &RuleConfig::default());
        assert!(issues_all.iter().any(|i| i.rule_id == "quality.long-line"));
    }

    #[test]
    fn runner_sorts_issues_by_file_and_line() {
        let config = RuleConfig::default();
        let parsed = vec![
            ParsedFile {
                file: file("Python", "b.py"),
                text: "import os\n".repeat(1) + &"y = 12345\n",
                parse: crate::parse::ParserRegistry::parse(
                    &file("Python", "b.py"),
                    "import os\ny = 12345\n",
                ),
            },
            ParsedFile {
                file: file("Python", "a.py"),
                text: "z = 9999999\n".to_owned(),
                parse: crate::parse::ParserRegistry::parse(
                    &file("Python", "a.py"),
                    "z = 9999999\n",
                ),
            },
        ];
        let issues = RuleRunner::with_default_rules().run(&parsed, &config);
        let files: Vec<&str> = issues.iter().map(|i| i.file_path.as_str()).collect();
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
    }
}
