use std::collections::{BTreeMap, BTreeSet};

use crate::cognition::trace::types::TraceRecord;
use crate::cognition::policy::types::PolicyStatus;

use super::types::*;

fn check_entity_stability(
    traces: &[&TraceRecord],
    violations: &mut Vec<TemporalViolation>,
) {
    let mut entity_types: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    for trace in traces {
        for (entity, etype) in &trace.semantic_response.intent.extracted_entities {
            entity_types
                .entry(entity.clone())
                .or_default()
                .entry(etype.clone())
                .or_default()
                .insert(trace.trace_id.clone());
        }
    }

    for (entity, type_map) in &entity_types {
        if type_map.len() > 1 {
            let mut all_ids: Vec<String> = type_map.values().flat_map(|ids| ids.iter().cloned()).collect();
            all_ids.sort();
            let types: Vec<&str> = type_map.keys().map(|s| s.as_str()).collect();
            violations.push(TemporalViolation::new(
                "ENT001",
                &format!(
                    "Entity '{}' has inconsistent types across traces: {:?}",
                    entity, types
                ),
                all_ids,
            ));
        }
    }
}

fn check_event_structure_stability(
    traces: &[&TraceRecord],
    violations: &mut Vec<TemporalViolation>,
) {
    let mut event_to_command: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    for trace in traces {
        for event in &trace.proposed_sequence.events {
            if let Some(cmd) = trace.execution_plan.commands.iter().find(|c| c.event_id == event.id) {
                event_to_command
                    .entry(event.event_type.clone())
                    .or_default()
                    .entry(cmd.command_type.clone())
                    .or_default()
                    .insert(trace.trace_id.clone());
            }
        }
    }

    for (event_type, cmd_map) in &event_to_command {
        if cmd_map.len() > 1 {
            let all_ids: Vec<String> = cmd_map.values().flat_map(|ids| ids.iter().cloned()).collect();
            let types: Vec<&str> = cmd_map.keys().map(|s| s.as_str()).collect();
            violations.push(TemporalViolation::new(
                "EVT-T001",
                &format!(
                    "Event type '{}' maps to inconsistent command types across traces: {:?}",
                    event_type, types
                ),
                all_ids,
            ));
        }
    }
}

fn event_sequence_signature(trace: &TraceRecord) -> (Vec<String>, Vec<String>) {
    let event_types: Vec<String> = trace.proposed_sequence.events.iter()
        .map(|e| e.event_type.clone())
        .collect();
    let event_ids: Vec<String> = trace.proposed_sequence.events.iter()
        .map(|e| e.id.clone())
        .collect();
    (event_types, event_ids)
}

fn plan_semantic_signature(plan: &crate::cognition::execution_adapter::types::ExecutionPlan) -> Vec<String> {
    plan.commands.iter().map(|c| c.command_type.clone()).collect()
}

fn check_execution_determinism_drift(
    traces: &[&TraceRecord],
    violations: &mut Vec<TemporalViolation>,
) {
    let mut groups: BTreeMap<(Vec<String>, Vec<String>), Vec<&TraceRecord>> = BTreeMap::new();

    for trace in traces {
        let sig = event_sequence_signature(trace);
        groups.entry(sig).or_default().push(trace);
    }

    for (_sig, group) in &groups {
        if group.len() < 2 {
            continue;
        }
        let baseline_sig = plan_semantic_signature(&group[0].execution_plan);
        for trace in group.iter().skip(1) {
            let trace_sig = plan_semantic_signature(&trace.execution_plan);
            if trace_sig != baseline_sig {
                let ids: Vec<String> = group.iter().map(|t| t.trace_id.clone()).collect();
                violations.push(TemporalViolation::new(
                    "EXE-T001",
                    "Execution plans diverge for identical event sequences",
                    ids,
                ));
                break;
            }
        }
    }
}

fn check_policy_stability(
    traces: &[&TraceRecord],
    violations: &mut Vec<TemporalViolation>,
) {
    let mut event_decisions: BTreeMap<String, BTreeMap<PolicyStatus, BTreeSet<String>>> = BTreeMap::new();

    // Group by event.event_type + event.id to identify "same" events across traces
    for trace in traces {
        for event in &trace.proposed_sequence.events {
            if let Some(dec) = trace.policy_result.decisions.iter().find(|d| d.event_id == event.id) {
                let key = format!("{}:{}", event.event_type, event.id);
                event_decisions
                    .entry(key)
                    .or_default()
                    .entry(dec.status.clone())
                    .or_default()
                    .insert(trace.trace_id.clone());
            }
        }
    }

    for (key, status_map) in &event_decisions {
        if status_map.len() > 1 {
            let all_ids: Vec<String> = status_map.values().flat_map(|ids| ids.iter().cloned()).collect();
            let statuses: Vec<&PolicyStatus> = status_map.keys().collect();
            violations.push(TemporalViolation::new(
                "POL-T001",
                &format!(
                    "Event '{}' gets inconsistent policy decisions across traces: {:?}",
                    key, statuses
                ),
                all_ids,
            ));
        }
    }
}

fn check_kernel_transition_stability(
    traces: &[&TraceRecord],
    violations: &mut Vec<TemporalViolation>,
) {
    let mut cmd_to_results: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    for trace in traces {
        for (cmd, res) in trace.execution_plan.commands.iter()
            .zip(trace.execution_result.results.iter())
        {
            cmd_to_results
                .entry(cmd.command_type.clone())
                .or_default()
                .entry(res.kernel_result.clone())
                .or_default()
                .insert(trace.trace_id.clone());
        }
    }

    for (cmd_type, result_map) in &cmd_to_results {
        if result_map.len() > 1 {
            let all_ids: Vec<String> = result_map.values().flat_map(|ids| ids.iter().cloned()).collect();
            violations.push(TemporalViolation::new(
                "KRN-T001",
                &format!("Command type '{}' produces different kernel results across traces", cmd_type),
                all_ids,
            ));
        }
    }
}

fn check_trace_consistency(
    traces: &[&TraceRecord],
    violations: &mut Vec<TemporalViolation>,
) {
    if traces.is_empty() {
        return;
    }

    let baseline_stages: BTreeSet<&str> = BTreeSet::from([
        "semantic_response",
        "proposed_sequence",
        "policy_result",
        "execution_plan",
        "execution_result",
    ]);

    let has_semantic = true;
    let mut has_proposed = true;
    let mut has_policy = true;
    let mut has_plan = true;
    let mut has_result = true;

    for trace in traces {
        if trace.proposed_sequence.events.is_empty() { has_proposed = false; }
        if trace.policy_result.decisions.is_empty() { has_policy = false; }
        if trace.execution_plan.commands.is_empty() { has_plan = false; }
        if trace.execution_result.results.is_empty() { has_result = false; }
    }

    let mut missing: Vec<&str> = Vec::new();
    if !has_semantic { missing.push("semantic_response"); }
    if !has_proposed { missing.push("proposed_sequence"); }
    if !has_policy { missing.push("policy_result"); }
    if !has_plan { missing.push("execution_plan"); }
    if !has_result { missing.push("execution_result"); }

    if !missing.is_empty() {
        let all_ids: Vec<String> = traces.iter().map(|t| t.trace_id.clone()).collect();
        violations.push(TemporalViolation::new(
            "TRC-T001",
            &format!(
                "Traces missing stages: {:?}. Expected all of {:?}",
                missing, baseline_stages
            ),
            all_ids,
        ));
    }
}

pub fn analyze_temporal_consistency(window: TraceWindow) -> TemporalReport {
    let window_id = window
        .traces
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "empty".to_string());

    let traces: Vec<&TraceRecord> = window.all_traces();
    let mut violations: Vec<TemporalViolation> = Vec::new();

    if traces.is_empty() {
        let score = 0.0;
        return TemporalReport::new(&window_id, score, violations);
    }

    check_entity_stability(&traces, &mut violations);
    check_event_structure_stability(&traces, &mut violations);
    check_execution_determinism_drift(&traces, &mut violations);
    check_policy_stability(&traces, &mut violations);
    check_kernel_transition_stability(&traces, &mut violations);
    check_trace_consistency(&traces, &mut violations);

    violations.sort();

    let total_rules = 6u64;
    let mut passed = total_rules;

    let codes: BTreeSet<&str> = violations.iter().map(|v| v.code.as_str()).collect();

    let rule_prefixes = ["ENT", "EVT-T", "EXE-T", "POL-T", "KRN-T", "TRC-T"];
    for prefix in &rule_prefixes {
        if codes.iter().any(|c| c.starts_with(prefix)) {
            passed -= 1;
        }
    }

    let score = passed as f64 / total_rules as f64;
    TemporalReport::new(&window_id, score, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::semantic_interpreter::engine::interpret;
    use crate::cognition::event_translator::engine::translate;
    use crate::cognition::policy::engine::evaluate;
    use crate::cognition::policy::types::PolicyRule;
    use crate::cognition::execution_adapter::engine::{build_plan, execute_plan};
    use crate::cognition::trace::engine::record_trace;

    fn make_trace(input: &str) -> TraceRecord {
        let resp = interpret(input);
        let seq = translate(resp.clone());
        let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        record_trace(resp, seq, policy, plan, result)
    }

    #[test]
    fn test_empty_window() {
        let window = TraceWindow::new();
        let report = analyze_temporal_consistency(window);
        assert!((report.score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_single_trace_window() {
        let mut window = TraceWindow::new();
        window.add(make_trace("find nodes"));
        let report = analyze_temporal_consistency(window);
        assert!(report.is_valid);
    }

    #[test]
    fn test_identical_traces_no_violations() {
        let mut window = TraceWindow::new();
        window.add(make_trace("find nodes"));
        window.add(make_trace("find nodes"));
        window.add(make_trace("find nodes"));
        let report = analyze_temporal_consistency(window);
        assert!(report.is_valid);
        assert!((report.score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mixed_traces_valid() {
        let mut window = TraceWindow::new();
        window.add(make_trace("find nodes"));
        window.add(make_trace("describe node 42"));
        window.add(make_trace("query graph"));
        let report = analyze_temporal_consistency(window);
        assert!(report.is_valid);
    }

    #[test]
    fn test_deterministic_identical_windows() {
        let mut a = TraceWindow::new();
        a.add(make_trace("test"));
        let mut b = TraceWindow::new();
        b.add(make_trace("test"));
        let ra = analyze_temporal_consistency(a);
        let rb = analyze_temporal_consistency(b);
        assert_eq!(ra, rb);
    }

    #[test]
    fn test_reorder_trace_insertion_no_effect() {
        let mut a = TraceWindow::new();
        a.add(make_trace("z"));
        a.add(make_trace("a"));
        a.add(make_trace("m"));
        let mut b = TraceWindow::new();
        b.add(make_trace("a"));
        b.add(make_trace("m"));
        b.add(make_trace("z"));
        assert_eq!(
            analyze_temporal_consistency(a),
            analyze_temporal_consistency(b)
        );
    }

    #[test]
    fn test_100_run_stability() {
        let mut window = TraceWindow::new();
        window.add(make_trace("find nodes"));
        window.add(make_trace("describe node 42"));
        let first = analyze_temporal_consistency(window.clone());
        for _ in 0..100 {
            let next = analyze_temporal_consistency(window.clone());
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut window = TraceWindow::new();
        window.add(make_trace("test"));
        let report = analyze_temporal_consistency(window);
        let json = serde_json::to_string(&report).unwrap();
        let parsed: TemporalReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, parsed);
    }

    #[test]
    fn test_score_stable_across_runs() {
        let mut window = TraceWindow::new();
        window.add(make_trace("find nodes"));
        window.add(make_trace("describe node 42"));
        let scores: Vec<f64> = (0..10)
            .map(|_| analyze_temporal_consistency(window.clone()).score)
            .collect();
        for i in 1..scores.len() {
            assert_eq!(scores[0].to_bits(), scores[i].to_bits());
        }
    }

    #[test]
    fn test_violation_ordering_stable() {
        let mut window = TraceWindow::new();
        // Create a window with entity drift by manually constructing traces
        let mut t1 = make_trace("find Node");
        t1.trace_id = "unique_a".to_string();
        t1.semantic_response.intent.extracted_entities.insert("Node".to_string(), "type_a".to_string());
        let mut t2 = make_trace("find Node");
        t2.trace_id = "unique_b".to_string();
        t2.semantic_response.intent.extracted_entities.insert("Node".to_string(), "type_b".to_string());
        window.add(t1);
        window.add(t2);
        let a = analyze_temporal_consistency(window.clone());
        let b = analyze_temporal_consistency(window);
        assert_eq!(a.violations, b.violations);
    }

    #[test]
    fn test_entity_drift_detected() {
        let mut window = TraceWindow::new();
        let mut t1 = make_trace("find Person");
        t1.trace_id = "entity_drift_1".to_string();
        t1.semantic_response.intent.extracted_entities.insert("Person".to_string(), "human".to_string());
        let mut t2 = make_trace("find Person");
        t2.trace_id = "entity_drift_2".to_string();
        t2.semantic_response.intent.extracted_entities.insert("Person".to_string(), "robot".to_string());
        window.add(t1);
        window.add(t2);
        let report = analyze_temporal_consistency(window);
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.code == "ENT001"));
    }

    #[test]
    fn test_entity_consistency_across_traces() {
        let mut window = TraceWindow::new();
        let mut t1 = make_trace("find Person");
        t1.trace_id = "entity_cons_1".to_string();
        t1.semantic_response.intent.extracted_entities.insert("Person".to_string(), "human".to_string());
        let mut t2 = make_trace("find Person");
        t2.trace_id = "entity_cons_2".to_string();
        t2.semantic_response.intent.extracted_entities.insert("Person".to_string(), "human".to_string());
        window.add(t1);
        window.add(t2);
        let report = analyze_temporal_consistency(window);
        assert!(report.is_valid);
    }

    #[test]
    fn test_event_drift_detected() {
        let mut window = TraceWindow::new();
        let mut t1 = make_trace("find nodes");
        t1.trace_id = "event_drift_1".to_string();
        if let Some(cmd) = t1.execution_plan.commands.first_mut() {
            cmd.command_type = "WrongType".to_string();
        }
        let mut t2 = make_trace("find nodes");
        t2.trace_id = "event_drift_2".to_string();
        window.add(t1);
        window.add(t2);
        let report = analyze_temporal_consistency(window);
        assert!(!report.is_valid);
    }

    #[test]
    fn test_execution_drift_detected() {
        let mut window = TraceWindow::new();
        let mut t1 = make_trace("find nodes");
        t1.trace_id = "exec_drift_1".to_string();
        if let Some(cmd) = t1.execution_plan.commands.first_mut() {
            cmd.command_type = "DifferentCmd".to_string();
        }
        let mut t2 = make_trace("find nodes");
        t2.trace_id = "exec_drift_2".to_string();
        window.add(t1);
        window.add(t2);
        let report = analyze_temporal_consistency(window);
        assert!(!report.is_valid);
    }
}
