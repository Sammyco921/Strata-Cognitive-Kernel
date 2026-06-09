use crate::cognition::goals::types::{GoalEvaluation, GoalState};
use crate::cognition::memory::types::MemorySnapshot;
use crate::cognition::system::types::CognitionSystemOutput;
use crate::cognition::trace::types::TraceRecord;
use crate::verification::types::{InvariantStatus, VerificationReport};

pub fn format_output(output: &CognitionSystemOutput) -> String {
    format!(
        "{}\n---\n{}",
        format_trace(&output.trace_record),
        format_inspection(output),
    )
}

pub fn format_trace(trace: &TraceRecord) -> String {
    format!(
        "Trace ID: {}\n\
         Raw Input: {}\n\
         Intent Type: {:?}\n\
         Explanation: {}\n\
         Proposed Events ({}): {:?}\n\
         Policy Decisions ({}): {:?}\n\
         Execution Commands ({}): {:?}\n\
         Execution Results ({}): {:?}",
        trace.trace_id,
        trace.raw_input,
        trace.semantic_response.intent.intent_type,
        trace.semantic_response.explanation,
        trace.proposed_sequence.events.len(),
        trace.proposed_sequence.events.iter().map(|e| &e.event_type).collect::<Vec<_>>(),
        trace.policy_result.decisions.len(),
        trace.policy_result.decisions.iter().map(|d| &d.status).collect::<Vec<_>>(),
        trace.execution_plan.commands.len(),
        trace.execution_plan.commands.iter().map(|c| &c.command_type).collect::<Vec<_>>(),
        trace.execution_result.results.len(),
        trace.execution_result.results.iter().map(|r| r.success).collect::<Vec<_>>(),
    )
}

pub fn format_inspection(output: &CognitionSystemOutput) -> String {
    format!(
        "Coherence Report: valid={}\n\
         Memory Snapshot: {} intents tracked, {} commands tracked\n\
         Goal Evaluations ({}): {:?}\n\
         Updated Goal State: {} goals\n\
         Pipeline Stages: {}",
        output.coherence_report.is_valid,
        output.memory_snapshot.state.intent_success_counts.len(),
        output.memory_snapshot.state.command_success_counts.len(),
        output.goal_evaluations.len(),
        output.goal_evaluations.iter().map(|g| format!("{}={}", g.goal_id.0, g.satisfied)).collect::<Vec<_>>(),
        output.updated_goal_state.goals.len(),
        output.stage_count(),
    )
}

pub fn format_goals(evaluations: &[GoalEvaluation], state: &GoalState) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Goal Evaluations ({}):", evaluations.len()));
    for e in evaluations {
        lines.push(format!("  {}: satisfied={}", e.goal_id.0, e.satisfied));
    }
    lines.push(format!("Goal State ({}):", state.goals.len()));
    for (id, goal) in &state.goals {
        lines.push(format!("  {}: status={:?}, desc={}", id.0, goal.status, goal.description));
    }
    lines.join("\n")
}

pub fn format_memory(snapshot: &MemorySnapshot) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("Memory Snapshot:".to_string());
    lines.push(format!("  Intents tracked: {}", snapshot.state.intent_success_counts.len()));
    for (intent, count) in &snapshot.state.intent_success_counts {
        let failure = snapshot.state.intent_failure_counts.get(intent).copied().unwrap_or(0);
        lines.push(format!("  {}: {} successes, {} failures", intent.name(), count, failure));
    }
    lines.push(format!("  Commands tracked: {}", snapshot.state.command_success_counts.len()));
    for (cmd, count) in &snapshot.state.command_success_counts {
        let failure = snapshot.state.command_failure_counts.get(cmd).copied().unwrap_or(0);
        lines.push(format!("  {}: {} successes, {} failures", cmd, count, failure));
    }
    lines.push(format!("  Update count: {}", snapshot.state.update_counter));
    lines.join("\n")
}

pub fn format_verification(report: &VerificationReport) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Verification Report ({} checks):", report.results.len()));
    for r in &report.results {
        let status = match r.status {
            InvariantStatus::Passed => "PASS",
            InvariantStatus::Failed => "FAIL",
        };
        let msg = if r.reason.is_empty() { String::new() } else { format!(": {}", r.reason) };
        lines.push(format!("  [{}] {:?} {}{}", status, r.layer, r.invariant_id.0, msg));
    }
    lines.push(format!("Result: {} passed, {} failed", report.passed, report.failed));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_output_stable() {
        let output = crate::cli::runner::CliRunner::run("find nodes");
        let formatted = format_output(&output);
        assert!(formatted.contains(&output.trace_record.trace_id));
    }

    #[test]
    fn test_format_output_deterministic() {
        let output = crate::cli::runner::CliRunner::run("find nodes");
        let first = format_output(&output);
        for _ in 0..100 {
            let next = format_output(&output);
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_format_verification_stable() {
        let report = crate::cli::runner::CliRunner::verify();
        let formatted = format_verification(&report);
        assert!(formatted.contains("Verification Report"));
    }

    #[test]
    fn test_format_goals_nonempty() {
        let (evaluations, state) = crate::cli::runner::CliRunner::goals("find nodes");
        let formatted = format_goals(&evaluations, &state);
        assert!(formatted.contains("Goal"));
    }

    #[test]
    fn test_format_memory_stable() {
        let snapshot = crate::cli::runner::CliRunner::memory("find nodes");
        let formatted = format_memory(&snapshot);
        assert!(formatted.contains("Memory Snapshot"));
    }

    #[test]
    fn test_format_output_stable_100_runs() {
        let output = crate::cli::runner::CliRunner::run("describe node 42");
        let first = format_output(&output);
        for _ in 0..100 {
            let o = crate::cli::runner::CliRunner::run("describe node 42");
            assert_eq!(first, format_output(&o));
        }
    }

    #[test]
    fn test_format_trace_contains_trace_id() {
        let output = crate::cli::runner::CliRunner::run("test");
        let formatted = format_trace(&output.trace_record);
        assert!(formatted.contains("Trace ID"));
    }
}
