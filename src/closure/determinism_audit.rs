use crate::closure::types::AuditResult;

const FORBIDDEN_PATTERNS: &[(&str, &str, &str)] = &[
    ("HashMap", "source", "HashMap usage prohibited (use BTreeMap)"),
    ("HashSet", "source", "HashSet usage prohibited (use BTreeSet)"),
    ("SystemTime", "source", "SystemTime usage prohibits determinism"),
    ("UNSTABLE", "source", "Instant usage prohibits determinism"),
    ("use rand::", "import", "rand crate usage prohibits determinism"),
    ("Uuid::", "source", "UUID generation prohibits determinism"),
    ("uuid::", "source", "UUID crate usage prohibits determinism"),
    ("thread_local!", "macro", "thread_local! usage prohibits determinism"),
    ("static mut", "declaration", "static mut usage prohibits determinism"),
    ("lazy_static!", "macro", "lazy_static! usage prohibits determinism"),
    ("std::time::Instant", "import", "Instant usage prohibits determinism"),
    ("std::time::SystemTime", "import", "SystemTime usage prohibits determinism"),
];

fn is_substrate_file(path: &str) -> bool {
    if path == "closure/determinism_audit.rs" {
        return false;
    }
    path.starts_with("kernel")
        || path.starts_with("ontology")
        || path.starts_with("semantic")
        || path.starts_with("abi")
        || path.starts_with("verification")
        || path.starts_with("closure")
}

fn scan_file_for_patterns(path: &str, content: &str, result: &mut AuditResult) {
    for (pattern, category, reason) in FORBIDDEN_PATTERNS {
        for (line_num, line) in content.lines().enumerate() {
            if line.contains(pattern) {
                let trimmed = line.trim();
                if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                    continue;
                }
                result.add_violation(
                    &format!("DET{:03}", {
                        let mut id = 0u16;
                        for c in pattern.chars().take(3) {
                            id = id.wrapping_add(c as u16);
                        }
                        id % 1000
                    }),
                    &format!("Found '{}' ({}) in {}:{}: {}", pattern, category, path, line_num + 1, reason),
                );
            }
        }
    }
}

pub fn run_determinism_audit() -> AuditResult {
    let mut result = AuditResult::new("Determinism Audit");

    let src_dir = std::path::Path::new("src");
    if !src_dir.exists() {
        result.add_violation("DET001", "src directory not found");
        return result;
    }

    let mut files: Vec<_> = Vec::new();
    let mut dirs = vec![src_dir.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();

    for file_path in &files {
        let relative = file_path
            .strip_prefix("src/")
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();
        if !is_substrate_file(&relative) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(file_path) {
            scan_file_for_patterns(&relative, &content, &mut result);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism_audit_passes() {
        let result = run_determinism_audit();
        assert!(result.passed(), "Determinism audit failed: {:?}", result.violations);
    }

    #[test]
    fn test_determinism_audit_deterministic() {
        let first = run_determinism_audit();
        for _ in 0..50 {
            let next = run_determinism_audit();
            assert_eq!(first.status, next.status);
            assert_eq!(first.violations.len(), next.violations.len());
            for (va, vb) in first.violations.iter().zip(next.violations.iter()) {
                assert_eq!(va.violation_id, vb.violation_id);
                assert_eq!(va.description, vb.description);
            }
        }
    }

    #[test]
    fn test_forbidden_patterns_list_nonempty() {
        assert!(!FORBIDDEN_PATTERNS.is_empty());
    }

    #[test]
    fn test_detects_hypothetical_hashmap() {
        let mut r = AuditResult::new("test");
        scan_file_for_patterns("test.rs", "use std::collections::HashMap;", &mut r);
        assert!(r.failed());
    }

    #[test]
    fn test_substrate_file_filter() {
        assert!(is_substrate_file("kernel.rs"));
        assert!(is_substrate_file("ontology/types.rs"));
        assert!(is_substrate_file("abi/registry.rs"));
        assert!(is_substrate_file("verification/engine.rs"));
        assert!(!is_substrate_file("main.rs"));
        assert!(!is_substrate_file("triage.rs"));
        assert!(!is_substrate_file("entropy.rs"));
    }
}
