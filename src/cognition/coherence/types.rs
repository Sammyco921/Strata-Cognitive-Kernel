use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoherenceViolation {
    pub code: String,
    pub message: String,
    pub stage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceScore {
    value: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoherenceReport {
    pub trace_id: String,
    pub score: CoherenceScore,
    pub violations: Vec<CoherenceViolation>,
    pub is_valid: bool,
}

impl CoherenceViolation {
    pub fn new(code: &str, message: &str, stage: &str) -> Self {
        CoherenceViolation {
            code: code.to_string(),
            message: message.to_string(),
            stage: stage.to_string(),
        }
    }
}

impl CoherenceScore {
    pub fn new(value: f64) -> Self {
        let clamped = value.clamp(0.0, 1.0);
        CoherenceScore { value: clamped }
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

impl PartialEq for CoherenceScore {
    fn eq(&self, other: &Self) -> bool {
        self.value.to_bits() == other.value.to_bits()
    }
}

impl Eq for CoherenceScore {}

impl PartialOrd for CoherenceScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CoherenceScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.to_bits().cmp(&other.value.to_bits())
    }
}

impl CoherenceReport {
    pub fn new(trace_id: &str, score: CoherenceScore, violations: Vec<CoherenceViolation>) -> Self {
        let is_valid = violations.is_empty();
        CoherenceReport {
            trace_id: trace_id.to_string(),
            score,
            violations,
            is_valid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherence_violation_creation() {
        let v = CoherenceViolation::new("EVT001", "Missing mapping", "execution");
        assert_eq!(v.code, "EVT001");
        assert_eq!(v.message, "Missing mapping");
        assert_eq!(v.stage, "execution");
    }

    #[test]
    fn test_coherence_violation_ordering() {
        let a = CoherenceViolation::new("A", "msg", "s");
        let b = CoherenceViolation::new("B", "msg", "s");
        assert!(a < b);
    }

    #[test]
    fn test_coherence_score_creation() {
        let s = CoherenceScore::new(0.85);
        assert!((s.value() - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_coherence_score_clamping() {
        let over = CoherenceScore::new(1.5);
        assert!((over.value() - 1.0).abs() < f64::EPSILON);
        let under = CoherenceScore::new(-0.5);
        assert!((under.value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_coherence_score_eq() {
        let a = CoherenceScore::new(0.5);
        let b = CoherenceScore::new(0.5);
        assert_eq!(a, b);
    }

    #[test]
    fn test_coherence_score_neq() {
        let a = CoherenceScore::new(0.5);
        let b = CoherenceScore::new(0.6);
        assert_ne!(a, b);
    }

    #[test]
    fn test_coherence_score_ord() {
        let low = CoherenceScore::new(0.3);
        let high = CoherenceScore::new(0.9);
        assert!(low < high);
    }

    #[test]
    fn test_coherence_report_valid() {
        let score = CoherenceScore::new(1.0);
        let report = CoherenceReport::new("trace_1", score, vec![]);
        assert!(report.is_valid);
    }

    #[test]
    fn test_coherence_report_invalid() {
        let score = CoherenceScore::new(0.5);
        let violations = vec![CoherenceViolation::new("E001", "issue", "s1")];
        let report = CoherenceReport::new("trace_1", score, violations);
        assert!(!report.is_valid);
    }

    #[test]
    fn test_coherence_report_roundtrip() {
        let score = CoherenceScore::new(0.75);
        let violations = vec![CoherenceViolation::new("E001", "msg", "s1")];
        let report = CoherenceReport::new("t1", score, violations);
        let json = serde_json::to_string(&report).unwrap();
        let parsed: CoherenceReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.trace_id, parsed.trace_id);
        assert_eq!(report.is_valid, parsed.is_valid);
        assert_eq!(report.violations.len(), parsed.violations.len());
        assert_eq!(report.score, parsed.score);
    }

    #[test]
    fn test_coherence_score_roundtrip() {
        let s = CoherenceScore::new(0.42);
        let json = serde_json::to_string(&s).unwrap();
        let parsed: CoherenceScore = serde_json::from_str(&json).unwrap();
        assert_eq!(s, parsed);
    }

    #[test]
    fn test_100_run_stability() {
        let v = CoherenceViolation::new("E001", "test", "s1");
        let json = serde_json::to_string(&v).unwrap();
        for _ in 0..100 {
            let v2 = CoherenceViolation::new("E001", "test", "s1");
            let j = serde_json::to_string(&v2).unwrap();
            assert_eq!(json, j);
        }
    }
}
