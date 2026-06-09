use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::cognition::trace::types::TraceRecord;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TraceWindow {
    pub traces: BTreeMap<String, TraceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TemporalInvariant {
    pub id: String,
    pub description: String,
    pub scope: TemporalScope,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TemporalScope {
    Global,
    PerEntity,
    PerEventType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TemporalViolation {
    pub code: String,
    pub message: String,
    pub trace_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalReport {
    pub window_id: String,
    pub score: f64,
    pub violations: Vec<TemporalViolation>,
    pub is_valid: bool,
}

impl TraceWindow {
    pub fn new() -> Self {
        TraceWindow {
            traces: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, record: TraceRecord) {
        let tid = record.trace_id.clone();
        self.traces.insert(tid, record);
    }

    pub fn len(&self) -> usize {
        self.traces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    pub fn get(&self, trace_id: &str) -> Option<&TraceRecord> {
        self.traces.get(trace_id)
    }

    pub fn all_traces(&self) -> Vec<&TraceRecord> {
        self.traces.values().collect()
    }
}

impl Default for TraceWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl TemporalInvariant {
    pub fn new(id: &str, description: &str, scope: TemporalScope) -> Self {
        TemporalInvariant {
            id: id.to_string(),
            description: description.to_string(),
            scope,
        }
    }
}

impl TemporalViolation {
    pub fn new(code: &str, message: &str, trace_ids: Vec<String>) -> Self {
        TemporalViolation {
            code: code.to_string(),
            message: message.to_string(),
            trace_ids,
        }
    }
}

impl PartialEq for TemporalReport {
    fn eq(&self, other: &Self) -> bool {
        self.window_id == other.window_id
            && self.score.to_bits() == other.score.to_bits()
            && self.violations == other.violations
            && self.is_valid == other.is_valid
    }
}

impl Eq for TemporalReport {}

impl PartialOrd for TemporalReport {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TemporalReport {
    fn cmp(&self, other: &Self) -> Ordering {
        self.window_id
            .cmp(&other.window_id)
            .then_with(|| self.score.to_bits().cmp(&other.score.to_bits()))
            .then_with(|| self.violations.cmp(&other.violations))
            .then_with(|| self.is_valid.cmp(&other.is_valid))
    }
}

impl TemporalReport {
    pub fn new(window_id: &str, score: f64, violations: Vec<TemporalViolation>) -> Self {
        let clamped = score.clamp(0.0, 1.0);
        let is_valid = violations.is_empty();
        TemporalReport {
            window_id: window_id.to_string(),
            score: clamped,
            violations,
            is_valid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_trace(id_suffix: &str) -> TraceRecord {
        let intent = crate::cognition::semantic_interpreter::types::SemanticIntent::new(
            &format!("input {}", id_suffix),
            crate::cognition::semantic_interpreter::types::IntentType::QueryGraph,
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let query = Some(crate::cognition::semantic_interpreter::types::SemanticQuery::new());
        let semantic_response = crate::cognition::semantic_interpreter::types::SemanticResponse {
            intent: intent.clone(),
            query: query.clone(),
            explanation: "test".to_string(),
        };
        let event = crate::cognition::event_translator::types::ProposedEvent::new(
            "evt_0", "GraphQueryRequested", BTreeMap::new(), &intent.id,
        );
        let proposed_sequence = crate::cognition::event_translator::types::ProposedEventSequence::new(
            intent.clone(), query, vec![event],
        );
        let decisions = vec![crate::cognition::policy::types::PolicyDecision::new(
            "evt_0", crate::cognition::policy::types::PolicyStatus::Approved, "ok",
        )];
        let policy_result = crate::cognition::policy::types::PolicyEvaluationResult::new(
            proposed_sequence.clone(), decisions,
        );
        let cmd = crate::cognition::execution_adapter::types::KernelCommand::new(
            "cmd:evt_0", "QueryGraph", BTreeMap::new(), &intent.id, "evt_0",
        );
        let execution_plan = crate::cognition::execution_adapter::types::ExecutionPlan::new(
            &intent.id, vec![cmd],
        );
        let results = vec![crate::cognition::execution_adapter::types::ExecutionResult::new(
            "cmd:evt_0", true, "nodes=0; edges=0", None,
        )];
        let execution_result = crate::cognition::execution_adapter::types::ExecutionPlanResult::new(
            execution_plan.clone(), results,
        );
        crate::cognition::trace::types::TraceRecord::new(
            semantic_response, proposed_sequence, policy_result, execution_plan, execution_result,
        )
    }

    #[test]
    fn test_trace_window_empty() {
        let w = TraceWindow::new();
        assert!(w.is_empty());
        assert_eq!(w.len(), 0);
    }

    #[test]
    fn test_trace_window_add() {
        let mut w = TraceWindow::new();
        w.add(make_dummy_trace("test"));
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn test_trace_window_get() {
        let mut w = TraceWindow::new();
        let t = make_dummy_trace("get");
        let tid = t.trace_id.clone();
        w.add(t);
        assert!(w.get(&tid).is_some());
    }

    #[test]
    fn test_trace_window_all_traces_ordered() {
        let mut w = TraceWindow::new();
        w.add(make_dummy_trace("z"));
        w.add(make_dummy_trace("a"));
        w.add(make_dummy_trace("m"));
        let all = w.all_traces();
        for i in 1..all.len() {
            assert!(all[i - 1].trace_id <= all[i].trace_id);
        }
    }

    #[test]
    fn test_temporal_invariant_creation() {
        let inv = TemporalInvariant::new("ENT001", "Entity stability", TemporalScope::PerEntity);
        assert_eq!(inv.id, "ENT001");
        assert_eq!(inv.scope, TemporalScope::PerEntity);
    }

    #[test]
    fn test_temporal_violation_creation() {
        let v = TemporalViolation::new("ENT001", "drift", vec!["t1".to_string(), "t2".to_string()]);
        assert_eq!(v.code, "ENT001");
        assert_eq!(v.trace_ids.len(), 2);
    }

    #[test]
    fn test_temporal_report_valid() {
        let r = TemporalReport::new("win1", 1.0, vec![]);
        assert!(r.is_valid);
    }

    #[test]
    fn test_temporal_report_invalid() {
        let v = TemporalViolation::new("E001", "err", vec![]);
        let r = TemporalReport::new("win1", 0.5, vec![v]);
        assert!(!r.is_valid);
    }

    #[test]
    fn test_temporal_report_score_clamped() {
        let r = TemporalReport::new("win1", 1.5, vec![]);
        assert!((r.score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_temporal_report_eq() {
        let a = TemporalReport::new("w1", 0.8, vec![]);
        let b = TemporalReport::new("w1", 0.8, vec![]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_temporal_report_ord() {
        let low = TemporalReport::new("a", 0.3, vec![]);
        let high = TemporalReport::new("b", 0.9, vec![]);
        assert!(low < high);
    }

    #[test]
    fn test_temporal_report_roundtrip() {
        let v = TemporalViolation::new("E001", "msg", vec!["t1".to_string()]);
        let r = TemporalReport::new("w1", 0.75, vec![v]);
        let json = serde_json::to_string(&r).unwrap();
        let parsed: TemporalReport = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
    }

    #[test]
    fn test_temporal_scope_ordering() {
        assert!(TemporalScope::Global < TemporalScope::PerEntity);
        assert!(TemporalScope::PerEntity < TemporalScope::PerEventType);
    }

    #[test]
    fn test_100_run_stability() {
        let v = TemporalViolation::new("E001", "test", vec!["t1".to_string()]);
        let json = serde_json::to_string(&v).unwrap();
        for _ in 0..100 {
            let v2 = TemporalViolation::new("E001", "test", vec!["t1".to_string()]);
            assert_eq!(json, serde_json::to_string(&v2).unwrap());
        }
    }
}
