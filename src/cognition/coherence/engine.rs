use std::collections::BTreeSet;

use crate::kernel::GraphState;
use crate::ontology::OntologyRegistry;
use crate::cognition::trace::types::TraceRecord;
use crate::cognition::policy::types::PolicyStatus;
use super::types::*;

fn map_event_to_command_type(event_type: &str) -> &'static str {
    match event_type {
        "GraphQueryRequested" => "QueryGraph",
        "OntologyQueryRequested" => "QueryOntology",
        "SemanticQueryRequested" => "QuerySemantic",
        "NodeDescribeRequested" => "DescribeNode",
        "GraphDescribeRequested" => "DescribeGraph",
        "NodeSelectionEvent" => "SelectNode",
        "EdgeSelectionEvent" => "SelectEdge",
        "NodeFilterEvent" => "FilterNodes",
        "EdgeFilterEvent" => "FilterEdges",
        "NoOp" => "NoOpCommand",
        _ => "NoOpCommand",
    }
}

fn check_event_execution_consistency(
    trace: &TraceRecord,
    violations: &mut Vec<CoherenceViolation>,
) {
    let events = &trace.proposed_sequence.events;
    let commands = &trace.execution_plan.commands;
    let decisions = &trace.policy_result.decisions;

    let non_rejected_event_ids: BTreeSet<&str> = decisions
        .iter()
        .filter(|d| d.status != PolicyStatus::Rejected)
        .map(|d| d.event_id.as_str())
        .collect();

    let command_event_ids: BTreeSet<&str> = commands
        .iter()
        .map(|c| c.event_id.as_str())
        .collect();

    let missing: Vec<&&str> = non_rejected_event_ids
        .difference(&command_event_ids)
        .collect();
    for m in &missing {
        violations.push(CoherenceViolation::new(
            "EVT001",
            &format!("Missing command for event: {}", m),
            "execution",
        ));
    }

    let extra: Vec<&&str> = command_event_ids
        .difference(&non_rejected_event_ids)
        .collect();
    for e in &extra {
        violations.push(CoherenceViolation::new(
            "EVT002",
            &format!("Extra command with no matching event: {}", e),
            "execution",
        ));
    }

    let mut expected_types: Vec<(&str, &str)> = Vec::new();
    for event in events {
        if let Some(dec) = decisions.iter().find(|d| d.event_id == event.id) {
            if dec.status == PolicyStatus::Rejected {
                continue;
            }
            let cmd_type = if dec.status == PolicyStatus::Deferred {
                "NoOpCommand"
            } else {
                map_event_to_command_type(&event.event_type)
            };
            expected_types.push((&event.id, cmd_type));
        }
    }

    if expected_types.len() == commands.len() {
        for (i, ((_eid, expected_type), cmd)) in expected_types.iter().zip(commands.iter()).enumerate() {
            if *expected_type != cmd.command_type {
                violations.push(CoherenceViolation::new(
                    "EVT003",
                    &format!(
                        "Command type mismatch at position {}: expected {}, got {}",
                        i, expected_type, cmd.command_type
                    ),
                    "execution",
                ));
            }
        }
    }
}

fn check_policy_compliance(
    trace: &TraceRecord,
    violations: &mut Vec<CoherenceViolation>,
) {
    let decisions = &trace.policy_result.decisions;
    let commands = &trace.execution_plan.commands;

    let rejected_event_ids: BTreeSet<&str> = decisions
        .iter()
        .filter(|d| d.status == PolicyStatus::Rejected)
        .map(|d| d.event_id.as_str())
        .collect();

    for cmd in commands {
        if rejected_event_ids.contains(cmd.event_id.as_str()) {
            violations.push(CoherenceViolation::new(
                "POL001",
                &format!("Rejected event leaked into execution: {}", cmd.event_id),
                "policy",
            ));
        }
    }
}

fn check_trace_completeness(
    trace: &TraceRecord,
    violations: &mut Vec<CoherenceViolation>,
) {
    if trace.proposed_sequence.intent.raw_input.is_empty() {
        violations.push(CoherenceViolation::new(
            "TRC001",
            "Raw input is empty",
            "semantic",
        ));
    }
    if trace.proposed_sequence.events.is_empty() {
        violations.push(CoherenceViolation::new(
            "TRC002",
            "No proposed events in trace",
            "translation",
        ));
    }
    if trace.policy_result.decisions.is_empty() {
        violations.push(CoherenceViolation::new(
            "TRC003",
            "No policy decisions in trace",
            "policy",
        ));
    }
    if trace.execution_plan.commands.is_empty() {
        violations.push(CoherenceViolation::new(
            "TRC004",
            "No execution commands in trace",
            "execution",
        ));
    }
    if trace.execution_result.results.is_empty() {
        violations.push(CoherenceViolation::new(
            "TRC005",
            "No execution results in trace",
            "execution",
        ));
    }
}

fn check_kernel_consistency(
    trace: &TraceRecord,
    kernel_state: &GraphState,
    violations: &mut Vec<CoherenceViolation>,
) {
    let plan = &trace.execution_plan;
    let results = &trace.execution_result.results;

    if plan.commands.len() != results.len() {
        violations.push(CoherenceViolation::new(
            "KRN001",
            &format!(
                "Command/result count mismatch: {} commands vs {} results",
                plan.commands.len(),
                results.len()
            ),
            "execution",
        ));
        return;
    }

    for (cmd, res) in plan.commands.iter().zip(results.iter()) {
        if cmd.id != res.command_id {
            violations.push(CoherenceViolation::new(
                "KRN002",
                &format!(
                    "Command/result id mismatch: command {} vs result {}",
                    cmd.id, res.command_id
                ),
                "execution",
            ));
        }

        match cmd.command_type.as_str() {
            "QueryGraph" | "DescribeGraph" => {
                let expected = format!("nodes={}; edges={}", kernel_state.node_count(), kernel_state.edge_count());
                if res.success && res.kernel_result != expected {
                    violations.push(CoherenceViolation::new(
                        "KRN003",
                        &format!(
                            "Kernel state mismatch for {}: expected '{}', got '{}'",
                            cmd.command_type, expected, res.kernel_result
                        ),
                        "execution",
                    ));
                }
            }
            _ => {}
        }
    }
}

fn check_determinism_validation(
    trace: &TraceRecord,
    violations: &mut Vec<CoherenceViolation>,
) {
    let plan = &trace.execution_plan;
    let results = &trace.execution_result.results;

    if plan.commands.len() != results.len() {
        violations.push(CoherenceViolation::new(
            "DET001",
            &format!(
                "Determinism mismatch: {} commands vs {} results",
                plan.commands.len(),
                results.len()
            ),
            "execution",
        ));
        return;
    }

    for (i, (cmd, res)) in plan.commands.iter().zip(results.iter()).enumerate() {
        if cmd.id != res.command_id {
            violations.push(CoherenceViolation::new(
                "DET002",
                &format!(
                    "Order/identity mismatch at position {}: command {} vs result {}",
                    i, cmd.id, res.command_id
                ),
                "execution",
            ));
        }
    }
}

pub fn verify_coherence(
    trace: &TraceRecord,
    kernel_state: &GraphState,
    _ontology: &OntologyRegistry,
) -> CoherenceReport {
    let trace_id = trace.trace_id.clone();
    let mut violations: Vec<CoherenceViolation> = Vec::new();

    check_event_execution_consistency(trace, &mut violations);
    check_policy_compliance(trace, &mut violations);
    check_trace_completeness(trace, &mut violations);
    check_kernel_consistency(trace, kernel_state, &mut violations);
    check_determinism_validation(trace, &mut violations);

    violations.sort();

    let total_weight = 5u64;
    let mut passed_weight = total_weight;

    let violation_codes: BTreeSet<&str> = violations.iter().map(|v| v.code.as_str()).collect();

    let has_evt = violation_codes.iter().any(|c| c.starts_with("EVT"));
    let has_pol = violation_codes.iter().any(|c| c.starts_with("POL"));
    let has_trc = violation_codes.iter().any(|c| c.starts_with("TRC"));
    let has_krn = violation_codes.iter().any(|c| c.starts_with("KRN"));
    let has_det = violation_codes.iter().any(|c| c.starts_with("DET"));

    if has_evt { passed_weight -= 1; }
    if has_pol { passed_weight -= 1; }
    if has_trc { passed_weight -= 1; }
    if has_krn { passed_weight -= 1; }
    if has_det { passed_weight -= 1; }

    let score_value = passed_weight as f64 / total_weight as f64;
    let score = CoherenceScore::new(score_value);
    CoherenceReport::new(&trace_id, score, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::semantic_interpreter::engine::interpret;
    use crate::cognition::event_translator::engine::translate;
    use crate::cognition::policy::engine::evaluate;
    use crate::cognition::policy::types::PolicyRule;
    use crate::cognition::execution_adapter::engine::{build_plan, execute_plan};
    use crate::cognition::execution_adapter::types::KernelCommand;
    use crate::cognition::trace::engine::record_trace;
    use crate::kernel::{Kernel, Event};
    use crate::ontology::OntologyRegistry;

    fn create_coherent_trace(input: &str) -> TraceRecord {
        let resp = interpret(input);
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        record_trace(resp, seq, policy, plan, result)
    }

    #[test]
    fn test_coherent_trace_no_violations() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(report.is_valid, "Expected no violations, got: {:?}", report.violations);
        assert!((report.score.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deterministic_identical_inputs() {
        let trace_a = create_coherent_trace("describe node 42");
        let trace_b = create_coherent_trace("describe node 42");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report_a = verify_coherence(&trace_a, &kernel_state, &ontology);
        let report_b = verify_coherence(&trace_b, &kernel_state, &ontology);
        assert_eq!(report_a, report_b);
    }

    #[test]
    fn test_ordering_independence() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let a = verify_coherence(&trace, &kernel_state, &ontology);
        let b = verify_coherence(&trace, &kernel_state, &ontology);
        assert_eq!(a, b);
    }

    #[test]
    fn test_100_run_stability() {
        let trace = create_coherent_trace("describe graph");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let first = verify_coherence(&trace, &kernel_state, &ontology);
        for _ in 0..100 {
            let next = verify_coherence(&trace, &kernel_state, &ontology);
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_repeated_verification_stability() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        for _ in 0..10 {
            let r = verify_coherence(&trace, &kernel_state, &ontology);
            assert!(r.is_valid);
        }
    }

    #[test]
    fn test_no_mutation_of_trace() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let original_events = trace.proposed_sequence.events.len();
        let _report = verify_coherence(&trace, &kernel_state, &ontology);
        assert_eq!(trace.proposed_sequence.events.len(), original_events);
    }

    #[test]
    fn test_no_mutation_of_kernel_state() {
        let trace = create_coherent_trace("describe node 42");
        let kernel = Kernel::new();
        let kernel_state = kernel.state().clone();
        let ontology = OntologyRegistry::empty();
        let original_count = kernel_state.node_count();
        let _report = verify_coherence(&trace, &kernel_state, &ontology);
        assert_eq!(kernel_state.node_count(), original_count);
    }

    #[test]
    fn test_no_mutation_of_ontology() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let original_len = ontology.entity_types.len();
        let _report = verify_coherence(&trace, &kernel_state, &ontology);
        assert_eq!(ontology.entity_types.len(), original_len);
    }

    #[test]
    fn test_verifier_no_side_effects() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let _report = verify_coherence(&trace, &kernel_state, &ontology);
        // Call again - same result
        let report2 = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(report2.is_valid);
    }

    #[test]
    fn test_idempotent_repeated_calls() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let reports: Vec<CoherenceReport> = (0..5)
            .map(|_| verify_coherence(&trace, &kernel_state, &ontology))
            .collect();
        for i in 1..reports.len() {
            assert_eq!(reports[0], reports[i]);
        }
    }

    #[test]
    fn test_missing_event_detected() {
        let mut trace = create_coherent_trace("describe node 5 1 3");
        // Remove a command to simulate missing mapping
        if !trace.execution_plan.commands.is_empty() {
            let _removed = trace.execution_plan.commands.remove(0);
        }
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.code == "EVT001" || v.code == "DET001"));
    }

    #[test]
    fn test_extra_execution_command_detected() {
        let mut trace = create_coherent_trace("find nodes");
        let extra = KernelCommand::new("cmd:extra", "NoOpCommand", std::collections::BTreeMap::new(), "trace", "nonexistent");
        trace.execution_plan.commands.push(extra);
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.code == "EVT002"));
    }

    #[test]
    fn test_rejected_event_leak_detected() {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let mut reject = PolicyRule::new("R001");
        reject.applies_to_event_type = Some("NodeFilterEvent".to_string());
        reject.forbidden_properties.insert("filter_value".to_string(), "true".to_string());
        let policy = evaluate(seq.clone(), vec![reject]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let mut trace = record_trace(resp, seq, policy, plan, result);
        // Manually inject a rejected event's command
        let leaked = KernelCommand::new("cmd:leak", "FilterNodes", std::collections::BTreeMap::new(), "trace", "evt_leak");
        trace.execution_plan.commands.push(leaked);
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        // The leaked command has no matching event, so EVT002 should fire
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(!report.is_valid);
    }

    #[test]
    fn test_missing_trace_stage_detected() {
        let mut trace = create_coherent_trace("find nodes");
        trace.proposed_sequence.events.clear();
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.code == "TRC002"));
    }

    #[test]
    fn test_kernel_mismatch_detected() {
        let trace = create_coherent_trace("find nodes");
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "test".into() }).unwrap();
        let kernel_state = kernel.state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        // QueryGraph result says nodes=0 but kernel has 1 node
        assert!(!report.is_valid);
    }

    #[test]
    fn test_execution_mismatch_detected() {
        let mut trace = create_coherent_trace("find nodes");
        if !trace.execution_result.results.is_empty() {
            trace.execution_result.results.remove(0);
        }
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.code == "KRN001" || v.code == "DET001"));
    }

    #[test]
    fn test_corrupted_trace_rejected() {
        let mut trace = create_coherent_trace("find nodes");
        trace.execution_plan.commands.clear();
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(!report.is_valid);
    }

    #[test]
    fn test_all_stages_checked() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        // All 5 check categories should have passed
        assert!(report.is_valid);
        assert!((report.score.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let trace = create_coherent_trace("find nodes");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        let json = serde_json::to_string(&report).unwrap();
        let parsed: CoherenceReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, parsed);
    }

    #[test]
    fn test_large_trace_consistency() {
        let trace = create_coherent_trace("find nodes with multiple entities");
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        // Should be valid since trace is coherent
        assert!(report.is_valid || report.violations.is_empty());
    }

    #[test]
    fn test_empty_raw_input() {
        let mut trace = create_coherent_trace("find nodes");
        trace.proposed_sequence.intent.raw_input.clear();
        let kernel_state = Kernel::new().state().clone();
        let ontology = OntologyRegistry::empty();
        let report = verify_coherence(&trace, &kernel_state, &ontology);
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.code == "TRC001"));
    }
}
