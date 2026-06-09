use std::collections::BTreeMap;

use crate::cognition::event_translator::types::ProposedEventSequence;
use crate::cognition::policy::types::{PolicyEvaluationResult, PolicyStatus};
use crate::kernel::{Kernel, Node, Edge};

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

fn compute_event_payload(event: &crate::cognition::event_translator::types::ProposedEvent) -> BTreeMap<String, String> {
    event.payload.clone()
}

pub fn build_plan(
    sequence: ProposedEventSequence,
    policy: PolicyEvaluationResult,
) -> ExecutionPlan {
    let decisions = policy.decisions;
    let sequence_id = sequence.intent.id.clone();
    let mut commands = Vec::new();

    for (event, decision) in sequence.events.iter().zip(decisions.iter()) {
        match decision.status {
            PolicyStatus::Approved => {
                let cmd = KernelCommand::new(
                    &format!("cmd:{}", event.id),
                    map_event_to_command_type(&event.event_type),
                    compute_event_payload(event),
                    &sequence_id,
                    &event.id,
                );
                commands.push(cmd);
            }
            PolicyStatus::Rejected => {
                // rejected events are excluded from the command plan
            }
            PolicyStatus::Modified => {
                let cmd = KernelCommand::new(
                    &format!("cmd:{}", event.id),
                    map_event_to_command_type(&event.event_type),
                    compute_event_payload(event),
                    &sequence_id,
                    &event.id,
                );
                commands.push(cmd);
            }
            PolicyStatus::Deferred => {
                let cmd = KernelCommand::new(
                    &format!("cmd:{}", event.id),
                    "NoOpCommand",
                    BTreeMap::new(),
                    &sequence_id,
                    &event.id,
                );
                commands.push(cmd);
            }
        }
    }

    ExecutionPlan::new(&sequence_id, commands)
}

pub fn execute_plan(plan: ExecutionPlan) -> ExecutionPlanResult {
    let engine = StrataEngine::new();
    let mut results = Vec::new();
    let mut executed = 0usize;
    let mut failed = 0usize;

    for command in &plan.commands {
        let envelope = CommandEnvelope::new(command.clone());
        let result = engine.execute(&envelope);
        if result.success {
            executed += 1;
        } else {
            failed += 1;
        }
        results.push(ExecutionResult::new(
            &command.id,
            result.success,
            &result.result,
            result.error,
        ));
    }

    let ncmds = results.len();
    let explanation = format!("commands={}; executed={}; failed={}", ncmds, executed, failed);
    ExecutionPlanResult {
        plan,
        results,
        explanation,
    }
}

pub struct StrataEngine {
    kernel: Kernel,
}

impl StrataEngine {
    pub fn new() -> Self {
        StrataEngine {
            kernel: Kernel::new(),
        }
    }

    pub fn with_kernel(kernel: Kernel) -> Self {
        StrataEngine { kernel }
    }

    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    pub fn execute(&self, envelope: &CommandEnvelope) -> CommandResultV1 {
        let command = &envelope.command;
        match command.command_type.as_str() {
            "NoOpCommand" => CommandResultV1::success(&command.id, "NoOp executed"),
            "QueryGraph" => {
                let state = self.kernel.state();
                let result = format!("nodes={}; edges={}", state.node_count(), state.edge_count());
                CommandResultV1::success(&command.id, &result)
            }
            "QueryOntology" => {
                CommandResultV1::success(&command.id, "ontology query executed")
            }
            "QuerySemantic" => {
                CommandResultV1::success(&command.id, "semantic query executed")
            }
            "DescribeNode" => {
                let node_id = command.payload.get("node_id")
                    .and_then(|id| id.parse::<u64>().ok());
                match node_id.and_then(|id| self.kernel.state().get_node(id)) {
                    Some(node) => {
                        let result = format!("node {} type={}", node.id, node.node_type);
                        CommandResultV1::success(&command.id, &result)
                    }
                    None => CommandResultV1::failure(&command.id, "node not found"),
                }
            }
            "DescribeGraph" => {
                let state = self.kernel.state();
                let result = format!("nodes={}; edges={}", state.node_count(), state.edge_count());
                CommandResultV1::success(&command.id, &result)
            }
            "SelectNode" => {
                let node_id = command.payload.get("node_id")
                    .and_then(|id| id.parse::<u64>().ok());
                match node_id.and_then(|id| self.kernel.state().get_node(id)) {
                    Some(node) => {
                        CommandResultV1::success(&command.id, &format!("selected node {}", node.id))
                    }
                    None => CommandResultV1::failure(&command.id, "node not found"),
                }
            }
            "SelectEdge" => {
                let edge_id = command.payload.get("edge_id")
                    .and_then(|id| id.parse::<u64>().ok());
                match edge_id.and_then(|id| self.kernel.state().get_edge(id)) {
                    Some(edge) => {
                        CommandResultV1::success(&command.id, &format!("selected edge {}", edge.id))
                    }
                    None => CommandResultV1::failure(&command.id, "edge not found"),
                }
            }
            "FilterNodes" => {
                let state = self.kernel.state();
                let key = command.payload.get("filter_key").cloned().unwrap_or_default();
                let value = command.payload.get("filter_value").cloned().unwrap_or_default();
                let matching: Vec<&Node> = state.nodes.values()
                    .filter(|n| n.node_type == value || n.properties.get(&key) == Some(&value))
                    .collect();
                CommandResultV1::success(&command.id, &format!("filtered {} nodes", matching.len()))
            }
            "FilterEdges" => {
                let state = self.kernel.state();
                let key = command.payload.get("filter_key").cloned().unwrap_or_default();
                let value = command.payload.get("filter_value").cloned().unwrap_or_default();
                let matching: Vec<&Edge> = state.edges.values()
                    .filter(|e| e.edge_type == value || e.properties.get(&key) == Some(&value))
                    .collect();
                CommandResultV1::success(&command.id, &format!("filtered {} edges", matching.len()))
            }
            _ => CommandResultV1::failure(&command.id, &format!("unknown command type: {}", command.command_type)),
        }
    }
}

impl Default for StrataEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::semantic_interpreter::engine::interpret;
    use crate::cognition::event_translator::engine::translate;
    use crate::cognition::policy::engine::evaluate;
    use crate::cognition::policy::types::PolicyRule;
    use crate::cognition::semantic_interpreter::types::{SemanticIntent, IntentType};
    use crate::cognition::event_translator::types::ProposedEvent;
    use crate::kernel::Event;

    fn make_sequence(events: Vec<(&str, &str, BTreeMap<String, String>)>) -> ProposedEventSequence {
        let intent = SemanticIntent::new("test", IntentType::QueryGraph, BTreeMap::new(), BTreeMap::new());
        let proposed: Vec<ProposedEvent> = events.into_iter().enumerate().map(|(_, (id, event_type, payload))| {
            ProposedEvent::new(id, event_type, payload, &intent.id)
        }).collect();
        ProposedEventSequence::new(intent, None, proposed)
    }

    fn make_policy(seq: &ProposedEventSequence, statuses: Vec<PolicyStatus>) -> PolicyEvaluationResult {
        let decisions = seq.events.iter().zip(statuses.iter()).map(|(evt, status)| {
            crate::cognition::policy::types::PolicyDecision::new(&evt.id, status.clone(), "test")
        }).collect();
        PolicyEvaluationResult::new(seq.clone(), decisions)
    }

    #[test]
    fn test_build_plan_approved_only() {
        let mut payload = BTreeMap::new();
        payload.insert("node_id".to_string(), "42".to_string());
        let seq = make_sequence(vec![("evt_0", "GraphQueryRequested", payload)]);
        let policy = make_policy(&seq, vec![PolicyStatus::Approved]);
        let plan = build_plan(seq, policy);
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(plan.commands[0].command_type, "QueryGraph");
        assert_eq!(plan.commands[0].payload.get("node_id").unwrap(), "42");
    }

    #[test]
    fn test_build_plan_rejected_excluded() {
        let seq = make_sequence(vec![
            ("evt_0", "GraphQueryRequested", BTreeMap::new()),
            ("evt_1", "NodeSelectionEvent", BTreeMap::new()),
        ]);
        let policy = make_policy(&seq, vec![PolicyStatus::Approved, PolicyStatus::Rejected]);
        let plan = build_plan(seq, policy);
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(plan.commands[0].command_type, "QueryGraph");
    }

    #[test]
    fn test_build_plan_deferred_becomes_noop() {
        let seq = make_sequence(vec![("evt_0", "GraphQueryRequested", BTreeMap::new())]);
        let policy = make_policy(&seq, vec![PolicyStatus::Deferred]);
        let plan = build_plan(seq, policy);
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(plan.commands[0].command_type, "NoOpCommand");
        assert!(plan.commands[0].payload.is_empty());
    }

    #[test]
    fn test_build_plan_modified_included() {
        let seq = make_sequence(vec![("evt_0", "GraphQueryRequested", BTreeMap::new())]);
        let policy = make_policy(&seq, vec![PolicyStatus::Modified]);
        let plan = build_plan(seq, policy);
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(plan.commands[0].command_type, "QueryGraph");
    }

    #[test]
    fn test_build_plan_all_event_types_map_correctly() {
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
        let seq = make_sequence(
            event_types.iter().map(|et| (*et, *et, BTreeMap::new())).collect()
        );
        let policy = make_policy(&seq, vec![PolicyStatus::Approved; 10]);
        let plan = build_plan(seq, policy);
        for (cmd, expected_type) in plan.commands.iter().zip(expected.iter()) {
            assert_eq!(cmd.command_type, *expected_type, "mismatch for {}", cmd.event_id);
        }
    }

    #[test]
    fn test_build_plan_order_preserved() {
        let seq = make_sequence(vec![
            ("evt_a", "GraphQueryRequested", BTreeMap::new()),
            ("evt_b", "NodeSelectionEvent", BTreeMap::new()),
            ("evt_c", "NoOp", BTreeMap::new()),
        ]);
        let policy = make_policy(&seq, vec![PolicyStatus::Approved; 3]);
        let plan = build_plan(seq, policy);
        assert_eq!(plan.commands[0].event_id, "evt_a");
        assert_eq!(plan.commands[1].event_id, "evt_b");
        assert_eq!(plan.commands[2].event_id, "evt_c");
    }

    #[test]
    fn test_build_plan_deterministic() {
        let seq = make_sequence(vec![
            ("evt_0", "GraphQueryRequested", BTreeMap::new()),
            ("evt_1", "NodeSelectionEvent", BTreeMap::new()),
        ]);
        let policy = make_policy(&seq, vec![PolicyStatus::Approved, PolicyStatus::Deferred]);
        let a = build_plan(seq.clone(), policy.clone());
        let b = build_plan(seq, policy);
        assert_eq!(a.commands.len(), b.commands.len());
        for (ca, cb) in a.commands.iter().zip(b.commands.iter()) {
            assert_eq!(ca.command_type, cb.command_type);
            assert_eq!(ca.event_id, cb.event_id);
        }
    }

    #[test]
    fn test_identical_input_identical_plan() {
        let seq = make_sequence(vec![
            ("evt_0", "GraphQueryRequested", BTreeMap::new()),
            ("evt_1", "NodeFilterEvent", BTreeMap::new()),
        ]);
        let policy = make_policy(&seq, vec![PolicyStatus::Approved, PolicyStatus::Approved]);
        let plan_a = build_plan(seq.clone(), policy.clone());
        let plan_b = build_plan(seq, policy);
        assert_eq!(plan_a, plan_b);
    }

    #[test]
    fn test_execute_plan_empty() {
        let plan = ExecutionPlan::new("empty", vec![]);
        let result = execute_plan(plan);
        assert_eq!(result.results.len(), 0);
        assert!(result.explanation.contains("commands=0"));
    }

    #[test]
    fn test_execute_plan_noop() {
        let cmd = KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0");
        let plan = ExecutionPlan::new("seq", vec![cmd]);
        let result = execute_plan(plan);
        assert_eq!(result.results.len(), 1);
        assert!(result.results[0].success);
        assert_eq!(result.results[0].kernel_result, "NoOp executed");
    }

    #[test]
    fn test_execute_plan_deterministic() {
        let cmd = KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0");
        let plan = ExecutionPlan::new("seq", vec![cmd]);
        let a = execute_plan(plan.clone());
        let b = execute_plan(plan);
        assert_eq!(a.results.len(), b.results.len());
        for (ra, rb) in a.results.iter().zip(b.results.iter()) {
            assert_eq!(ra.success, rb.success);
            assert_eq!(ra.kernel_result, rb.kernel_result);
        }
    }

    #[test]
    fn test_strata_engine_execute_noop() {
        let cmd = KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0");
        let envelope = CommandEnvelope::new(cmd);
        let engine = StrataEngine::new();
        let result = engine.execute(&envelope);
        assert!(result.success);
        assert_eq!(result.result, "NoOp executed");
    }

    #[test]
    fn test_strata_engine_execute_query_graph() {
        let cmd = KernelCommand::new("cmd:0", "QueryGraph", BTreeMap::new(), "trace", "evt_0");
        let envelope = CommandEnvelope::new(cmd);
        let engine = StrataEngine::new();
        let result = engine.execute(&envelope);
        assert!(result.success);
        assert!(result.result.contains("nodes=0"));
    }

    #[test]
    fn test_strata_engine_execute_unknown() {
        let cmd = KernelCommand::new("cmd:0", "InvalidType", BTreeMap::new(), "trace", "evt_0");
        let envelope = CommandEnvelope::new(cmd);
        let engine = StrataEngine::new();
        let result = engine.execute(&envelope);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("unknown"));
    }

    #[test]
    fn test_strata_engine_with_kernel() {
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "test".into() }).unwrap();
        let engine = StrataEngine::with_kernel(kernel);
        let mut payload = BTreeMap::new();
        payload.insert("node_id".to_string(), "1".to_string());
        let cmd = KernelCommand::new("cmd:0", "DescribeNode", payload, "trace", "evt_0");
        let envelope = CommandEnvelope::new(cmd);
        let result = engine.execute(&envelope);
        assert!(result.success);
        assert!(result.result.contains("node 1"));
    }

    #[test]
    fn test_strata_engine_node_not_found() {
        let mut payload = BTreeMap::new();
        payload.insert("node_id".to_string(), "999".to_string());
        let cmd = KernelCommand::new("cmd:0", "DescribeNode", payload, "trace", "evt_0");
        let envelope = CommandEnvelope::new(cmd);
        let engine = StrataEngine::new();
        let result = engine.execute(&envelope);
        assert!(!result.success);
    }

    #[test]
    fn test_strata_engine_filter_nodes() {
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "Person".into() }).unwrap();
        kernel.propose_and_commit(Event::CreateNode { id: 2, node_type: "Place".into() }).unwrap();
        let engine = StrataEngine::with_kernel(kernel);
        let mut payload = BTreeMap::new();
        payload.insert("filter_key".to_string(), "type".to_string());
        payload.insert("filter_value".to_string(), "Person".to_string());
        let cmd = KernelCommand::new("cmd:0", "FilterNodes", payload, "trace", "evt_0");
        let envelope = CommandEnvelope::new(cmd);
        let result = engine.execute(&envelope);
        assert!(result.success);
    }

    #[test]
    fn test_execute_plan_result_explanation() {
        let cmds = vec![
            KernelCommand::new("cmd:0", "NoOpCommand", BTreeMap::new(), "trace", "evt_0"),
            KernelCommand::new("cmd:1", "NoOpCommand", BTreeMap::new(), "trace", "evt_1"),
        ];
        let plan = ExecutionPlan::new("seq", cmds);
        let result = execute_plan(plan);
        assert!(result.explanation.contains("commands=2"));
        assert!(result.explanation.contains("executed=2"));
        assert!(result.explanation.contains("failed=0"));
    }

    #[test]
    fn test_full_end_to_end() {
        let resp = interpret("find nodes");
        let seq = translate(resp);
        let rule = PolicyRule::new("R001");
        let policy = evaluate(seq.clone(), vec![rule]);
        let plan = build_plan(seq, policy.clone());
        assert_eq!(plan.commands.len(), policy.decisions.iter().filter(|d| d.status != PolicyStatus::Rejected).count());
        let result = execute_plan(plan);
        assert_eq!(result.results.len(), result.plan.commands.len());
    }

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

    #[test]
    fn test_100_run_stability() {
        let resp = interpret("describe node 42");
        let seq = translate(resp);
        let rule = PolicyRule::new("R001");
        let policy = evaluate(seq.clone(), vec![rule]);
        let plan = build_plan(seq, policy.clone());
        let first = execute_plan(plan);
        for _ in 0..100 {
            let seq2 = translate(interpret("describe node 42"));
            let policy2 = evaluate(seq2.clone(), vec![PolicyRule::new("R001")]);
            let plan2 = build_plan(seq2, policy2);
            let next = execute_plan(plan2);
            assert_eq!(first.results.len(), next.results.len());
            for (ra, rb) in first.results.iter().zip(next.results.iter()) {
                assert_eq!(ra.success, rb.success);
                assert_eq!(ra.kernel_result, rb.kernel_result);
            }
        }
    }

    #[test]
    fn test_hundred_run_stability_same_input() {
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
}
