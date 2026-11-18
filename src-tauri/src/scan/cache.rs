//! Scan cache keys: per-file rule results are reused when neither the file
//! content, rule version, nor configuration changed.

use sha2::{Digest, Sha256};

/// Builds the deterministic cache key for one file's rule results.
pub fn rule_cache_key(
    project_id: &str,
    relative_path: &str,
    content_hash: &str,
    rule_version: &str,
    config_json: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_id.as_bytes());
    hasher.update([0x1f]);
    hasher.update(relative_path.as_bytes());
    hasher.update([0x1f]);
    hasher.update(content_hash.as_bytes());
    hasher.update([0x1f]);
    hasher.update(rule_version.as_bytes());
    hasher.update([0x1f]);
    hasher.update(config_json.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex[..32].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_same_key_changed_content_new_key() {
        let a = rule_cache_key("p", "src/a.py", "hash1", "rules-1", "{}");
        let a2 = rule_cache_key("p", "src/a.py", "hash1", "rules-1", "{}");
        let changed = rule_cache_key("p", "src/a.py", "hash2", "rules-1", "{}");
        let reconfigured = rule_cache_key(
            "p",
            "src/a.py",
            "hash1",
            "rules-1",
            "{\"maxFunctionLines\":80}",
        );
        assert_eq!(a, a2);
        assert_ne!(a, changed);
        assert_ne!(a, reconfigured);
    }

    #[test]
    fn different_files_do_not_collide() {
        let a = rule_cache_key("p", "src/a.py", "h", "rules-1", "{}");
        let b = rule_cache_key("p", "src/b.py", "h", "rules-1", "{}");
        assert_ne!(a, b);
    }
}
