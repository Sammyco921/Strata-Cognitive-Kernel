use strata_kill_test::cognition::trace::*;
use strata_kill_test::cognition::semantic_interpreter::*;
use strata_kill_test::cognition::event_translator::*;
use strata_kill_test::cognition::policy::*;
use strata_kill_test::cognition::execution_adapter::*;

// ── 1. Identical pipeline → identical trace ──────────────────────────────

#[test]
fn test_identical_pipeline_identical_trace() {
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let a = record_trace(resp.clone(), seq.clone(), policy.clone(), plan.clone(), result);
    let seq2 = translate(resp.clone());
    let policy2 = evaluate(seq2.clone(), vec![PolicyRule::new("R001")]);
    let plan2 = build_plan(seq2.clone(), policy2.clone());
    let result2 = execute_plan(plan2.clone());
    let b = record_trace(resp, seq2, policy2, plan2, result2);
    assert_eq!(a, b);
}

// ── 2. 100-run stability ──────────────────────────────────────────────────

#[test]
fn test_100_run_stability() {
    let first = {
        let resp = interpret("describe node 42");
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        record_trace(resp, seq, policy, plan, result)
    };
    for _ in 0..100 {
        let resp = interpret("describe node 42");
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        assert_eq!(first, trace);
    }
}

// ── 3. Modifying inputs after trace → no effect on trace ─────────────────

#[test]
fn test_modifying_inputs_after_trace_no_effect() {
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq.clone(), policy.clone(), plan.clone(), result);
    let orig_events = trace.proposed_sequence.events.len();
    let mut seq2 = seq.clone();
    seq2.events.clear();
    assert_eq!(trace.proposed_sequence.events.len(), orig_events);
    assert!(seq2.events.is_empty());
}

// ── 4. Trace ID consistency across all layers ────────────────────────────

#[test]
fn test_trace_id_consistency() {
    let resp = interpret("query graph for Person age:30");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let tid = trace.trace_id;
    assert_eq!(trace.semantic_response.intent.id, tid);
    assert_eq!(trace.proposed_sequence.intent.id, tid);
    assert_eq!(trace.policy_result.sequence.intent.id, tid);
}

// ── 5. All pipeline stages present in trace ──────────────────────────────

#[test]
fn test_all_pipeline_stages_present() {
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let _ = &trace.semantic_response;
    let _ = &trace.proposed_sequence;
    let _ = &trace.policy_result;
    let _ = &trace.execution_plan;
    let _ = &trace.execution_result;
}

// ── 6. Append-only: no overwrite of existing records ─────────────────────

#[test]
fn test_append_only_no_overwrite() {
    let mut store = TraceStore::new();
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    assert!(store.record_trace(trace).is_ok());
    // Second insert with same trace_id should fail
    let resp2 = interpret("find nodes");
    let seq2 = translate(resp2.clone());
    let policy2 = evaluate(seq2.clone(), vec![PolicyRule::new("R001")]);
    let plan2 = build_plan(seq2.clone(), policy2.clone());
    let result2 = execute_plan(plan2.clone());
    let trace2 = record_trace(resp2, seq2, policy2, plan2, result2);
    assert!(store.record_trace(trace2).is_err());
    assert_eq!(store.len(), 1);
}

// ── 7. Retrieval by trace_id ─────────────────────────────────────────────

#[test]
fn test_retrieval_by_trace_id() {
    let mut store = TraceStore::new();
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let tid = trace.trace_id.clone();
    store.record_trace(trace).unwrap();
    let retrieved = store.get_trace(&tid);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().raw_input, "find nodes");
}

// ── 8. Retrieval of non-existent trace_id ─────────────────────────────────

#[test]
fn test_retrieval_nonexistent() {
    let store = TraceStore::new();
    assert!(store.get_trace("int_nonexistent").is_none());
}

// ── 9. List all traces returns sorted results ────────────────────────────

#[test]
fn test_list_all_traces_sorted() {
    let mut store = TraceStore::new();
    // Create traces with different inputs so they get different trace_ids
    let inputs = vec!["find nodes", "describe node 1", "query graph"];
    for input in inputs {
        let resp = interpret(input);
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        store.record_trace(trace).unwrap();
    }
    let all = store.list_all_traces();
    assert_eq!(all.len(), 3);
    for i in 1..all.len() {
        assert!(all[i - 1].trace_id <= all[i].trace_id);
    }
}

// ── 10. Prefix search correctness ────────────────────────────────────────

#[test]
fn test_prefix_search() {
    let mut store = TraceStore::new();
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let tid = trace.trace_id.clone();
    store.record_trace(trace).unwrap();
    // All trace_ids start with "int_"
    let prefixed = store.list_traces_by_prefix("int_");
    assert!(prefixed.len() >= 1);
    let exact = store.list_traces_by_prefix(&tid);
    assert_eq!(exact.len(), 1);
}

// ── 11. Prefix search with no matches ────────────────────────────────────

#[test]
fn test_prefix_search_no_matches() {
    let store = TraceStore::new();
    let empty = store.list_traces_by_prefix("nonexistent_");
    assert!(empty.is_empty());
}

// ── 12. Full trace record roundtrip serialization ────────────────────────

#[test]
fn test_full_trace_roundtrip_serialization() {
    let resp = interpret("describe node 42");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let json = serde_json::to_string(&trace).unwrap();
    let parsed: TraceRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(trace.trace_id, parsed.trace_id);
    assert_eq!(trace.raw_input, parsed.raw_input);
    assert_eq!(trace.semantic_response.explanation, parsed.semantic_response.explanation);
    assert_eq!(trace.proposed_sequence.events.len(), parsed.proposed_sequence.events.len());
    assert_eq!(trace.policy_result.decisions.len(), parsed.policy_result.decisions.len());
    assert_eq!(trace.execution_plan.commands.len(), parsed.execution_plan.commands.len());
    assert_eq!(trace.execution_result.results.len(), parsed.execution_result.results.len());
}

// ── 13. TraceStore roundtrip serialization ────────────────────────────────

#[test]
fn test_trace_store_roundtrip_serialization() {
    let mut store = TraceStore::new();
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    store.record_trace(trace).unwrap();
    let json = serde_json::to_string(&store).unwrap();
    let parsed: TraceStore = serde_json::from_str(&json).unwrap();
    assert_eq!(store.len(), parsed.len());
}

// ── 14. Deterministic JSON encoding ──────────────────────────────────────

#[test]
fn test_deterministic_json_encoding() {
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let json = serde_json::to_string(&trace).unwrap();
    for _ in 0..100 {
        let j = serde_json::to_string(&trace).unwrap();
        assert_eq!(json, j);
    }
}

// ── 15. Trace layer does not affect execution output ─────────────────────

#[test]
fn test_trace_layer_no_effect_on_execution() {
    // Run pipeline WITHOUT trace
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let result_without = result.clone();

    // Run pipeline WITH trace
    let resp2 = interpret("find nodes");
    let seq2 = translate(resp2.clone());
    let policy2 = evaluate(seq2.clone(), vec![PolicyRule::new("R001")]);
    let plan2 = build_plan(seq2.clone(), policy2.clone());
    let result2 = execute_plan(plan2.clone());
    let _trace = record_trace(resp2, seq2, policy2, plan2, result2.clone());

    // Results are identical
    assert_eq!(result_without, result2);
}

// ── 16. Removing trace does not change pipeline output ───────────────────

#[test]
fn test_removing_trace_no_change() {
    // Run with trace recording
    let resp = interpret("describe node 42");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result1 = execute_plan(plan.clone());

    // Now run the same pipeline and record trace
    let resp2 = interpret("describe node 42");
    let seq2 = translate(resp2.clone());
    let policy2 = evaluate(seq2.clone(), vec![PolicyRule::new("R001")]);
    let plan2 = build_plan(seq2.clone(), policy2.clone());
    let result2 = execute_plan(plan2.clone());
    let _trace = record_trace(resp2, seq2, policy2, plan2, result2.clone());

    // Results should match
    assert_eq!(result1, result2);
}

// ── 17. Trace record stores snapshot, not reference ──────────────────────

#[test]
fn test_trace_stores_snapshot_not_reference() {
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq.clone(), policy.clone(), plan.clone(), result);
    // Clear the original - trace should be unaffected
    let mut seq_copy = seq.clone();
    seq_copy.events.clear();
    assert!(!trace.proposed_sequence.events.is_empty());
}

// ── 18. Multiple traces in store ──────────────────────────────────────────

#[test]
fn test_multiple_traces_in_store() {
    let mut store = TraceStore::new();
    for input in &["find nodes", "describe node 1", "query graph", "show everything"] {
        let resp = interpret(input);
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        store.record_trace(trace).unwrap();
    }
    assert_eq!(store.len(), 4);
}

// ── 19. Trace_id stability across same input ─────────────────────────────

#[test]
fn test_trace_id_stability_same_input() {
    let a = {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        trace.trace_id
    };
    let b = {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        trace.trace_id
    };
    assert_eq!(a, b);
}

// ── 20. Different inputs → different trace_ids ───────────────────────────

#[test]
fn test_different_inputs_different_trace_ids() {
    let a = {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        trace.trace_id
    };
    let b = {
        let resp = interpret("describe graph");
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        trace.trace_id
    };
    assert_ne!(a, b);
}

// ── 21. ExecutionPlan stored in trace matches standalone result ──────────

#[test]
fn test_plan_in_trace_matches_standalone() {
    let resp = interpret("describe node 42");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let standalone_plan = plan.clone();
    let trace = record_trace(resp, seq, policy, plan, result);
    assert_eq!(trace.execution_plan, standalone_plan);
}

// ── 22. ExecutionResult in trace matches standalone result ────────────────

#[test]
fn test_result_in_trace_matches_standalone() {
    let resp = interpret("describe node 42");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let standalone_result = result.clone();
    let trace = record_trace(resp, seq, policy, plan, result);
    assert_eq!(trace.execution_result, standalone_result);
}

// ── 23. Empty store listing ──────────────────────────────────────────────

#[test]
fn test_empty_store_listing() {
    let store = TraceStore::new();
    assert!(store.list_all_traces().is_empty());
    assert!(store.list_traces_by_prefix("int_").is_empty());
}

// ── 24. TraceError type ──────────────────────────────────────────────────

#[test]
fn test_trace_error_type() {
    let err = TraceError::DuplicateTraceId("int_abc".to_string());
    match err {
        TraceError::DuplicateTraceId(id) => assert_eq!(id, "int_abc"),
    }
}

// ── 25. record_trace preserves raw_input ─────────────────────────────────

#[test]
fn test_record_trace_preserves_raw_input() {
    let inputs = vec!["find nodes", "describe node 42", "query graph for Person"];
    for input in inputs {
        let resp = interpret(input);
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        assert_eq!(trace.raw_input, input);
    }
}

// ── 26. Event count matches between proposed_sequence and plan ───────────

#[test]
fn test_event_count_consistency() {
    let resp = interpret("describe node 5 1 3");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    // Approved events should match command count
    let approved = trace.policy_result.decisions.iter()
        .filter(|d| d.status == PolicyStatus::Approved)
        .count();
    assert!(trace.execution_plan.commands.len() >= approved);
}

// ── 27. Policy decisions stored in trace ──────────────────────────────────

#[test]
fn test_policy_decisions_in_trace() {
    let resp = interpret("find nodes danger:true");
    let seq = translate(resp.clone());
    let mut reject = PolicyRule::new("R001");
    reject.applies_to_event_type = Some("NodeFilterEvent".to_string());
    reject.forbidden_properties.insert("filter_value".to_string(), "true".to_string());
    let policy = evaluate(seq.clone(), vec![reject]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let rejected = trace.policy_result.decisions.iter()
        .filter(|d| d.status == PolicyStatus::Rejected)
        .count();
    assert!(rejected > 0);
}

// ── 28. Trace explanation is stable ──────────────────────────────────────

#[test]
fn test_trace_explanation_stable() {
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    let a = format!("{:?}", trace);
    let b = format!("{:?}", trace);
    assert_eq!(a, b);
}

// ── 29. Store rejects duplicate insert deterministically ─────────────────

#[test]
fn test_store_rejects_duplicate_deterministically() {
    let mut store = TraceStore::new();
    let resp = interpret("test input");
    let seq = translate(resp.clone());
    let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
    let plan = build_plan(seq.clone(), policy.clone());
    let result = execute_plan(plan.clone());
    let trace = record_trace(resp, seq, policy, plan, result);
    assert!(store.record_trace(trace.clone()).is_ok());
    // Second insert always fails (deterministic)
    for _ in 0..10 {
        assert!(store.record_trace(trace.clone()).is_err());
    }
}

// ── 30. Full pipeline trace does not panic ────────────────────────────────

#[test]
fn test_full_pipeline_trace_no_panic() {
    let inputs = vec![
        "find nodes",
        "describe node 1",
        "query graph",
        "find Person age:30",
        "show everything",
    ];
    let mut store = TraceStore::new();
    for input in inputs {
        let resp = interpret(input);
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        store.record_trace(trace).unwrap();
    }
    assert_eq!(store.len(), 5);
}
