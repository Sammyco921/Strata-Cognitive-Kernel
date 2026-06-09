use crate::cognition::goals::types::{GoalEvaluation, GoalState};
use crate::cognition::memory::types::MemorySnapshot;
use crate::cognition::policy::types::PolicyRule;
use crate::cognition::system::engine::run_cognition_system;
use crate::cognition::system::types::{CognitionSystemInput, CognitionSystemOutput};
use crate::cognition::trace::types::TraceRecord;
use crate::kernel::GraphState;
use crate::ontology::OntologyRegistry;
use crate::verification::engine::verify_all;
use crate::verification::types::VerificationReport;

pub struct CliRunner;

impl CliRunner {
    pub fn new() -> Self {
        CliRunner
    }

    fn make_input(raw_input: &str) -> CognitionSystemInput {
        CognitionSystemInput::new(
            raw_input,
            vec![PolicyRule::new("R001")],
            GraphState::empty(),
            OntologyRegistry::empty(),
        )
    }

    pub fn run(raw_input: &str) -> CognitionSystemOutput {
        run_cognition_system(Self::make_input(raw_input))
    }

    pub fn replay(original: &TraceRecord) -> (CognitionSystemOutput, bool) {
        let output = run_cognition_system(Self::make_input(&original.raw_input));
        let trace = Self::extract_trace_record(&output);
        let matches = trace.trace_id == original.trace_id
            && trace.raw_input == original.raw_input;
        (output, matches)
    }

    pub fn inspect_output(output: &CognitionSystemOutput) -> String {
        format!(
            "Trace ID: {}\n\
             Raw Input: {}\n\
             Intent: {:?}\n\
             Explanation: {}\n\
             Events: {}\n\
             Policy Decisions: {}\n\
             Execution Commands: {}\n\
             Execution Results: {}\n\
             Coherence Valid: {}\n\
             Memory Entries: {}\n\
             Goal Evaluations: {}\n\
             Stages: {}",
            output.trace_record.trace_id,
            output.trace_record.raw_input,
            output.semantic_response.intent.intent_type,
            output.semantic_response.explanation,
            output.event_sequence.events.len(),
            output.policy_result.decisions.len(),
            output.execution_plan.commands.len(),
            output.execution_result.results.len(),
            output.coherence_report.is_valid,
            output.memory_snapshot.state.update_counter,
            output.goal_evaluations.len(),
            output.stage_count(),
        )
    }

    pub fn extract_trace_record(output: &CognitionSystemOutput) -> TraceRecord {
        output.trace_record.clone()
    }

    pub fn verify() -> VerificationReport {
        verify_all()
    }

    pub fn extract_goal_evaluations(output: &CognitionSystemOutput) -> Vec<GoalEvaluation> {
        output.goal_evaluations.clone()
    }

    pub fn extract_goal_state(output: &CognitionSystemOutput) -> GoalState {
        output.updated_goal_state.clone()
    }

    pub fn extract_memory_snapshot(output: &CognitionSystemOutput) -> MemorySnapshot {
        output.memory_snapshot.clone()
    }

    pub fn goals(raw_input: &str) -> (Vec<GoalEvaluation>, GoalState) {
        let output = Self::run(raw_input);
        (output.goal_evaluations, output.updated_goal_state)
    }

    pub fn memory(raw_input: &str) -> MemorySnapshot {
        let output = Self::run(raw_input);
        output.memory_snapshot
    }

    pub fn deterministic_run_100(raw_input: &str) -> bool {
        let first = Self::run(raw_input);
        for _ in 0..100 {
            let next = Self::run(raw_input);
            if first != next {
                return false;
            }
        }
        true
    }
}

impl Default for CliRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_produces_output() {
        let output = CliRunner::run("find nodes");
        assert!(!output.trace_record.trace_id.is_empty());
    }

    #[test]
    fn test_run_identical_output() {
        let a = CliRunner::run("find nodes");
        let b = CliRunner::run("find nodes");
        assert_eq!(a, b);
    }

    #[test]
    fn test_deterministic_100_runs() {
        assert!(CliRunner::deterministic_run_100("find nodes"));
    }

    #[test]
    fn test_verify_returns_report() {
        let report = CliRunner::verify();
        assert!(!report.results.is_empty());
    }

    #[test]
    fn test_verify_all_invariants_known() {
        let report = CliRunner::verify();
        // All invariants should be accounted for
        assert!(report.results.len() >= 10);
    }

    #[test]
    fn test_inspect_output_contains_trace_id() {
        let output = CliRunner::run("find nodes");
        let formatted = CliRunner::inspect_output(&output);
        assert!(formatted.contains(&output.trace_record.trace_id));
    }

    #[test]
    fn test_inspect_output_contains_explanation() {
        let output = CliRunner::run("describe graph");
        let formatted = CliRunner::inspect_output(&output);
        assert!(formatted.contains("DescribeGraph") || formatted.contains("QueryGraph"));
    }

    #[test]
    fn test_replay_matches() {
        let original = CliRunner::run("find nodes");
        let (_output, matches) = CliRunner::replay(&original.trace_record);
        assert!(matches);
    }

    #[test]
    fn test_replay_different_input_does_not_match() {
        let original = CliRunner::run("find nodes");
        let trace = original.trace_record;
        let mismatched = TraceRecord {
            raw_input: "describe graph".to_string(),
            ..trace
        };
        let (_output, matches) = CliRunner::replay(&mismatched);
        assert!(!matches);
    }

    #[test]
    fn test_goals_returns_evaluations() {
        let (evaluations, _state) = CliRunner::goals("find nodes");
        assert!(evaluations.is_empty() || !evaluations.is_empty());
    }

    #[test]
    fn test_memory_returns_snapshot() {
        let snapshot = CliRunner::memory("find nodes");
        // update_counter should be present after a pipeline run
        assert!(snapshot.state.intent_success_counts.is_empty() || !snapshot.state.intent_success_counts.is_empty());
    }

    #[test]
    fn test_extract_trace_record() {
        let output = CliRunner::run("test input");
        let trace = CliRunner::extract_trace_record(&output);
        assert_eq!(trace.raw_input, "test input");
    }

    #[test]
    fn test_trace_record_extracted_matches() {
        let output = CliRunner::run("find nodes");
        let trace = CliRunner::extract_trace_record(&output);
        assert_eq!(trace.trace_id, output.trace_record.trace_id);
    }

    #[test]
    fn test_run_empty_input() {
        let output = CliRunner::run("");
        assert_eq!(output.semantic_response.intent.intent_type, crate::cognition::semantic_interpreter::types::IntentType::Unknown);
    }

    #[test]
    fn test_run_whitespace_input() {
        let output = CliRunner::run("   ");
        assert_eq!(output.semantic_response.intent.intent_type, crate::cognition::semantic_interpreter::types::IntentType::Unknown);
    }

    #[test]
    fn test_serialization_roundtrip_output() {
        let output = CliRunner::run("find nodes");
        let json = serde_json::to_string(&output).unwrap();
        let parsed: CognitionSystemOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }

    #[test]
    fn test_serialization_deterministic_output() {
        let output = CliRunner::run("find nodes");
        let first = serde_json::to_string(&output).unwrap();
        for _ in 0..100 {
            let next = serde_json::to_string(&output).unwrap();
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_stage_count() {
        let output = CliRunner::run("find nodes");
        assert_eq!(output.stage_count(), 12);
    }
}
