//! Duplicate-code detection over normalized source windows containing control
//! flow. Exact and renamed copies take priority; near matches are best-effort
//! for small edits and require review before consolidation.
pub mod matcher;
pub mod token_normalize;

use matcher::match_clones;
use serde::{Deserialize, Serialize};

/// Clone detection configuration; `min_lines` mirrors
/// `RuleConfig::duplicate_min_lines` by default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CloneConfig {
    pub min_lines: usize,
    pub similarity_threshold: f32,
    pub max_groups: usize,
}

impl Default for CloneConfig {
    fn default() -> Self {
        Self {
            min_lines: 6,
            similarity_threshold: 0.8,
            max_groups: 200,
        }
    }
}

impl CloneConfig {
    pub fn from_rule_config(config: &crate::rules::config::RuleConfig) -> Self {
        Self {
            min_lines: config.duplicate_min_lines.max(2),
            ..Self::default()
        }
    }
}

pub use matcher::{CloneKind, CloneMember};

#[derive(Debug, Clone)]
pub struct CloneDetector {
    config: CloneConfig,
}

impl CloneDetector {
    pub fn new(config: CloneConfig) -> Self {
        Self { config }
    }

    pub fn find(&self, files: &[crate::parse::ParsedFile]) -> Vec<CloneGroup> {
        self.find_with_checkpoint(files, || Ok::<(), std::convert::Infallible>(()))
            .unwrap_or_else(|never| match never {})
    }

    pub fn find_with_checkpoint<E>(
        &self,
        files: &[crate::parse::ParsedFile],
        check: impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<CloneGroup>, E> {
        match_clones(files, &self.config, check)
    }
}

pub use matcher::CloneGroup;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{CoverageKind, FileRecord};
    use crate::parse::{ParseResult, ParsedFile};

    fn parsed(path: &str, text: &str) -> ParsedFile {
        let file = FileRecord {
            id: path.into(),
            scan_run_id: "run".into(),
            project_id: "p".into(),
            relative_path: path.into(),
            absolute_path: path.into(),
            encoding: "utf-8".into(),
            size_bytes: 0,
            content_hash: String::new(),
            language: "Python".into(),
            parse_capability: CoverageKind::Ast,
            excluded_reason: None,
        };
        ParsedFile {
            file,
            text: text.to_owned(),
            parse: ParseResult::default(),
        }
    }

    const BODY_A: &str = r#"def calc_invoice(items, tax_rate):
    subtotal = 0
    for item in items:
        if item.quantity <= 0:
            continue
        subtotal += item.price * item.quantity
    tax = subtotal * tax_rate
    return subtotal + tax
"#;

    const BODY_B: &str = r#"def compute_bill(entries, vat):
    base = 0
    for entry in entries:
        if entry.qty <= 0:
            continue
        base += entry.cost * entry.qty
    fee = base * vat
    return base + fee
"#;

    #[test]
    fn exact_and_renamed_clones_are_found() {
        let files = vec![
            parsed("src/billing_a.py", BODY_A),
            parsed("src/billing_b.py", BODY_B),
        ];
        let groups = CloneDetector::new(CloneConfig {
            min_lines: 6,
            similarity_threshold: 0.8,
            max_groups: 50,
        })
        .find(&files);
        assert_eq!(groups.len(), 1, "{groups:?}");
        assert_eq!(groups[0].members.len(), 2);
        assert_eq!(groups[0].kind, CloneKind::Similar);
        assert!(groups[0].similarity >= 0.8);
        assert_eq!(groups[0].members[0].file_path, "src/billing_a.py");
        assert_eq!(groups[0].members[0].start_line, 1);
    }

    #[test]
    fn exact_clone_locations_are_independent_of_input_order() {
        let detector = CloneDetector::new(CloneConfig::default());
        let mut files = vec![parsed("b.py", BODY_A), parsed("a.py", BODY_A)];
        let groups = detector.find(&files);
        files.reverse();
        assert_eq!(detector.find(&files), groups);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, CloneKind::Exact);
        assert_eq!(groups[0].members[0].file_path, "a.py");
        assert_eq!(groups[0].members[0].start_line, 1);
        assert_eq!(groups[0].members[0].end_line, 6);
    }

    #[test]
    fn one_changed_line_is_a_near_clone_only_above_threshold() {
        let original = BODY_A.lines().take(6).collect::<Vec<_>>().join("\n");
        let changed = original.replace("subtotal +=", "subtotal -=");
        let files = [parsed("a.py", &original), parsed("b.py", &changed)];
        let groups = CloneDetector::new(CloneConfig::default()).find(&files);
        assert_eq!(groups.len(), 1, "{groups:?}");
        assert_eq!(groups[0].kind, CloneKind::Similar);
        assert!((groups[0].similarity - 5.0 / 6.0).abs() < f32::EPSILON);
        assert!(
            CloneDetector::new(CloneConfig {
                similarity_threshold: 0.9,
                ..CloneConfig::default()
            })
            .find(&files)
            .is_empty()
        );
    }

    #[test]
    fn checkpoint_can_interrupt_indexing_a_long_file() {
        let source = BODY_A.repeat(5_000);
        let files = [parsed("long.py", &source)];
        let detector = CloneDetector::new(CloneConfig::default());
        let mut calls = 0;
        let result = detector.find_with_checkpoint(&files, || {
            calls += 1;
            if calls == 6 { Err("canceled") } else { Ok(()) }
        });
        assert_eq!(result, Err("canceled"));
    }

    #[test]
    fn large_repeated_bucket_yields_non_overlapping_locations() {
        let source = "for row in rows:\n".repeat(18_000);
        let files = [parsed("generated.py", &source)];
        let groups = CloneDetector::new(CloneConfig::default()).find(&files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, CloneKind::Exact);
        assert_eq!(groups[0].members.len(), 3_000);
        assert_eq!(groups[0].members.last().unwrap().end_line, 18_000);
    }

    #[test]
    fn ordinary_short_repeats_are_ignored() {
        let repeated = "    return render(template)\n";
        let files = vec![
            parsed(
                "a.py",
                &format!("def f1():\n{repeated}    x = 1\n    y = 2\n    z = 3\n    return x\n"),
            ),
            parsed(
                "b.py",
                &format!("def f2():\n{repeated}    p = 4\n    q = 5\n    r = 6\n    return p\n"),
            ),
        ];
        let groups = CloneDetector::new(CloneConfig::default()).find(&files);
        assert!(groups.is_empty(), "{groups:?}");
    }

    #[test]
    fn same_file_overlap_windows_are_not_self_clones() {
        let text = format!("{BODY_A}{BODY_A}");
        let files = vec![parsed("dup.py", &text)];
        let groups = CloneDetector::new(CloneConfig {
            min_lines: 6,
            similarity_threshold: 0.8,
            max_groups: 50,
        })
        .find(&files);
        // repeated body inside one file is a genuine clone, reported once
        assert_eq!(groups.len(), 1, "{groups:?}");
        assert!(groups[0].members[0].file_path == "dup.py");
    }
}
