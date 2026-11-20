//! Versioned rule configuration with defaults. Serialized into each scan run
//! so historical scores stay explainable.

use serde::{Deserialize, Serialize};

pub const RULE_CONFIG_VERSION: u32 = 1;
/// Bumped whenever rule semantics, source positions, or thresholds change materially.
pub const RULE_VERSION: &str = "rules-4";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RuleConfig {
    pub version: u32,
    pub max_line_length: usize,
    pub max_function_lines: u32,
    pub max_cyclomatic: u32,
    pub max_cognitive: u32,
    pub max_nesting: u32,
    pub max_parameters: u32,
    pub max_class_methods: u32,
    pub max_class_lines: u32,
    pub max_file_lines: u32,
    /// Minimum normalized block length for duplicate detection (task 6).
    pub duplicate_min_lines: usize,
    /// `None` enables every rule; a list restricts the run to those IDs.
    pub enabled_rules: Option<Vec<String>>,
    pub disabled_rules: Vec<String>,
    /// Additional directory names excluded from traversal for this run.
    pub excluded_directories: Vec<String>,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            version: RULE_CONFIG_VERSION,
            max_line_length: 120,
            max_function_lines: 60,
            max_cyclomatic: 15,
            max_cognitive: 25,
            max_nesting: 4,
            max_parameters: 5,
            max_class_methods: 20,
            max_class_lines: 300,
            max_file_lines: 600,
            duplicate_min_lines: 6,
            enabled_rules: None,
            disabled_rules: vec![],
            excluded_directories: vec![],
        }
    }
}

impl RuleConfig {
    /// Parses a scan-run config JSON; unknown fields are ignored so configs
    /// stay forward-compatible within a version.
    pub fn from_json(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    /// Rules are individually toggleable; a bare family name (e.g.
    /// `security`) toggles the whole family.
    pub fn is_rule_enabled(&self, rule_id: &str) -> bool {
        let family = rule_id.split('.').next().unwrap_or("");
        if self
            .disabled_rules
            .iter()
            .any(|d| d == rule_id || d == family)
        {
            return false;
        }
        match &self.enabled_rules {
            None => true,
            Some(list) => list.iter().any(|e| e == rule_id || e == family),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_all_thresholds() {
        let config = RuleConfig::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.max_line_length, 120);
        assert!(config.enabled_rules.is_none());
    }

    #[test]
    fn json_round_trip_ignores_unknown_fields() {
        let config = RuleConfig::from_json(&serde_json::json!({
            "maxFunctionLines": 80,
            "futureSetting": true
        }));
        assert_eq!(config.max_function_lines, 80);
        assert_eq!(config.max_cyclomatic, 15);
    }

    #[test]
    fn custom_exclusion_names_are_saved_with_scan_thresholds() {
        let config = RuleConfig::from_json(&serde_json::json!({
            "maxFunctionLines": 80,
            "excludedDirectories": ["generated", "vendor"]
        }));
        assert_eq!(config.max_function_lines, 80);
        assert_eq!(config.excluded_directories, ["generated", "vendor"]);
    }

    #[test]
    fn enable_disable_filters() {
        let mut config = RuleConfig::default();
        assert!(config.is_rule_enabled("security.hardcoded-secret"));
        config.disabled_rules.push("security".into());
        assert!(!config.is_rule_enabled("security.hardcoded-secret"));
        config.disabled_rules.clear();
        config.enabled_rules = Some(vec!["complexity".into()]);
        assert!(!config.is_rule_enabled("basic.naming"));
        assert!(config.is_rule_enabled("complexity.long-function"));
    }
}
