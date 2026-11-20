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
use super::source_redaction::{is_credential_path, sensitive_source_lines};
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
            let credential_file = is_credential_path(&parsed.file.relative_path);
            let sensitive_lines: Vec<u32> = sensitive_source_lines(&parsed.text)
                .enumerate()
                .filter_map(|(index, (_, sensitive))| sensitive.then_some(index as u32 + 1))
                .collect();
            for rule in &self.rules {
                if !config.is_rule_enabled(rule.id()) {
                    continue;
                }
                if !rule.supports(ctx.language(), coverage) {
                    continue;
                }
                for mut draft in dedup_drafts(rule.check(&ctx)) {
                    let first = sensitive_lines.partition_point(|line| *line < draft.start_line);
                    if credential_file
                        || sensitive_lines
                            .get(first)
                            .is_some_and(|line| *line <= draft.end_line)
                    {
                        draft.evidence = "疑似凭据已隐藏".into();
                    }
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
    fn redacts_multiline_credential_evidence_but_keeps_ordinary_findings() {
        let config = RuleConfig {
            enabled_rules: Some(vec!["quality.long-line".into()]),
            ..RuleConfig::default()
        };
        let secret = "a".repeat(160);
        let ordinary = "z".repeat(160);
        for (language, path, opener, closer) in [
            ("Python", "config.py", "password = \"\"\"", "\"\"\""),
            ("Python", "config.py", "password: str = '''", "'''"),
            ("Python", "config.py", "password: str = f\"\"\"", "\"\"\""),
            ("Python", "config.py", "password: str = r'''", "'''"),
            ("Python", "config.py", "password: bytes = b\"\"\"", "\"\"\""),
            ("Python", "config.py", "password: str = u'''", "'''"),
            ("Python", "config.py", "password: str = fr\"\"\"", "\"\"\""),
            ("Python", "config.py", "password: str = rf'''", "'''"),
            (
                "Python",
                "config.py",
                "password: bytes = br\"\"\"",
                "\"\"\"",
            ),
            ("Python", "config.py", "password: bytes = rb'''", "'''"),
            ("TypeScript", "config.ts", "const api_key: string = `", "`"),
            ("Go", "config.go", "api_key := `", "`"),
            (
                "YAML",
                "config.yml",
                "secret: | # setting=value",
                "next: value",
            ),
            (
                "Python",
                "config.py",
                "-----BEGIN PRIVATE KEY-----",
                "-----END PRIVATE KEY-----",
            ),
        ] {
            let text = format!("{opener}\n  {secret}\n{closer}\n{ordinary}\n");
            let issues = run_on_file(&file(language, path), &text, &config);
            assert_eq!(issues.len(), 2, "{language}");
            assert_eq!(issues[0].start_line, 2, "{language}");
            assert_eq!(issues[0].evidence, "疑似凭据已隐藏", "{language}");
            assert!(issues[1].evidence.contains(&ordinary), "{language}");
            assert!(
                !serde_json::to_string(&issues).unwrap().contains(&secret),
                "{language} must not retain the credential in issue fields"
            );
        }
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
