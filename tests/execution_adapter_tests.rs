use strata_kill_test::cognition::execution_adapter::*;
use strata_kill_test::cognition::event_translator::*;
use strata_kill_test::cognition::semantic_interpreter::*;
use strata_kill_test::cognition::policy::*;
use strata_kill_test::kernel::{Kernel, Event};
use std::collections::BTreeMap;

// ── 1. Identical input → identical plan ───────────────────────────────────

#[test]
fn test_identical_input_identical_plan() {
    let resp = interpret("find nodes");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule.clone()]);
    let a = build_plan(seq.clone(), policy.clone());
    let b = build_plan(seq, policy);
    assert_eq!(a, b);
}

// ── 2. 100-run stability ──────────────────────────────────────────────────

#[test]
fn test_100_run_stability() {
    let resp = interpret("describe node 42");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let plan = build_plan(seq, policy);
    let first = execute_plan(plan.clone());
    for _ in 0..100 {
        let next = execute_plan(plan.clone());
        assert_eq!(first, next);
    }
}

// ── 3. Approved events become commands ────────────────────────────────────

#[test]
fn test_approved_events_become_commands() {
    let resp = interpret("describe node 42");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let plan = build_plan(seq, policy);
    assert!(plan.commands.len() > 0);
    for cmd in &plan.commands {
        assert_ne!(cmd.command_type, "NoOpCommand", "approved events should not become NoOp");
    }
}

// ── 4. Rejected events excluded from plan ─────────────────────────────────

#[test]
fn test_rejected_events_excluded() {
    let resp = interpret("find nodes danger:true");
    let seq = translate(resp);
    let mut reject_rule = PolicyRule::new("R001");
    reject_rule.applies_to_event_type = Some("NodeFilterEvent".to_string());
    reject_rule.forbidden_properties.insert("filter_value".to_string(), "true".to_string());
    let policy = evaluate(seq.clone(), vec![reject_rule]);
    let plan = build_plan(seq, policy);
    for cmd in &plan.commands {
        assert_ne!(cmd.command_type, "FilterNodes");
    }
}

// ── 5. Deferred events become NoOpCommand ─────────────────────────────────

#[test]
fn test_deferred_events_become_noop() {
    let resp = interpret("find nodes");
    let seq = translate(resp);
    let mut rule = PolicyRule::new("R001");
    rule.applies_to_event_type = Some("NoOp".to_string());
    let policy = evaluate(seq.clone(), vec![rule]);
    let plan = build_plan(seq, policy);
    for cmd in &plan.commands {
        assert_eq!(cmd.command_type, "NoOpCommand");
        assert!(cmd.payload.is_empty());
    }
}

// ── 6. Event order preserved in command order ─────────────────────────────

#[test]
fn test_event_order_preserved() {
    let resp = interpret("describe node 5 1 3");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let plan = build_plan(seq, policy);
    for i in 1..plan.commands.len() {
        assert!(plan.commands[i - 1].event_id <= plan.commands[i].event_id);
    }
}

// ── 7. All event types map to correct commands ────────────────────────────

#[test]
fn test_event_type_mapping() {
    let event_types = vec![
        "GraphQueryRequested",
        "OntologyQueryRequested",
        "SemanticQueryRequested",
        "NodeDescribeRequested",
        "GraphDescribeRequested",
        "NodeSelectionEvent",
        "EdgeSelectionEvent",
        "NodeFilterEvent",
        "EdgeFilterEvent",
        "NoOp",
    ];
    let expected = vec![
        "QueryGraph",
        "QueryOntology",
        "QuerySemantic",
        "DescribeNode",
        "DescribeGraph",
        "SelectNode",
        "SelectEdge",
        "FilterNodes",
        "FilterEdges",
        "NoOpCommand",
    ];
    let intent = strata_kill_test::cognition::semantic_interpreter::types::SemanticIntent::new(
        "test", strata_kill_test::cognition::semantic_interpreter::types::IntentType::QueryGraph,
        BTreeMap::new(), BTreeMap::new(),
    );
    let proposed: Vec<_> = event_types.iter().enumerate().map(|(i, et)| {
        strata_kill_test::cognition::event_translator::types::ProposedEvent::new(
            &format!("evt_{}", i), et, BTreeMap::new(), &intent.id,
        )
    }).collect();
    let seq = strata_kill_test::cognition::event_translator::types::ProposedEventSequence::new(
        intent, None, proposed,
    );
    let decisions: Vec<_> = seq.events.iter().map(|e| {
        PolicyDecision::new(&e.id, PolicyStatus::Approved, "test")
    }).collect();
    let policy = PolicyEvaluationResult::new(seq.clone(), decisions);
    let plan = build_plan(seq, policy);
    for (cmd, expected_type) in plan.commands.iter().zip(expected.iter()) {
        assert_eq!(cmd.command_type, *expected_type, "mismatch for event type {}", cmd.event_id);
    }
}

// ── 8. Execute plan returns ExecutionPlanResult ───────────────────────────

#[test]
fn test_execute_plan_returns_result() {
    let resp = interpret("find nodes");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let plan = build_plan(seq, policy);
    let result = execute_plan(plan);
    assert_eq!(result.results.len(), result.plan.commands.len());
    assert!(result.explanation.contains("commands="));
}

// ── 9. Execution result contains ABI-aligned output ───────────────────────

#[test]
fn test_execution_result_abi_aligned() {
    let cmd = KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0");
    let plan = ExecutionPlan::new("seq", vec![cmd]);
    let result = execute_plan(plan);
    for r in &result.results {
        assert!(r.command_id.starts_with("cmd:"));
    }
}

// ── 10. No mutation of policy after build_plan ────────────────────────────

#[test]
fn test_no_mutation_of_policy() {
    let resp = interpret("find nodes");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let original_len = policy.decisions.len();
    let _ = build_plan(seq, policy.clone());
    assert_eq!(policy.decisions.len(), original_len);
}

// ── 11. No mutation of sequence after build_plan ──────────────────────────

#[test]
fn test_no_mutation_of_sequence() {
    let resp = interpret("find nodes");
    let seq = translate(resp.clone());
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let original_len = seq.events.len();
    let _ = build_plan(seq, policy);
    let seq2 = translate(resp);
    assert_eq!(seq2.events.len(), original_len);
}

// ── 12. Kernel integration via StrataEngine ────────────────────────────────

#[test]
fn test_kernel_integration_path() {
    let mut kernel = Kernel::new();
    kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "test".into() }).unwrap();
    let engine = StrataEngine::with_kernel(kernel);
    let cmd = KernelCommand::new("cmd:0", "DescribeNode", {
        let mut p = BTreeMap::new();
        p.insert("node_id".to_string(), "1".to_string());
        p
    }, "trace", "evt_0");
    let envelope = CommandEnvelope::new(cmd);
    let result = engine.execute(&envelope);
    assert!(result.success);
    assert!(result.result.contains("node 1"));
}

// ── 13. No direct kernel access (via ABI only) ────────────────────────────

#[test]
fn test_no_bypass_of_abi() {
    let cmd = KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0");
    let envelope = CommandEnvelope::new(cmd);
    // Must go through CommandEnvelope → StrataEngine → CommandResultV1
    let engine = StrataEngine::new();
    let result = engine.execute(&envelope);
    assert_eq!(result.command_id, "cmd:0");
    assert!(result.success);
    assert_eq!(result.result, "NoOp executed");
}

// ── 14. ExecutionPlan roundtrip serialization ─────────────────────────────

#[test]
fn test_execution_plan_roundtrip() {
    let cmd = KernelCommand::new("cmd:0", "QueryGraph", BTreeMap::new(), "trace", "evt_0");
    let plan = ExecutionPlan::new("seq", vec![cmd]);
    let json = serde_json::to_string(&plan).unwrap();
    let parsed: ExecutionPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(plan, parsed);
}

// ── 15. ExecutionPlanResult roundtrip serialization ───────────────────────

#[test]
fn test_execution_plan_result_roundtrip() {
    let cmd = KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0");
    let plan = ExecutionPlan::new("seq", vec![cmd]);
    let result = execute_plan(plan);
    let json = serde_json::to_string(&result).unwrap();
    let parsed: ExecutionPlanResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result.explanation, parsed.explanation);
    assert_eq!(result.results.len(), parsed.results.len());
}

// ── 16. CommandEnvelope roundtrip serialization ───────────────────────────

#[test]
fn test_command_envelope_roundtrip() {
    let cmd = KernelCommand::new("cmd:0", "QueryGraph", BTreeMap::new(), "trace", "evt_0");
    let envelope = CommandEnvelope::new(cmd);
    let json = serde_json::to_string(&envelope).unwrap();
    let parsed: CommandEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(envelope, parsed);
}

// ── 17. CommandResultV1 roundtrip serialization ──────────────────────────

#[test]
fn test_command_result_v1_roundtrip() {
    let r = CommandResultV1::success("cmd:0", "done");
    let json = serde_json::to_string(&r).unwrap();
    let parsed: CommandResultV1 = serde_json::from_str(&json).unwrap();
    assert_eq!(r, parsed);
}

// ── 18. Empty plan execution ──────────────────────────────────────────────

#[test]
fn test_empty_plan_execution() {
    let plan = ExecutionPlan::new("empty", vec![]);
    let result = execute_plan(plan);
    assert_eq!(result.results.len(), 0);
    assert!(result.explanation.contains("commands=0"));
    assert!(result.explanation.contains("executed=0"));
    assert!(result.explanation.contains("failed=0"));
}

// ── 19. Execution explanation format correct ──────────────────────────────

#[test]
fn test_execution_explanation_format() {
    let cmds = vec![
        KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0"),
        KernelCommand::new("cmd:1", "NoOpCommand", BTreeMap::new(), "trace", "evt_1"),
    ];
    let plan = ExecutionPlan::new("seq", cmds);
    let result = execute_plan(plan);
    assert_eq!(result.explanation, "commands=2; executed=2; failed=0");
}

// ── 20. Modified events retain payload ────────────────────────────────────

#[test]
fn test_modified_events_retain_payload() {
    let mut payload = BTreeMap::new();
    payload.insert("node_id".to_string(), "42".to_string());
    let intent = strata_kill_test::cognition::semantic_interpreter::types::SemanticIntent::new(
        "test", strata_kill_test::cognition::semantic_interpreter::types::IntentType::DescribeNode,
        BTreeMap::new(), BTreeMap::new(),
    );
    let evt = strata_kill_test::cognition::event_translator::types::ProposedEvent::new(
        "evt_0", "NodeDescribeRequested", payload.clone(), &intent.id,
    );
    let seq = strata_kill_test::cognition::event_translator::types::ProposedEventSequence::new(
        intent, None, vec![evt],
    );
    let decisions = vec![PolicyDecision::new("evt_0", PolicyStatus::Modified, "test")];
    let policy = PolicyEvaluationResult::new(seq.clone(), decisions);
    let plan = build_plan(seq, policy);
    assert_eq!(plan.commands.len(), 1);
    assert_eq!(plan.commands[0].command_type, "DescribeNode");
    assert!(plan.commands[0].payload.contains_key("node_id"));
}

// ── 21. StrataEngine handles unknown command type ─────────────────────────

#[test]
fn test_strata_engine_unknown_command() {
    let cmd = KernelCommand::new("cmd:0", "BogusType", BTreeMap::new(), "trace", "evt_0");
    let envelope = CommandEnvelope::new(cmd);
    let engine = StrataEngine::new();
    let result = engine.execute(&envelope);
    assert!(!result.success);
    assert!(result.error.is_some());
}

// ── 22. Execution result order matches plan command order ─────────────────

#[test]
fn test_result_order_matches_command_order() {
    let cmds = vec![
        KernelCommand::new("cmd:a", "NoOpCommand", BTreeMap::new(), "trace", "evt_a"),
        KernelCommand::new("cmd:b", "NoOpCommand", BTreeMap::new(), "trace", "evt_b"),
    ];
    let plan = ExecutionPlan::new("seq", cmds);
    let result = execute_plan(plan);
    for (i, r) in result.results.iter().enumerate() {
        assert_eq!(r.command_id, format!("cmd:{}", if i == 0 { 'a' } else { 'b' }));
    }
}

// ── 23. Full pipeline: interpret → translate → evaluate → build → execute ─

#[test]
fn test_full_pipeline_end_to_end() {
    let resp = interpret("find nodes");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let plan = build_plan(seq, policy);
    let result = execute_plan(plan);
    assert!(result.results.len() > 0);
    for r in &result.results {
        assert!(r.success);
    }
}

// ── 24. Plan explanation matches command count ────────────────────────────

#[test]
fn test_plan_explanation_matches_count() {
    let resp = interpret("describe node 42");
    let seq = translate(resp);
    let rule = PolicyRule::new("R001");
    let policy = evaluate(seq.clone(), vec![rule]);
    let plan = build_plan(seq, policy);
    assert_eq!(plan.explanation, format!("commands={}", plan.commands.len()));
}

// ── 25. Kernel query returns meaningful result ────────────────────────────

#[test]
fn test_kernel_query_meaningful_result() {
    let mut kernel = Kernel::new();
    kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "Person".into() }).unwrap();
    kernel.propose_and_commit(Event::CreateNode { id: 2, node_type: "Place".into() }).unwrap();
    let engine = StrataEngine::with_kernel(kernel);
    let cmd = KernelCommand::new("cmd:0", "QueryGraph", BTreeMap::new(), "trace", "evt_0");
    let envelope = CommandEnvelope::new(cmd);
    let result = engine.execute(&envelope);
    assert!(result.success);
    assert!(result.result.contains("nodes=2"));
}
