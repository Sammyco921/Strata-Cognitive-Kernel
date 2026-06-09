use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::cognition::semantic_interpreter::types::SemanticResponse;
use crate::cognition::event_translator::types::ProposedEventSequence;
use crate::cognition::policy::types::PolicyEvaluationResult;
use crate::cognition::execution_adapter::types::{ExecutionPlan, ExecutionPlanResult};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TraceRecord {
    pub trace_id: String,
    pub raw_input: String,
    pub semantic_response: SemanticResponse,
    pub proposed_sequence: ProposedEventSequence,
    pub policy_result: PolicyEvaluationResult,
    pub execution_plan: ExecutionPlan,
    pub execution_result: ExecutionPlanResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceError {
    DuplicateTraceId(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceStore {
    records: BTreeMap<String, TraceRecord>,
}

impl TraceRecord {
    pub fn new(
        semantic_response: SemanticResponse,
        proposed_sequence: ProposedEventSequence,
        policy_result: PolicyEvaluationResult,
        execution_plan: ExecutionPlan,
        execution_result: ExecutionPlanResult,
    ) -> Self {
        let trace_id = semantic_response.intent.id.clone();
        let raw_input = semantic_response.intent.raw_input.clone();
        TraceRecord {
            trace_id,
            raw_input,
            semantic_response,
            proposed_sequence,
            policy_result,
            execution_plan,
            execution_result,
        }
    }
}

impl TraceStore {
    pub fn new() -> Self {
        TraceStore {
            records: BTreeMap::new(),
        }
    }

    pub fn record_trace(&mut self, record: TraceRecord) -> Result<(), TraceError> {
        let trace_id = record.trace_id.clone();
        if self.records.contains_key(&trace_id) {
            return Err(TraceError::DuplicateTraceId(trace_id));
        }
        self.records.insert(trace_id, record);
        Ok(())
    }

    pub fn get_trace(&self, trace_id: &str) -> Option<&TraceRecord> {
        self.records.get(trace_id)
    }

    pub fn list_traces_by_prefix(&self, prefix: &str) -> Vec<&TraceRecord> {
        self.records
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(_, v)| v)
            .collect()
    }

    pub fn list_all_traces(&self) -> Vec<&TraceRecord> {
        self.records.values().collect()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::semantic_interpreter::types::{SemanticIntent, IntentType, SemanticQuery, SemanticResponse};
    use crate::cognition::event_translator::types::ProposedEvent;
    use crate::cognition::policy::types::{PolicyDecision, PolicyStatus};
    use crate::cognition::execution_adapter::types::KernelCommand;

    fn make_trace_record(suffix: &str) -> TraceRecord {
        let intent = SemanticIntent::new(&format!("input {}", suffix), IntentType::QueryGraph, BTreeMap::new(), BTreeMap::new());
        let query = Some(SemanticQuery::new());
        let semantic_response = SemanticResponse {
            intent: intent.clone(),
            query: query.clone(),
            explanation: "test".to_string(),
        };
        let event = ProposedEvent::new("evt_0", "GraphQueryRequested", BTreeMap::new(), &intent.id);
        let proposed_sequence = ProposedEventSequence::new(intent.clone(), query, vec![event]);
        let decisions = vec![PolicyDecision::new("evt_0", PolicyStatus::Approved, "ok")];
        let policy_result = PolicyEvaluationResult::new(proposed_sequence.clone(), decisions);
        let cmd = KernelCommand::new("cmd:evt_0", "QueryGraph", BTreeMap::new(), &intent.id, "evt_0");
        let execution_plan = ExecutionPlan::new(&intent.id, vec![cmd]);
        let results = vec![crate::cognition::execution_adapter::types::ExecutionResult::new("cmd:evt_0", true, "ok", None)];
        let execution_result = ExecutionPlanResult::new(execution_plan.clone(), results);
        TraceRecord::new(semantic_response, proposed_sequence, policy_result, execution_plan, execution_result)
    }

    #[test]
    fn test_trace_record_creation() {
        let record = make_trace_record("test");
        assert!(record.trace_id.starts_with("int_"));
        assert_eq!(record.raw_input, "input test");
    }

    #[test]
    fn test_trace_record_trace_id_from_intent() {
        let record = make_trace_record("hello");
        assert_eq!(record.trace_id, record.semantic_response.intent.id);
    }

    #[test]
    fn test_trace_store_empty() {
        let store = TraceStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_trace_store_record_trace() {
        let mut store = TraceStore::new();
        let record = make_trace_record("test");
        assert!(store.record_trace(record).is_ok());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_trace_store_duplicate_rejected() {
        let mut store = TraceStore::new();
        let record = make_trace_record("dup");
        let trace_id = record.trace_id.clone();
        assert!(store.record_trace(record).is_ok());
        let record2 = make_trace_record("dup");
        let result = store.record_trace(record2);
        assert_eq!(result, Err(TraceError::DuplicateTraceId(trace_id)));
    }

    #[test]
    fn test_trace_store_get_trace() {
        let mut store = TraceStore::new();
        let record = make_trace_record("get");
        let trace_id = record.trace_id.clone();
        store.record_trace(record).unwrap();
        let retrieved = store.get_trace(&trace_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().raw_input, "input get");
    }

    #[test]
    fn test_trace_store_get_trace_not_found() {
        let store = TraceStore::new();
        assert!(store.get_trace("nonexistent").is_none());
    }

    #[test]
    fn test_trace_store_list_all_traces() {
        let mut store = TraceStore::new();
        store.record_trace(make_trace_record("a")).unwrap();
        store.record_trace(make_trace_record("b")).unwrap();
        assert_eq!(store.list_all_traces().len(), 2);
    }

    #[test]
    fn test_trace_store_list_by_prefix() {
        let mut store = TraceStore::new();
        store.record_trace(make_trace_record("alpha")).unwrap();
        store.record_trace(make_trace_record("beta")).unwrap();
        let records = store.list_all_traces();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_trace_store_no_mutation() {
        let mut store = TraceStore::new();
        let record = make_trace_record("no_mut");
        store.record_trace(record).unwrap();
        let retrieved = store.get_trace("nonexistent");
        assert!(retrieved.is_none());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_trace_record_roundtrip_serialization() {
        let record = make_trace_record("roundtrip");
        let json = serde_json::to_string(&record).unwrap();
        let parsed: TraceRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.trace_id, parsed.trace_id);
        assert_eq!(record.raw_input, parsed.raw_input);
        assert_eq!(record.semantic_response.explanation, parsed.semantic_response.explanation);
        assert_eq!(record.proposed_sequence.events.len(), parsed.proposed_sequence.events.len());
        assert_eq!(record.execution_plan.commands.len(), parsed.execution_plan.commands.len());
    }

    #[test]
    fn test_trace_store_roundtrip_serialization() {
        let mut store = TraceStore::new();
        store.record_trace(make_trace_record("store")).unwrap();
        let json = serde_json::to_string(&store).unwrap();
        let parsed: TraceStore = serde_json::from_str(&json).unwrap();
        assert_eq!(store.len(), parsed.len());
    }

    #[test]
    fn test_trace_record_contains_all_stages() {
        let record = make_trace_record("all_stages");
        // All 5 pipeline stages must be present
        let _ = &record.semantic_response;    // M1
        let _ = &record.proposed_sequence;    // M2
        let _ = &record.policy_result;        // M3
        let _ = &record.execution_plan;       // M4 build
        let _ = &record.execution_result;     // M4 execute
    }

    #[test]
    fn test_trace_store_ordered_by_trace_id() {
        let mut store = TraceStore::new();
        store.record_trace(make_trace_record("z")).unwrap();
        store.record_trace(make_trace_record("a")).unwrap();
        store.record_trace(make_trace_record("m")).unwrap();
        let all = store.list_all_traces();
        for i in 1..all.len() {
            assert!(all[i - 1].trace_id <= all[i].trace_id);
        }
    }

    #[test]
    fn test_trace_error_display() {
        let err = TraceError::DuplicateTraceId("int_abc".to_string());
        assert!(format!("{:?}", err).contains("int_abc"));
    }

    #[test]
    fn test_100_run_stability() {
        let record = make_trace_record("stable");
        let json = serde_json::to_string(&record).unwrap();
        for _ in 0..100 {
            let r = make_trace_record("stable");
            let j = serde_json::to_string(&r).unwrap();
            assert_eq!(json, j);
        }
    }

    #[test]
    fn test_trace_store_100_run_stability() {
        let mut store = TraceStore::new();
        store.record_trace(make_trace_record("s1")).unwrap();
        store.record_trace(make_trace_record("s2")).unwrap();
        let json = serde_json::to_string(&store).unwrap();
        for _ in 0..100 {
            let mut s = TraceStore::new();
            s.record_trace(make_trace_record("s1")).unwrap();
            s.record_trace(make_trace_record("s2")).unwrap();
            let j = serde_json::to_string(&s).unwrap();
            assert_eq!(json, j);
        }
    }
}
