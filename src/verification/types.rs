#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InvariantId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvariantLayer {
    Kernel,
    Ontology,
    Semantic,
    Abi,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvariantStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InvariantSpec {
    pub id: InvariantId,
    pub layer: InvariantLayer,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerificationResult {
    pub invariant_id: InvariantId,
    pub layer: InvariantLayer,
    pub status: InvariantStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerificationReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<VerificationResult>,
}

impl VerificationResult {
    pub fn passed(id: InvariantId, layer: InvariantLayer) -> Self {
        VerificationResult {
            invariant_id: id,
            layer,
            status: InvariantStatus::Passed,
            reason: String::new(),
        }
    }

    pub fn failed(id: InvariantId, layer: InvariantLayer, reason: &str) -> Self {
        let id_str = id.0.clone();
        VerificationResult {
            invariant_id: id,
            layer,
            status: InvariantStatus::Failed,
            reason: format!("{} failed: {}", id_str, reason),
        }
    }
}

impl VerificationReport {
    pub fn new(results: Vec<VerificationResult>) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|r| r.status == InvariantStatus::Passed).count();
        let failed = results.iter().filter(|r| r.status == InvariantStatus::Failed).count();
        VerificationReport { total, passed, failed, results }
    }

    pub fn is_all_passed(&self) -> bool {
        self.failed == 0
    }

    pub fn to_deterministic_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.push(format!(
            "{{\"total\":{},\"passed\":{},\"failed\":{},\"results\":[",
            self.total, self.passed, self.failed,
        ));
        let result_strings: Vec<String> = self.results.iter().map(|r| {
            let id = escape_json(&r.invariant_id.0);
            let layer = format!("{:?}", r.layer);
            let status = format!("{:?}", r.status);
            let reason = if r.reason.is_empty() {
                "null".to_string()
            } else {
                escape_json(&r.reason)
            };
            format!("{{\"id\":{},\"layer\":\"{}\",\"status\":\"{}\",\"reason\":{}}}", id, layer, status, reason)
        }).collect();
        parts.push(result_strings.join(","));
        parts.push("]}".to_string());
        parts.concat()
    }
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn test_invariant_id_ordering() {
        let mut set = BTreeSet::new();
        set.insert(InvariantId("b".into()));
        set.insert(InvariantId("a".into()));
        set.insert(InvariantId("c".into()));
        let ordered: Vec<&InvariantId> = set.iter().collect();
        assert_eq!(ordered[0].0, "a");
        assert_eq!(ordered[1].0, "b");
        assert_eq!(ordered[2].0, "c");
    }

    #[test]
    fn test_verification_result_passed() {
        let r = VerificationResult::passed(InvariantId("test".into()), InvariantLayer::Kernel);
        assert_eq!(r.status, InvariantStatus::Passed);
        assert!(r.reason.is_empty());
    }

    #[test]
    fn test_verification_result_failed() {
        let r = VerificationResult::failed(InvariantId("TEST".into()), InvariantLayer::Abi, "something went wrong");
        assert_eq!(r.status, InvariantStatus::Failed);
        assert!(r.reason.contains("TEST"));
        assert!(r.reason.contains("something went wrong"));
    }

    #[test]
    fn test_verification_report_counts() {
        let results = vec![
            VerificationResult::passed(InvariantId("a".into()), InvariantLayer::Kernel),
            VerificationResult::passed(InvariantId("b".into()), InvariantLayer::Ontology),
            VerificationResult::failed(InvariantId("c".into()), InvariantLayer::Semantic, "fail"),
        ];
        let report = VerificationReport::new(results);
        assert_eq!(report.total, 3);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 1);
        assert!(!report.is_all_passed());
    }

    #[test]
    fn test_report_serialization_determinism() {
        let results = vec![
            VerificationResult::passed(InvariantId("a".into()), InvariantLayer::Kernel),
            VerificationResult::failed(InvariantId("b".into()), InvariantLayer::Abi, "err"),
        ];
        let report = VerificationReport::new(results);
        let s1 = report.to_deterministic_string();
        let s2 = report.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_report_serialization_contains_ids() {
        let results = vec![
            VerificationResult::passed(InvariantId("X".into()), InvariantLayer::Kernel),
        ];
        let report = VerificationReport::new(results);
        let json = report.to_deterministic_string();
        assert!(json.contains("\"X\""));
        assert!(json.contains("Kernel"));
        assert!(json.contains("Passed"));
    }

    #[test]
    fn test_stability_100_runs() {
        let results = vec![
            VerificationResult::passed(InvariantId("a".into()), InvariantLayer::Kernel),
            VerificationResult::failed(InvariantId("b".into()), InvariantLayer::Abi, "err"),
        ];
        let first = VerificationReport::new(results.clone()).to_deterministic_string();
        for _ in 0..100 {
            let s = VerificationReport::new(results.clone()).to_deterministic_string();
            assert_eq!(first, s);
        }
    }
}
