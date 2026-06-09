#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuditViolation {
    pub audit_name: String,
    pub violation_id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuditResult {
    pub audit_name: String,
    pub status: AuditStatus,
    pub violations: Vec<AuditViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubstrateClosureReport {
    pub total_audits: usize,
    pub passed_audits: usize,
    pub failed_audits: usize,
    pub total_violations: usize,
    pub results: Vec<AuditResult>,
    pub closure_status: AuditStatus,
}

impl AuditViolation {
    pub fn new(audit_name: &str, violation_id: &str, description: &str) -> Self {
        AuditViolation {
            audit_name: audit_name.to_string(),
            violation_id: violation_id.to_string(),
            description: description.to_string(),
        }
    }
}

impl AuditResult {
    pub fn new(audit_name: &str) -> Self {
        AuditResult {
            audit_name: audit_name.to_string(),
            status: AuditStatus::Passed,
            violations: Vec::new(),
        }
    }

    pub fn add_violation(&mut self, violation_id: &str, description: &str) {
        self.violations.push(AuditViolation::new(
            &self.audit_name,
            violation_id,
            description,
        ));
        self.status = AuditStatus::Failed;
    }

    pub fn passed(&self) -> bool {
        self.status == AuditStatus::Passed
    }

    pub fn failed(&self) -> bool {
        self.status == AuditStatus::Failed
    }
}

impl SubstrateClosureReport {
    pub fn new(results: Vec<AuditResult>) -> Self {
        let total_audits = results.len();
        let passed_audits = results.iter().filter(|r| r.passed()).count();
        let failed_audits = results.iter().filter(|r| r.failed()).count();
        let total_violations: usize = results.iter().map(|r| r.violations.len()).sum();
        let closure_status = if failed_audits == 0 && total_violations == 0 {
            AuditStatus::Passed
        } else {
            AuditStatus::Failed
        };
        SubstrateClosureReport {
            total_audits,
            passed_audits,
            failed_audits,
            total_violations,
            results,
            closure_status,
        }
    }

    pub fn is_passed(&self) -> bool {
        self.closure_status == AuditStatus::Passed
    }

    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        out.push_str("{\"total_audits\":");
        out.push_str(&self.total_audits.to_string());
        out.push_str(",\"passed_audits\":");
        out.push_str(&self.passed_audits.to_string());
        out.push_str(",\"failed_audits\":");
        out.push_str(&self.failed_audits.to_string());
        out.push_str(",\"total_violations\":");
        out.push_str(&self.total_violations.to_string());
        out.push_str(",\"closure_status\":\"");
        out.push_str(if self.is_passed() { "Passed" } else { "Failed" });
        out.push_str("\",\"results\":[");
        for (i, result) in self.results.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"audit_name\":\"");
            out.push_str(&result.audit_name);
            out.push_str("\",\"status\":\"");
            out.push_str(if result.passed() { "Passed" } else { "Failed" });
            out.push_str("\",\"violations\":[");
            for (j, v) in result.violations.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push_str("{\"violation_id\":\"");
                out.push_str(&v.violation_id);
                out.push_str("\",\"description\":\"");
                out.push_str(&v.description);
                out.push_str("\"}");
            }
            out.push_str("]}");
        }
        out.push_str("]}");
        out
    }

    pub fn from_json_string(s: &str) -> Option<Self> {
        let s = s.trim();
        if !s.starts_with("{\"total_audits\":") || !s.ends_with("]}") {
            return None;
        }
        let total_audits = Self::json_parse_int(s, "\"total_audits\":")?;
        let passed_audits = Self::json_parse_int(s, "\"passed_audits\":")?;
        let failed_audits = Self::json_parse_int(s, "\"failed_audits\":")?;
        let total_violations = Self::json_parse_int(s, "\"total_violations\":")?;
        let closure_status_str = Self::json_parse_str(s, "\"closure_status\":\"")?;
        let closure_status = if closure_status_str == "Passed" {
            AuditStatus::Passed
        } else {
            AuditStatus::Failed
        };
        let results = Self::json_parse_results(s)?;
        Some(SubstrateClosureReport {
            total_audits,
            passed_audits,
            failed_audits,
            total_violations,
            results,
            closure_status,
        })
    }

    fn json_parse_int(s: &str, key: &str) -> Option<usize> {
        let start = s.find(key)?;
        let value_start = start + key.len();
        let value_end = s[value_start..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(s.len() - value_start);
        s[value_start..value_start + value_end].parse().ok()
    }

    fn json_parse_str(s: &str, key: &str) -> Option<String> {
        let start = s.find(key)?;
        let value_start = start + key.len();
        let value_end = s[value_start..].find('"')?;
        Some(s[value_start..value_start + value_end].to_string())
    }

    fn json_parse_results(s: &str) -> Option<Vec<AuditResult>> {
        let results_key = "\"results\":[";
        let start = s.find(results_key)? + results_key.len();
        if s[start..].starts_with(']') {
            return Some(Vec::new());
        }
        let mut results = Vec::new();
        let mut pos = start;
        loop {
            if pos >= s.len() || s[pos..].starts_with(']') {
                break;
            }
            let chunk = &s[pos..];
            let (result, consumed) = Self::json_parse_single_result(chunk)?;
            results.push(result);
            pos += consumed;
            if pos < s.len() && s[pos..].starts_with(',') {
                pos += 1;
            }
        }
        Some(results)
    }

    fn json_parse_single_result(s: &str) -> Option<(AuditResult, usize)> {
        let mut consumed = 0usize;
        if !s.starts_with("{\"audit_name\":\"") {
            return None;
        }
        consumed += "{\"audit_name\":\"".len();
        let end = s[consumed..].find('"')?;
        let audit_name = s[consumed..consumed + end].to_string();
        consumed += end + 1;

        if !s[consumed..].starts_with(",\"status\":\"") {
            return None;
        }
        consumed += ",\"status\":\"".len();
        let end = s[consumed..].find('"')?;
        let status_str = s[consumed..consumed + end].to_string();
        consumed += end + 1;

        if !s[consumed..].starts_with(",\"violations\":[") {
            return None;
        }
        consumed += ",\"violations\":[".len();

        let mut result = AuditResult::new(&audit_name);

        if s[consumed..].starts_with(']') {
            consumed += 1;
        } else {
            loop {
                if s[consumed..].starts_with(']') {
                    consumed += 1;
                    break;
                }
                if s[consumed..].starts_with(',') {
                    consumed += 1;
                    continue;
                }
                let (violation_id, desc, v_consumed) =
                    Self::json_parse_single_violation(&s[consumed..])?;
                result.add_violation(&violation_id, &desc);
                consumed += v_consumed;
            }
        }

        if !s[consumed..].starts_with('}') {
            return None;
        }
        consumed += 1;

        if status_str == "Failed" && result.passed() {
            result.status = AuditStatus::Failed;
        }

        Some((result, consumed))
    }

    fn json_parse_single_violation(s: &str) -> Option<(String, String, usize)> {
        let mut consumed = 0usize;
        if !s.starts_with("{\"violation_id\":\"") {
            return None;
        }
        consumed += "{\"violation_id\":\"".len();
        let end = s[consumed..].find('"')?;
        let violation_id = s[consumed..consumed + end].to_string();
        consumed += end + 1;

        if !s[consumed..].starts_with(",\"description\":\"") {
            return None;
        }
        consumed += ",\"description\":\"".len();
        let end = s[consumed..].find('"')?;
        let description = s[consumed..consumed + end].to_string();
        consumed += end + 1;

        if s[consumed..].starts_with('}') {
            consumed += 1;
        }

        Some((violation_id, description, consumed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_violation_new() {
        let v = AuditViolation::new("dep", "V001", "cycle detected");
        assert_eq!(v.audit_name, "dep");
        assert_eq!(v.violation_id, "V001");
        assert_eq!(v.description, "cycle detected");
    }

    #[test]
    fn test_audit_result_default_passed() {
        let r = AuditResult::new("test-audit");
        assert!(r.passed());
        assert!(!r.failed());
        assert!(r.violations.is_empty());
    }

    #[test]
    fn test_audit_result_add_violation() {
        let mut r = AuditResult::new("test-audit");
        r.add_violation("V001", "something wrong");
        assert!(r.failed());
        assert!(!r.passed());
        assert_eq!(r.violations.len(), 1);
        assert_eq!(r.violations[0].violation_id, "V001");
    }

    #[test]
    fn test_substrate_closure_report_empty() {
        let report = SubstrateClosureReport::new(Vec::new());
        assert_eq!(report.total_audits, 0);
        assert_eq!(report.passed_audits, 0);
        assert_eq!(report.failed_audits, 0);
        assert_eq!(report.total_violations, 0);
        assert!(report.is_passed());
    }

    #[test]
    fn test_substrate_closure_report_with_violations() {
        let mut r = AuditResult::new("dep-audit");
        r.add_violation("V001", "cycle");
        let report = SubstrateClosureReport::new(vec![r]);
        assert_eq!(report.total_audits, 1);
        assert_eq!(report.passed_audits, 0);
        assert_eq!(report.failed_audits, 1);
        assert_eq!(report.total_violations, 1);
        assert!(!report.is_passed());
    }

    #[test]
    fn test_report_json_roundtrip() {
        let mut r1 = AuditResult::new("dep-audit");
        r1.add_violation("V001", "cycle detected");
        let mut r2 = AuditResult::new("surface-audit");
        r2.add_violation("V002", "leaked type");
        let report = SubstrateClosureReport::new(vec![r1, r2]);
        let json = report.to_json_string();
        let parsed = SubstrateClosureReport::from_json_string(&json).unwrap();
        assert_eq!(report.total_audits, parsed.total_audits);
        assert_eq!(report.passed_audits, parsed.passed_audits);
        assert_eq!(report.failed_audits, parsed.failed_audits);
        assert_eq!(report.total_violations, parsed.total_violations);
        assert_eq!(report.is_passed(), parsed.is_passed());
    }

    #[test]
    fn test_report_json_roundtrip_no_violations() {
        let r = AuditResult::new("dep-audit");
        let report = SubstrateClosureReport::new(vec![r]);
        let json = report.to_json_string();
        let parsed = SubstrateClosureReport::from_json_string(&json).unwrap();
        assert_eq!(report.total_audits, parsed.total_audits);
        assert_eq!(report.is_passed(), parsed.is_passed());
    }

    #[test]
    fn test_report_json_invalid_input() {
        assert!(SubstrateClosureReport::from_json_string("not json").is_none());
        assert!(SubstrateClosureReport::from_json_string("{}").is_none());
    }

    #[test]
    fn test_deterministic_serialization() {
        let mut r1 = AuditResult::new("dep-audit");
        r1.add_violation("V001", "cycle detected");
        let mut r2 = AuditResult::new("surface-audit");
        r2.add_violation("V002", "leaked type");
        let report = SubstrateClosureReport::new(vec![r1, r2]);
        let first = report.to_json_string();
        for _ in 0..50 {
            assert_eq!(first, report.to_json_string());
        }
    }

    #[test]
    fn test_report_ordering_deterministic() {
        let r1 = AuditResult::new("a");
        let mut r2 = AuditResult::new("b");
        r2.add_violation("V1", "test");
        let report = SubstrateClosureReport::new(vec![r1, r2]);
        assert_eq!(report.results[0].audit_name, "a");
        assert_eq!(report.results[1].audit_name, "b");
    }
}
