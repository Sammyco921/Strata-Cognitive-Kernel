use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::cognition::event_translator::types::ProposedEventSequence;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PolicyStatus {
    Approved,
    Rejected,
    Modified,
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub event_id: String,
    pub status: PolicyStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub applies_to_event_type: Option<String>,
    pub applies_to_intent_type: Option<String>,
    pub required_properties: BTreeMap<String, String>,
    pub forbidden_properties: BTreeMap<String, String>,
    pub max_confidence: Option<f64>,
    pub min_confidence: Option<f64>,
}

impl PartialEq for PolicyRule {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.applies_to_event_type == other.applies_to_event_type
            && self.applies_to_intent_type == other.applies_to_intent_type
            && self.required_properties == other.required_properties
            && self.forbidden_properties == other.forbidden_properties
            && self.max_confidence.map(f64::to_bits) == other.max_confidence.map(f64::to_bits)
            && self.min_confidence.map(f64::to_bits) == other.min_confidence.map(f64::to_bits)
    }
}

impl Eq for PolicyRule {}

impl Ord for PolicyRule {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for PolicyRule {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PolicyEvaluationResult {
    pub sequence: ProposedEventSequence,
    pub decisions: Vec<PolicyDecision>,
    pub explanation: String,
}

impl PolicyDecision {
    pub fn new(event_id: &str, status: PolicyStatus, reason: &str) -> Self {
        PolicyDecision {
            event_id: event_id.to_string(),
            status,
            reason: reason.to_string(),
        }
    }
}

impl PolicyRule {
    pub fn new(id: &str) -> Self {
        PolicyRule {
            id: id.to_string(),
            applies_to_event_type: None,
            applies_to_intent_type: None,
            required_properties: BTreeMap::new(),
            forbidden_properties: BTreeMap::new(),
            max_confidence: None,
            min_confidence: None,
        }
    }

    pub fn matches_event_type(&self, event_type: &str) -> bool {
        match &self.applies_to_event_type {
            Some(t) => t == event_type,
            None => true,
        }
    }

    pub fn matches_intent_type(&self, intent_type: &str) -> bool {
        match &self.applies_to_intent_type {
            Some(t) => t == intent_type,
            None => true,
        }
    }

    pub fn matches(&self, event_type: &str, intent_type: &str) -> bool {
        self.matches_event_type(event_type) && self.matches_intent_type(intent_type)
    }
}

impl PolicyEvaluationResult {
    pub fn new(
        sequence: ProposedEventSequence,
        decisions: Vec<PolicyDecision>,
    ) -> Self {
        let approved = decisions
            .iter()
            .filter(|d| d.status == PolicyStatus::Approved)
            .count();
        let rejected = decisions
            .iter()
            .filter(|d| d.status == PolicyStatus::Rejected)
            .count();
        let deferred = decisions
            .iter()
            .filter(|d| d.status == PolicyStatus::Deferred)
            .count();
        let explanation = format!(
            "events={}; approved={}; rejected={}; deferred={}",
            decisions.len(),
            approved,
            rejected,
            deferred,
        );
        PolicyEvaluationResult {
            sequence,
            decisions,
            explanation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sequence() -> ProposedEventSequence {
        let intent = crate::cognition::semantic_interpreter::types::SemanticIntent::new(
            "test",
            crate::cognition::semantic_interpreter::types::IntentType::QueryGraph,
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let events = vec![crate::cognition::event_translator::types::ProposedEvent::new(
            "evt_0",
            "GraphQueryRequested",
            BTreeMap::new(),
            &intent.id,
        )];
        ProposedEventSequence::new(intent, None, events)
    }

    #[test]
    fn test_policy_decision_new() {
        let d = PolicyDecision::new("evt_0", PolicyStatus::Approved, "ok");
        assert_eq!(d.event_id, "evt_0");
        assert_eq!(d.status, PolicyStatus::Approved);
        assert_eq!(d.reason, "ok");
    }

    #[test]
    fn test_policy_rule_default_matches_all() {
        let rule = PolicyRule::new("R001");
        assert!(rule.matches("any_type", "any_intent"));
    }

    #[test]
    fn test_policy_rule_matches_event_type() {
        let rule = PolicyRule::new("R001");
        assert!(rule.matches_event_type("GraphQueryRequested"));
        assert!(rule.matches_event_type("anything"));
    }

    #[test]
    fn test_policy_rule_specific_event_type() {
        let mut rule = PolicyRule::new("R001");
        rule.applies_to_event_type = Some("GraphQueryRequested".to_string());
        assert!(rule.matches_event_type("GraphQueryRequested"));
        assert!(!rule.matches_event_type("NoOp"));
    }

    #[test]
    fn test_policy_rule_specific_intent_type() {
        let mut rule = PolicyRule::new("R001");
        rule.applies_to_intent_type = Some("QueryGraph".to_string());
        assert!(rule.matches_intent_type("QueryGraph"));
        assert!(!rule.matches_intent_type("Unknown"));
    }

    #[test]
    fn test_policy_rule_ord_by_id() {
        let a = PolicyRule::new("A001");
        let b = PolicyRule::new("B001");
        assert!(a < b);
    }

    #[test]
    fn test_policy_evaluation_result() {
        let decisions = vec![
            PolicyDecision::new("evt_0", PolicyStatus::Approved, ""),
            PolicyDecision::new("evt_1", PolicyStatus::Rejected, ""),
            PolicyDecision::new("evt_2", PolicyStatus::Deferred, ""),
        ];
        let result = PolicyEvaluationResult::new(sample_sequence(), decisions);
        assert!(result.explanation.contains("events=3"));
        assert!(result.explanation.contains("approved=1"));
        assert!(result.explanation.contains("rejected=1"));
        assert!(result.explanation.contains("deferred=1"));
    }

    #[test]
    fn test_policy_decision_roundtrip() {
        let d = PolicyDecision::new("evt_0", PolicyStatus::Modified, "injected props");
        let json = serde_json::to_string(&d).unwrap();
        let parsed: PolicyDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(d.event_id, parsed.event_id);
        assert_eq!(d.status, parsed.status);
    }

    #[test]
    fn test_policy_rule_roundtrip() {
        let mut rule = PolicyRule::new("R001");
        rule.applies_to_event_type = Some("GraphQueryRequested".to_string());
        rule.forbidden_properties
            .insert("dangerous".to_string(), "true".to_string());
        let json = serde_json::to_string(&rule).unwrap();
        let parsed: PolicyRule = serde_json::from_str(&json).unwrap();
        assert_eq!(rule.id, parsed.id);
        assert_eq!(rule.forbidden_properties, parsed.forbidden_properties);
    }

    #[test]
    fn test_policy_status_ordering() {
        assert!(PolicyStatus::Approved < PolicyStatus::Rejected);
        assert!(PolicyStatus::Deferred > PolicyStatus::Modified);
    }
}
