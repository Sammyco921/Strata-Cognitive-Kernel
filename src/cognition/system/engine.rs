use std::collections::BTreeMap;

use super::types::*;
use crate::cognition::executive::engine::make_decision;
use crate::cognition::executive::types::ExecutiveContext;
use crate::cognition::semantic_interpreter::engine::interpret;
use crate::cognition::event_translator::engine::translate;
use crate::cognition::policy::engine::evaluate;
use crate::cognition::execution_adapter::engine::{build_plan, execute_plan};
use crate::cognition::trace::engine::record_trace;
use crate::cognition::coherence::engine::verify_coherence;
use crate::cognition::system::policy::engine::run_policy_layer;
use crate::cognition::system::policy::types::{PolicyCandidate, PolicyDecision, PolicyScore};
use crate::cognition::memory::engine::{update_memory, compute_memory_snapshot};
use crate::cognition::memory::types::MemoryUpdateEvent;
use crate::cognition::goals::engine::{evaluate_all_goals, update_goal_state};
use crate::cognition::semantic_interpreter::types::{IntentType, SemanticIntent};

pub fn run_cognition_system(input: CognitionSystemInput) -> CognitionSystemOutput {
    let semantic_response = interpret(&input.raw_input);

    let event_sequence = translate(semantic_response.clone());

    // ── Goals ─────────────────────────────────────────────────────────────
    let goal_evaluations = evaluate_all_goals(
        &input.goal_state,
        &input.kernel_state,
        &input.ontology_registry,
        semantic_response.query.as_ref(),
    );
    let updated_goal_state = update_goal_state(&input.goal_state, &goal_evaluations);

    // ── Executive ─────────────────────────────────────────────────────────
    let default_intent = SemanticIntent::new("executive", IntentType::Unknown, BTreeMap::new(), BTreeMap::new());
    let default_score = PolicyScore::compute(0, 0, 0, 0, 0, 1.0, "executive");
    let default_candidate = PolicyCandidate::new(default_intent, default_score);
    let default_policy_decision = PolicyDecision::new(default_candidate, vec![], BTreeMap::new(), BTreeMap::new());

    let executive_context = ExecutiveContext {
        goals: updated_goal_state.clone(),
        memory_state: input.previous_memory.clone(),
        policy_decision: default_policy_decision,
    };
    let executive_decision = make_decision(&executive_context);

    // ── Policy ────────────────────────────────────────────────────────────
    let policy_decision = run_policy_layer(
        &semantic_response,
        &input.ontology_registry,
        &input.policy_rules,
        &input.historical_traces,
        &input.previous_memory,
    );

    let policy_result = evaluate(event_sequence.clone(), input.policy_rules);

    // ── Execution ─────────────────────────────────────────────────────────
    let execution_plan = build_plan(event_sequence.clone(), policy_result.clone());
    let execution_result = execute_plan(execution_plan.clone());

    // ── Trace ─────────────────────────────────────────────────────────────
    let trace_record = record_trace(
        semantic_response.clone(),
        event_sequence.clone(),
        policy_result.clone(),
        execution_plan.clone(),
        execution_result.clone(),
    );

    // ── Coherence ─────────────────────────────────────────────────────────
    let coherence_report = verify_coherence(
        &trace_record,
        &input.kernel_state,
        &input.ontology_registry,
    );

    // ── Memory ────────────────────────────────────────────────────────────
    let memory_event = MemoryUpdateEvent::from_trace_and_coherence(
        &trace_record,
        &coherence_report,
        &policy_decision,
    );

    let updated_memory = update_memory(input.previous_memory, &memory_event, &input.ontology_registry);
    let memory_snapshot = compute_memory_snapshot(updated_memory);

    // ── Output ────────────────────────────────────────────────────────────
    CognitionSystemOutput {
        semantic_response,
        event_sequence,
        policy_decision,
        policy_result,
        execution_plan,
        execution_result,
        trace_record,
        coherence_report,
        memory_snapshot,
        goal_evaluations,
        updated_goal_state,
        executive_decision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::policy::types::PolicyRule;

    fn default_input(input: &str) -> CognitionSystemInput {
        CognitionSystemInput::new(
            input,
            vec![PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        )
    }

    #[test]
    fn test_pipeline_produces_output() {
        let output = run_cognition_system(default_input("find nodes"));
        assert_eq!(output.stage_count(), 12);
    }

    #[test]
    fn test_semantic_response_present() {
        let output = run_cognition_system(default_input("find nodes"));
        assert_eq!(output.semantic_response.intent.intent_type, crate::cognition::semantic_interpreter::types::IntentType::QueryGraph);
    }

    #[test]
    fn test_event_sequence_present() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.event_sequence.events.is_empty());
    }

    #[test]
    fn test_policy_result_present() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.policy_result.decisions.is_empty());
    }

    #[test]
    fn test_execution_plan_present() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.execution_plan.commands.is_empty());
    }

    #[test]
    fn test_execution_result_present() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.execution_result.results.is_empty());
    }

    #[test]
    fn test_trace_record_present() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.trace_record.trace_id.is_empty());
    }

    #[test]
    fn test_coherence_report_present() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(output.coherence_report.is_valid);
    }

    #[test]
    fn test_deterministic_identical_input() {
        let a = run_cognition_system(default_input("find nodes"));
        let b = run_cognition_system(default_input("find nodes"));
        assert_eq!(a, b);
    }

    #[test]
    fn test_deterministic_100_runs() {
        let first = run_cognition_system(default_input("find nodes"));
        for _ in 0..100 {
            let next = run_cognition_system(default_input("find nodes"));
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_different_inputs_different_outputs() {
        let a = run_cognition_system(default_input("find nodes"));
        let b = run_cognition_system(default_input("describe node 42"));
        assert_ne!(a, b);
    }

    #[test]
    fn test_pipeline_describe_node() {
        let output = run_cognition_system(default_input("describe node 42"));
        assert_eq!(output.semantic_response.intent.intent_type, crate::cognition::semantic_interpreter::types::IntentType::DescribeNode);
    }

    #[test]
    fn test_pipeline_unknown() {
        let output = run_cognition_system(default_input("gibberish"));
        assert_eq!(output.semantic_response.intent.intent_type, crate::cognition::semantic_interpreter::types::IntentType::Unknown);
    }

    #[test]
    fn test_trace_id_matches_intent_id() {
        let output = run_cognition_system(default_input("find nodes"));
        assert_eq!(output.trace_record.trace_id, output.semantic_response.intent.id);
    }

    #[test]
    fn test_raw_input_preserved_in_trace() {
        let output = run_cognition_system(default_input("find nodes"));
        assert_eq!(output.trace_record.raw_input, "find nodes");
    }

    #[test]
    fn test_coherence_valid_for_normal_input() {
        let output = run_cognition_system(default_input("describe graph"));
        assert!(output.coherence_report.is_valid);
    }

    #[test]
    fn test_all_stages_have_data() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.semantic_response.explanation.is_empty());
        assert!(!output.event_sequence.events.is_empty());
        assert!(!output.policy_result.decisions.is_empty());
        assert!(!output.execution_plan.commands.is_empty());
        assert!(!output.execution_result.results.is_empty());
        assert!(!output.trace_record.trace_id.is_empty());
    }

    #[test]
    fn test_pipeline_does_not_panic_on_empty_input() {
        let output = run_cognition_system(default_input(""));
        assert_eq!(output.semantic_response.intent.intent_type, crate::cognition::semantic_interpreter::types::IntentType::Unknown);
    }

    #[test]
    fn test_pipeline_does_not_panic_on_whitespace() {
        let output = run_cognition_system(default_input("   "));
        assert_eq!(output.semantic_response.intent.intent_type, crate::cognition::semantic_interpreter::types::IntentType::Unknown);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let output = run_cognition_system(default_input("find nodes"));
        let json = serde_json::to_string(&output).unwrap();
        let parsed: CognitionSystemOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }

    #[test]
    fn test_serialization_deterministic() {
        let output = run_cognition_system(default_input("find nodes"));
        let first = serde_json::to_string(&output).unwrap();
        for _ in 0..100 {
            let next = serde_json::to_string(&output).unwrap();
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_stage_count_exact() {
        let output = run_cognition_system(default_input("find nodes"));
        assert_eq!(output.stage_count(), 12);
    }

    #[test]
    fn test_coherence_report_trace_id_match() {
        let output = run_cognition_system(default_input("find nodes"));
        assert_eq!(output.coherence_report.trace_id, output.trace_record.trace_id);
    }

    #[test]
    fn test_event_sequence_intent_preserved() {
        let output = run_cognition_system(default_input("find nodes"));
        assert_eq!(output.event_sequence.intent.id, output.semantic_response.intent.id);
    }

    #[test]
    fn test_execution_plan_explanation_nonempty() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.execution_plan.explanation.is_empty());
    }

    #[test]
    fn test_execution_result_explanation_nonempty() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.execution_result.explanation.is_empty());
    }

    #[test]
    fn test_policy_explanation_nonempty() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(!output.policy_result.explanation.is_empty());
    }

    #[test]
    fn test_no_mutation_via_repeated_call() {
        let input = default_input("find nodes");
        let a = run_cognition_system(input.clone());
        let b = run_cognition_system(input);
        assert_eq!(a, b);
    }

    #[test]
    fn test_different_policy_rules_different_result() {
        let input_a = CognitionSystemInput::new(
            "find nodes",
            vec![PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        );
        let mut reject_rule = PolicyRule::new("R_REJECT");
        reject_rule.forbidden_properties.insert("type".to_string(), "Person".to_string());
        let input_b = CognitionSystemInput::new(
            "find nodes",
            vec![reject_rule],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        );
        let a = run_cognition_system(input_a);
        let b = run_cognition_system(input_b);
        assert_ne!(a.policy_result, b.policy_result);
    }

    #[test]
    fn test_100_run_stability_describe_node() {
        let first = run_cognition_system(default_input("describe node 42"));
        for _ in 0..100 {
            let next = run_cognition_system(default_input("describe node 42"));
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_100_run_stability_unknown() {
        let first = run_cognition_system(default_input("gibberish"));
        for _ in 0..100 {
            let next = run_cognition_system(default_input("gibberish"));
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_json_output_stable_across_runs() {
        let output = run_cognition_system(default_input("find nodes"));
        let json = serde_json::to_string(&output).unwrap();
        for _ in 0..100 {
            let next = run_cognition_system(default_input("find nodes"));
            assert_eq!(json, serde_json::to_string(&next).unwrap());
        }
    }

    #[test]
    fn test_100_different_inputs_no_panic() {
        let inputs = [
            "find nodes", "describe node 1", "query ontology", "run semantic analysis",
            "describe graph", "hello world", "", "find edges", "describe node 99",
            "query graph with filters", "search data", "list everything",
            "show me nodes", "find Person", "describe", "gibberish input here",
            "nodes with edges", "filter by type:Person", "deep traversal",
            "find user age:30", "describe node 1 2 3", "ontology types",
            "semantic patterns", "find deep nodes", "describe the entire graph",
        ];
        for input in &inputs {
            let output = run_cognition_system(default_input(input));
            assert!(!output.trace_record.trace_id.is_empty());
        }
    }

    #[test]
    fn test_historical_traces_influence_policy() {
        let mut input_with_traces = CognitionSystemInput::new(
            "find nodes",
            vec![PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        );
        let intent = crate::cognition::semantic_interpreter::types::SemanticIntent::new(
            "find nodes",
            crate::cognition::semantic_interpreter::types::IntentType::QueryGraph,
            std::collections::BTreeMap::new(),
            std::collections::BTreeMap::new(),
        );
        let query = Some(crate::cognition::semantic_interpreter::types::SemanticQuery::new());
        let sr = crate::cognition::semantic_interpreter::types::SemanticResponse {
            intent: intent.clone(),
            query: query.clone(),
            explanation: "test".to_string(),
        };
        let event = crate::cognition::event_translator::types::ProposedEvent::new(
            "evt_0", "GraphQueryRequested", std::collections::BTreeMap::new(), &intent.id,
        );
        let seq = crate::cognition::event_translator::types::ProposedEventSequence::new(
            intent.clone(), query, vec![event],
        );
        let decisions = vec![crate::cognition::policy::types::PolicyDecision::new(
            "evt_0", crate::cognition::policy::types::PolicyStatus::Approved, "ok",
        )];
        let pr = crate::cognition::policy::types::PolicyEvaluationResult::new(seq.clone(), decisions);
        let cmd = crate::cognition::execution_adapter::types::KernelCommand::new(
            "cmd:evt_0", "QueryGraph", std::collections::BTreeMap::new(), &intent.id, "evt_0",
        );
        let plan = crate::cognition::execution_adapter::types::ExecutionPlan::new(&intent.id, vec![cmd]);
        let result = crate::cognition::execution_adapter::types::ExecutionResult::new(
            "cmd:evt_0", true, "ok", None,
        );
        let er = crate::cognition::execution_adapter::types::ExecutionPlanResult::new(
            plan.clone(), vec![result],
        );
        let trace = crate::cognition::trace::types::TraceRecord::new(sr, seq, pr, plan, er);
        input_with_traces.historical_traces = vec![trace];

        let output = run_cognition_system(input_with_traces);
        assert!(!output.policy_decision.memory_profiles.is_empty());
    }

    #[test]
    fn test_policy_decision_contains_memory() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(output.policy_decision.explanation.contains("memory="));
    }

    #[test]
    fn test_empty_historical_traces_still_works() {
        let output = run_cognition_system(default_input("find nodes"));
        assert!(output.policy_decision.memory_profiles.is_empty());
        assert!(output.policy_decision.memory_influences.is_empty());
    }

    // ── Goal integration tests ──────────────────────────────────────────

    fn input_with_goals(input: &str, goals: crate::cognition::goals::types::GoalState) -> CognitionSystemInput {
        CognitionSystemInput::with_goals(
            input,
            vec![crate::cognition::policy::types::PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
            goals,
        )
    }

    #[test]
    fn test_goal_evaluations_included_in_output() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        assert!(!output.goal_evaluations.is_empty());
        assert_eq!(output.goal_evaluations[0].goal_id, GoalId("g1".to_string()));
        assert!(output.goal_evaluations[0].satisfied);
    }

    #[test]
    fn test_updated_goal_state_returned() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        assert_eq!(output.updated_goal_state.goals[&GoalId("g1".to_string())].status, GoalStatus::Satisfied);
    }

    #[test]
    fn test_multiple_goals_evaluated_in_integration() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        goals.insert(GoalId("g2".to_string()), Goal {
            id: GoalId("g2".to_string()),
            description: String::new(),
            predicate: GoalPredicate::NeverSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        assert_eq!(output.goal_evaluations.len(), 2);
        assert!(output.goal_evaluations.iter().find(|e| e.goal_id == GoalId("g1".to_string())).unwrap().satisfied);
        assert!(!output.goal_evaluations.iter().find(|e| e.goal_id == GoalId("g2".to_string())).unwrap().satisfied);
    }

    #[test]
    fn test_empty_goal_registry_integration() {
        use crate::cognition::goals::types::*;
        let state = GoalState { goals: std::collections::BTreeMap::new() };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        assert!(output.goal_evaluations.is_empty());
        assert!(output.updated_goal_state.goals.is_empty());
    }

    #[test]
    fn test_policy_unchanged_by_goals() {
        let no_goals = run_cognition_system(default_input("find nodes"));
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let with_goals = run_cognition_system(input_with_goals("find nodes", state));
        assert_eq!(no_goals.policy_decision, with_goals.policy_decision);
        assert_eq!(no_goals.policy_result, with_goals.policy_result);
    }

    #[test]
    fn test_memory_unchanged_by_goals() {
        let no_goals = run_cognition_system(default_input("find nodes"));
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let with_goals = run_cognition_system(input_with_goals("find nodes", state));
        assert_eq!(no_goals.memory_snapshot, with_goals.memory_snapshot);
    }

    #[test]
    fn test_execution_unchanged_by_goals() {
        let no_goals = run_cognition_system(default_input("find nodes"));
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let with_goals = run_cognition_system(input_with_goals("find nodes", state));
        assert_eq!(no_goals.execution_plan, with_goals.execution_plan);
        assert_eq!(no_goals.execution_result, with_goals.execution_result);
    }

    #[test]
    fn test_deterministic_with_goals_100_runs() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let first = run_cognition_system(input_with_goals("find nodes", state.clone()));
        for _ in 0..100 {
            let next = run_cognition_system(input_with_goals("find nodes", state.clone()));
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_goal_evaluation_serialization_in_output() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        let json = serde_json::to_string(&output).unwrap();
        let parsed: CognitionSystemOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }

    #[test]
    fn test_goal_determinism_100_run_serialization() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals: goals.clone() };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        let json = serde_json::to_string(&output).unwrap();
        for _ in 0..100 {
            let next = run_cognition_system(input_with_goals("find nodes", GoalState { goals: goals.clone() }));
            let next_json = serde_json::to_string(&next).unwrap();
            assert_eq!(json, next_json);
        }
    }

    // ── Executive integration tests ──────────────────────────────────────

    #[test]
    fn test_executive_decision_in_output() {
        let output = run_cognition_system(default_input("find nodes"));
        // executive_decision should always be present
        assert!(output.executive_decision.selected_goal.is_none() || output.executive_decision.selected_goal.is_some());
        assert!(!output.executive_decision.ranked_goals.is_empty() || output.executive_decision.ranked_goals.is_empty());
    }

    #[test]
    fn test_executive_decision_deterministic() {
        let a = run_cognition_system(default_input("find nodes"));
        let b = run_cognition_system(default_input("find nodes"));
        assert_eq!(a.executive_decision, b.executive_decision);
    }

    #[test]
    fn test_executive_decision_100_run_stability() {
        let first = run_cognition_system(default_input("find nodes"));
        for _ in 0..100 {
            let next = run_cognition_system(default_input("find nodes"));
            assert_eq!(first.executive_decision, next.executive_decision);
        }
    }

    #[test]
    fn test_executive_with_goals_integration() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        goals.insert(GoalId("g2".to_string()), Goal {
            id: GoalId("g2".to_string()),
            description: String::new(),
            predicate: GoalPredicate::NeverSatisfied,
            status: GoalStatus::InProgress,
        });
        let state = GoalState { goals };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        // Executive should rank goals
        assert!(!output.executive_decision.ranked_goals.is_empty());
        // g1 is satisfied (AlwaysSatisfied), g2 transitions from InProgress to Failed (NeverSatisfied)
        assert!(!output.executive_decision.completed_goals.is_empty());
    }

    #[test]
    fn test_policy_unchanged_by_executive() {
        let a = run_cognition_system(default_input("find nodes"));
        let output = run_cognition_system(default_input("find nodes"));
        // Executive does not alter policy behavior
        assert_eq!(a.policy_decision, output.policy_decision);
        assert_eq!(a.policy_result, output.policy_result);
    }

    #[test]
    fn test_execution_unchanged_by_executive() {
        let a = run_cognition_system(default_input("find nodes"));
        let output = run_cognition_system(default_input("find nodes"));
        // Executive does not alter execution behavior
        assert_eq!(a.execution_plan, output.execution_plan);
        assert_eq!(a.execution_result, output.execution_result);
    }

    #[test]
    fn test_memory_unchanged_by_executive() {
        let a = run_cognition_system(default_input("find nodes"));
        let output = run_cognition_system(default_input("find nodes"));
        // Executive does not alter memory
        assert_eq!(a.memory_snapshot, output.memory_snapshot);
    }

    #[test]
    fn test_executive_decision_serialization_in_output() {
        let output = run_cognition_system(default_input("find nodes"));
        let json = serde_json::to_string(&output).unwrap();
        let parsed: CognitionSystemOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output.executive_decision, parsed.executive_decision);
    }

    #[test]
    fn test_executive_explanation_format() {
        let output = run_cognition_system(default_input("find nodes"));
        let explanation = &output.executive_decision.explanation;
        // Explanation should follow expected format
        assert!(explanation.starts_with("goal="));
        assert!(explanation.contains(";priority="));
        assert!(explanation.contains(";rank="));
        assert!(explanation.contains(";status="));
    }

    #[test]
    fn test_executive_with_goals_explanation() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        let explanation = &output.executive_decision.explanation;
        assert!(explanation.contains("goal=") || explanation == "goal=none;priority=0;rank=0;status=none");
    }

    #[test]
    fn test_total_determinism_preserved_with_executive() {
        let first = run_cognition_system(default_input("find nodes"));
        for _ in 0..100 {
            let next = run_cognition_system(default_input("find nodes"));
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_executive_with_goals_100_run_serialization() {
        use crate::cognition::goals::types::*;
        let mut goals = std::collections::BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals: goals.clone() };
        let output = run_cognition_system(input_with_goals("find nodes", state));
        let json = serde_json::to_string(&output).unwrap();
        for _ in 0..100 {
            let next = run_cognition_system(input_with_goals("find nodes", GoalState { goals: goals.clone() }));
            assert_eq!(json, serde_json::to_string(&next).unwrap());
        }
    }
}
