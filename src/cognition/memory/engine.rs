use crate::ontology::OntologyRegistry;

use super::types::{CognitiveMemoryState, MemorySnapshot, MemoryUpdateEvent};

pub fn update_memory(
    state: CognitiveMemoryState,
    event: &MemoryUpdateEvent,
    ontology_registry: &OntologyRegistry,
) -> CognitiveMemoryState {
    let mut next = state;
    next.update_counter = next.update_counter.wrapping_add(1);
    next.last_updated_trace_id = event.trace_id.clone();

    let is_success = event.execution_result_summary == "all_success" && event.coherence_valid;

    if is_success {
        *next.intent_success_counts.entry(event.intent_type.clone()).or_insert(0) += 1;
    } else {
        *next.intent_failure_counts.entry(event.intent_type.clone()).or_insert(0) += 1;
    }

    for cmd_type in &event.command_list {
        if is_success {
            *next.command_success_counts.entry(cmd_type.clone()).or_insert(0) += 1;
        } else {
            *next.command_failure_counts.entry(cmd_type.clone()).or_insert(0) += 1;
        }
    }

    update_ontology_alignment(&mut next, ontology_registry);

    next
}

fn update_ontology_alignment(
    state: &mut CognitiveMemoryState,
    ontology_registry: &OntologyRegistry,
) {
    let total_types = ontology_registry.entity_types.len()
        + ontology_registry.relationship_types.len()
        + ontology_registry.property_types.len();

    if total_types == 0 {
        return;
    }

    let n = state.update_counter as f64;

    for (type_name, _) in &ontology_registry.entity_types {
        let old = state.ontology_alignment_scores.get(type_name).copied().unwrap_or(0.0);
        let incremental = 1.0 / total_types as f64;
        if state.update_counter <= 1 {
            state.ontology_alignment_scores.insert(type_name.clone(), incremental);
        } else {
            let new_score = old + (incremental - old) / n;
            state.ontology_alignment_scores.insert(type_name.clone(), new_score);
        }
    }

    for (type_name, _) in &ontology_registry.relationship_types {
        let old = state.ontology_alignment_scores.get(type_name).copied().unwrap_or(0.0);
        let incremental = 1.0 / total_types as f64;
        if state.update_counter <= 1 {
            state.ontology_alignment_scores.insert(type_name.clone(), incremental);
        } else {
            let new_score = old + (incremental - old) / n;
            state.ontology_alignment_scores.insert(type_name.clone(), new_score);
        }
    }

    for (type_name, _) in &ontology_registry.property_types {
        let old = state.ontology_alignment_scores.get(type_name).copied().unwrap_or(0.0);
        let incremental = 1.0 / total_types as f64;
        if state.update_counter <= 1 {
            state.ontology_alignment_scores.insert(type_name.clone(), incremental);
        } else {
            let new_score = old + (incremental - old) / n;
            state.ontology_alignment_scores.insert(type_name.clone(), new_score);
        }
    }
}

pub fn compute_memory_snapshot(state: CognitiveMemoryState) -> MemorySnapshot {
    MemorySnapshot::from_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use crate::cognition::semantic_interpreter::types::IntentType;
    use crate::ontology::OntologyRegistry;

    fn make_event(
        trace_id: &str,
        intent_type: IntentType,
        all_success: bool,
        coherence_valid: bool,
        cmd_types: &[&str],
    ) -> MemoryUpdateEvent {
        let execution_result_summary = if all_success { "all_success" } else { "partial_failure" };
        MemoryUpdateEvent {
            trace_id: trace_id.to_string(),
            intent_type,
            execution_result_summary: execution_result_summary.to_string(),
            policy_decision_summary: "ranked=1;approved=1".to_string(),
            coherence_valid,
            command_list: cmd_types.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn empty_registry() -> OntologyRegistry {
        OntologyRegistry::empty()
    }

    fn registry_with_types(type_names: &[&str]) -> OntologyRegistry {
        let mut reg = OntologyRegistry::empty();
        for name in type_names {
            reg.entity_types.insert(
                name.to_string(),
                crate::ontology::types::EntityType {
                    name: name.to_string(),
                    description: None,
                },
            );
        }
        reg
    }

    #[test]
    fn test_empty_memory_state() {
        let state = CognitiveMemoryState::empty();
        assert_eq!(state.update_counter, 0);
        assert!(state.intent_success_counts.is_empty());
        assert!(state.intent_failure_counts.is_empty());
        assert!(state.command_success_counts.is_empty());
        assert!(state.command_failure_counts.is_empty());
        assert!(state.ontology_alignment_scores.is_empty());
        assert!(state.last_updated_trace_id.is_empty());
    }

    #[test]
    fn test_success_increments_intent_counter() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["QueryGraph"]);
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.intent_success_counts.get(&IntentType::QueryGraph).copied().unwrap_or(0), 1);
        assert_eq!(next.intent_failure_counts.get(&IntentType::QueryGraph).copied().unwrap_or(0), 0);
    }

    #[test]
    fn test_failure_increments_intent_counter() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::DescribeNode, false, true, &["DescribeNode"]);
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.intent_failure_counts.get(&IntentType::DescribeNode).copied().unwrap_or(0), 1);
        assert_eq!(next.intent_success_counts.get(&IntentType::DescribeNode).copied().unwrap_or(0), 0);
    }

    #[test]
    fn test_coherence_invalid_counts_as_failure() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, false, &["QueryGraph"]);
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.intent_failure_counts.get(&IntentType::QueryGraph).copied().unwrap_or(0), 1);
        assert_eq!(next.intent_success_counts.get(&IntentType::QueryGraph).copied().unwrap_or(0), 0);
    }

    #[test]
    fn test_command_success_counts() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["QueryGraph", "CreateNode"]);
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.command_success_counts.get("QueryGraph").copied().unwrap_or(0), 1);
        assert_eq!(next.command_success_counts.get("CreateNode").copied().unwrap_or(0), 1);
        assert_eq!(next.command_failure_counts.get("QueryGraph").copied().unwrap_or(0), 0);
    }

    #[test]
    fn test_command_failure_counts() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, false, true, &["QueryGraph", "CreateNode"]);
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.command_failure_counts.get("QueryGraph").copied().unwrap_or(0), 1);
        assert_eq!(next.command_failure_counts.get("CreateNode").copied().unwrap_or(0), 1);
        assert_eq!(next.command_success_counts.get("QueryGraph").copied().unwrap_or(0), 0);
    }

    #[test]
    fn test_update_counter_increments() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let s1 = update_memory(state, &event, &empty_registry());
        assert_eq!(s1.update_counter, 1);
        let s2 = update_memory(s1, &event, &empty_registry());
        assert_eq!(s2.update_counter, 2);
    }

    #[test]
    fn test_last_updated_trace_id() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("trace_abc", IntentType::Unknown, true, true, &[]);
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.last_updated_trace_id, "trace_abc");
    }

    #[test]
    fn test_accumulation_across_multiple_updates() {
        let state = CognitiveMemoryState::empty();
        let success = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let failure = make_event("t2", IntentType::QueryGraph, false, true, &["Q"]);
        let s1 = update_memory(state, &success, &empty_registry());
        let s2 = update_memory(s1, &success, &empty_registry());
        let s3 = update_memory(s2, &failure, &empty_registry());
        assert_eq!(s3.intent_success_counts.get(&IntentType::QueryGraph).copied().unwrap_or(0), 2);
        assert_eq!(s3.intent_failure_counts.get(&IntentType::QueryGraph).copied().unwrap_or(0), 1);
        assert_eq!(s3.command_success_counts.get("Q").copied().unwrap_or(0), 2);
        assert_eq!(s3.command_failure_counts.get("Q").copied().unwrap_or(0), 1);
        assert_eq!(s3.update_counter, 3);
    }

    #[test]
    fn test_different_intents_count_separately() {
        let state = CognitiveMemoryState::empty();
        let qg = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let dn = make_event("t2", IntentType::DescribeNode, true, true, &["D"]);
        let uk = make_event("t3", IntentType::Unknown, false, true, &["U"]);
        let s1 = update_memory(state, &qg, &empty_registry());
        let s2 = update_memory(s1, &dn, &empty_registry());
        let s3 = update_memory(s2, &uk, &empty_registry());
        assert_eq!(s3.intent_success_counts.get(&IntentType::QueryGraph).copied().unwrap_or(0), 1);
        assert_eq!(s3.intent_success_counts.get(&IntentType::DescribeNode).copied().unwrap_or(0), 1);
        assert_eq!(s3.intent_failure_counts.get(&IntentType::Unknown).copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_identical_trace_produces_identical_memory() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let a = update_memory(state.clone(), &event, &empty_registry());
        let b = update_memory(state, &event, &empty_registry());
        assert_eq!(a.intent_success_counts, b.intent_success_counts);
        assert_eq!(a.command_success_counts, b.command_success_counts);
        assert_eq!(a.update_counter, b.update_counter);
    }

    #[test]
    fn test_no_mutation_of_input_trace() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let trace_id_before = event.trace_id.clone();
        let summary_before = event.execution_result_summary.clone();
        let _ = update_memory(state, &event, &empty_registry());
        assert_eq!(event.trace_id, trace_id_before);
        assert_eq!(event.execution_result_summary, summary_before);
    }

    #[test]
    fn test_empty_trace_handling() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::Unknown, true, true, &[]);
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.intent_success_counts.get(&IntentType::Unknown).copied().unwrap_or(0), 1);
        assert!(next.command_success_counts.is_empty());
    }

    #[test]
    fn test_large_trace_handling() {
        let state = CognitiveMemoryState::empty();
        let mut cmd_types = Vec::new();
        for i in 0..1000 {
            cmd_types.push(format!("Cmd{}", i));
        }
        let event = MemoryUpdateEvent {
            trace_id: "large".to_string(),
            intent_type: IntentType::QueryGraph,
            execution_result_summary: "all_success".to_string(),
            policy_decision_summary: "ranked=1;approved=1".to_string(),
            coherence_valid: true,
            command_list: cmd_types,
        };
        let next = update_memory(state, &event, &empty_registry());
        assert_eq!(next.command_success_counts.len(), 1000);
        assert_eq!(next.update_counter, 1);
    }

    #[test]
    fn test_100_deterministic_updates() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let first = update_memory(state.clone(), &event, &empty_registry());
        for _ in 0..100 {
            let next = update_memory(state.clone(), &event, &empty_registry());
            assert_eq!(next.intent_success_counts, first.intent_success_counts);
            assert_eq!(next.command_success_counts, first.command_success_counts);
        }
    }

    #[test]
    fn test_memory_snapshot_derived_stats() {
        let mut state = CognitiveMemoryState::empty();
        let s1 = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let s2 = make_event("t2", IntentType::QueryGraph, true, true, &["Q"]);
        let f1 = make_event("t3", IntentType::QueryGraph, false, true, &["Q"]);
        state = update_memory(state, &s1, &empty_registry());
        state = update_memory(state, &s2, &empty_registry());
        state = update_memory(state, &f1, &empty_registry());

        let snapshot = MemorySnapshot::from_state(state);
        let rate = snapshot.success_rate_per_intent.get(&IntentType::QueryGraph).copied().unwrap_or(0.0);
        assert!((rate - 2.0 / 3.0).abs() < 1e-10);
        let fail_rate = snapshot.failure_rate_per_intent.get(&IntentType::QueryGraph).copied().unwrap_or(0.0);
        assert!((fail_rate - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_command_reliability_scores() {
        let mut state = CognitiveMemoryState::empty();
        let s = make_event("t1", IntentType::QueryGraph, true, true, &["Q", "C"]);
        let f = make_event("t2", IntentType::DescribeNode, false, true, &["Q"]);
        state = update_memory(state, &s, &empty_registry());
        state = update_memory(state, &f, &empty_registry());
        let snapshot = MemorySnapshot::from_state(state);
        let q_rel = snapshot.command_reliability_scores.get("Q").copied().unwrap_or(0.0);
        assert!((q_rel - 0.5).abs() < 1e-10);
        let c_rel = snapshot.command_reliability_scores.get("C").copied().unwrap_or(0.0);
        assert!((c_rel - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        state = update_memory(state, &event, &empty_registry());
        let json = serde_json::to_string(&state).unwrap();
        let parsed: CognitiveMemoryState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.intent_success_counts, parsed.intent_success_counts);
        assert_eq!(state.intent_failure_counts, parsed.intent_failure_counts);
        assert_eq!(state.command_success_counts, parsed.command_success_counts);
        assert_eq!(state.command_failure_counts, parsed.command_failure_counts);
        assert_eq!(state.update_counter, parsed.update_counter);
        assert_eq!(state.last_updated_trace_id, parsed.last_updated_trace_id);
    }

    #[test]
    fn test_snapshot_serialization_roundtrip() {
        let state = CognitiveMemoryState::empty();
        let snapshot = MemorySnapshot::from_state(state);
        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: MemorySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot.state.update_counter, parsed.state.update_counter);
    }

    #[test]
    fn test_memory_snapshot_empty_state() {
        let state = CognitiveMemoryState::empty();
        let snapshot = MemorySnapshot::from_state(state);
        assert!(snapshot.success_rate_per_intent.is_empty());
        assert!(snapshot.failure_rate_per_intent.is_empty());
        assert!(snapshot.command_reliability_scores.is_empty());
    }

    #[test]
    fn test_ordering_stability_btreemap() {
        let mut state = CognitiveMemoryState::empty();
        let qg = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let dn = make_event("t2", IntentType::DescribeNode, true, true, &["D"]);
        state = update_memory(state, &qg, &empty_registry());
        state = update_memory(state, &dn, &empty_registry());
        let keys: Vec<&IntentType> = state.intent_success_counts.keys().collect();
        for i in 1..keys.len() {
            assert!(keys[i - 1] < keys[i]);
        }
    }

    #[test]
    fn test_ontology_alignment_with_registry() {
        let state = CognitiveMemoryState::empty();
        let reg = registry_with_types(&["Person", "Organization"]);
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let next = update_memory(state, &event, &reg);
        assert!(next.ontology_alignment_scores.contains_key("Person"));
        assert!(next.ontology_alignment_scores.contains_key("Organization"));
    }

    #[test]
    fn test_ontology_alignment_with_empty_registry() {
        let state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let next = update_memory(state, &event, &empty_registry());
        assert!(next.ontology_alignment_scores.is_empty());
    }

    #[test]
    fn test_ontology_alignment_accumulates_over_runs() {
        let mut state = CognitiveMemoryState::empty();
        let reg = registry_with_types(&["Person"]);
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        state = update_memory(state, &event, &reg);
        state = update_memory(state, &event, &reg);
        state = update_memory(state, &event, &reg);
        let score = state.ontology_alignment_scores.get("Person").copied().unwrap_or(0.0);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_memory_update_event_construction() {
        let intent = crate::cognition::semantic_interpreter::types::SemanticIntent::new(
            "test",
            IntentType::QueryGraph,
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let sr = crate::cognition::semantic_interpreter::types::SemanticResponse {
            intent: intent.clone(),
            query: None,
            explanation: "test".to_string(),
        };
        let event = crate::cognition::event_translator::types::ProposedEvent::new(
            "evt_0", "GraphQueryRequested", BTreeMap::new(), &intent.id,
        );
        let seq = crate::cognition::event_translator::types::ProposedEventSequence::new(
            intent.clone(), None, vec![event],
        );
        let decisions = vec![crate::cognition::policy::types::PolicyDecision::new(
            "evt_0", crate::cognition::policy::types::PolicyStatus::Approved, "ok",
        )];
        let pr = crate::cognition::policy::types::PolicyEvaluationResult::new(seq.clone(), decisions);
        let cmd = crate::cognition::execution_adapter::types::KernelCommand::new(
            "cmd:evt_0", "QueryGraph", BTreeMap::new(), &intent.id, "evt_0",
        );
        let plan = crate::cognition::execution_adapter::types::ExecutionPlan::new(&intent.id, vec![cmd]);
        let result = crate::cognition::execution_adapter::types::ExecutionResult::new(
            "cmd:evt_0", true, "ok", None,
        );
        let er = crate::cognition::execution_adapter::types::ExecutionPlanResult::new(
            plan.clone(), vec![result],
        );
        let trace = crate::cognition::trace::types::TraceRecord::new(sr, seq, pr, plan, er);
        let score = crate::cognition::coherence::types::CoherenceScore::new(1.0);
        let coherence = crate::cognition::coherence::types::CoherenceReport::new(
            &trace.trace_id, score, vec![],
        );
        let ranked = vec![];
        let decision = crate::cognition::system::policy::types::PolicyDecision::new(
            crate::cognition::system::policy::types::PolicyCandidate {
                intent: intent.clone(),
                score: crate::cognition::system::policy::types::PolicyScore::compute(
                    10, 5, 3, 2, 1, 1.0, &intent.id,
                ),
            },
            ranked,
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let update_event = MemoryUpdateEvent::from_trace_and_coherence(&trace, &coherence, &decision);
        assert_eq!(update_event.trace_id, trace.trace_id);
        assert_eq!(update_event.intent_type, IntentType::QueryGraph);
        assert_eq!(update_event.execution_result_summary, "all_success");
        assert!(update_event.coherence_valid);
    }

    #[test]
    fn test_snapshot_consistency_after_updates() {
        let mut state = CognitiveMemoryState::empty();
        let reg = registry_with_types(&["Person"]);
        for i in 0..10 {
            let success = i % 2 == 0;
            let event = make_event(
                &format!("t{}", i),
                IntentType::QueryGraph,
                success,
                true,
                &["Q"],
            );
            state = update_memory(state, &event, &reg);
        }
        let snapshot = MemorySnapshot::from_state(state);
        let success_rate = snapshot.success_rate_per_intent.get(&IntentType::QueryGraph).copied().unwrap_or(0.0);
        assert!((success_rate - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_partial_update_does_not_affect_other_intents() {
        let state = CognitiveMemoryState::empty();
        let qg = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        let dn = make_event("t2", IntentType::DescribeNode, true, true, &["D"]);
        let s1 = update_memory(state, &qg, &empty_registry());
        let s2 = update_memory(s1, &dn, &empty_registry());
        assert_eq!(s2.intent_success_counts.len(), 2);
    }

    #[test]
    fn test_cognitive_memory_state_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CognitiveMemoryState>();
    }

    #[test]
    fn test_cognitive_memory_state_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CognitiveMemoryState>();
    }

    #[test]
    fn test_memory_snapshot_serialization_deterministic() {
        let mut state = CognitiveMemoryState::empty();
        let event = make_event("t1", IntentType::QueryGraph, true, true, &["Q"]);
        state = update_memory(state, &event, &empty_registry());
        let snapshot = MemorySnapshot::from_state(state);
        let first = serde_json::to_string(&snapshot).unwrap();
        for _ in 0..100 {
            let json = serde_json::to_string(&snapshot).unwrap();
            assert_eq!(first, json);
        }
    }

    #[test]
    fn test_memory_update_event_no_panic() {
        let intent = crate::cognition::semantic_interpreter::types::SemanticIntent::new(
            "test", IntentType::Unknown, BTreeMap::new(), BTreeMap::new(),
        );
        let sr = crate::cognition::semantic_interpreter::types::SemanticResponse {
            intent: intent.clone(), query: None, explanation: "".to_string(),
        };
        let event = crate::cognition::event_translator::types::ProposedEvent::new(
            "evt_0", "NoOp", BTreeMap::new(), &intent.id,
        );
        let seq = crate::cognition::event_translator::types::ProposedEventSequence::new(
            intent.clone(), None, vec![event],
        );
        let decisions = vec![crate::cognition::policy::types::PolicyDecision::new(
            "evt_0", crate::cognition::policy::types::PolicyStatus::Rejected, "no",
        )];
        let pr = crate::cognition::policy::types::PolicyEvaluationResult::new(seq.clone(), decisions);
        let plan = crate::cognition::execution_adapter::types::ExecutionPlan::new(
            &intent.id, vec![],
        );
        let er = crate::cognition::execution_adapter::types::ExecutionPlanResult::new(
            plan.clone(), vec![],
        );
        let trace = crate::cognition::trace::types::TraceRecord::new(sr, seq, pr, plan, er);
        let score = crate::cognition::coherence::types::CoherenceScore::new(0.0);
        let coherence = crate::cognition::coherence::types::CoherenceReport::new(
            &trace.trace_id, score, vec![],
        );
        let ranked = vec![];
        let decision = crate::cognition::system::policy::types::PolicyDecision::new(
            crate::cognition::system::policy::types::PolicyCandidate {
                intent: intent.clone(),
                score: crate::cognition::system::policy::types::PolicyScore::compute(
                    0, 0, 0, 0, 0, 0.0, &intent.id,
                ),
            },
            ranked,
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let update_event = MemoryUpdateEvent::from_trace_and_coherence(&trace, &coherence, &decision);
        assert_eq!(update_event.execution_result_summary, "all_success");
        assert!(update_event.coherence_valid);
    }
}
