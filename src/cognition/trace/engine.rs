use crate::cognition::semantic_interpreter::types::SemanticResponse;
use crate::cognition::event_translator::types::ProposedEventSequence;
use crate::cognition::policy::types::PolicyEvaluationResult;
use crate::cognition::execution_adapter::types::{ExecutionPlan, ExecutionPlanResult};

use super::types::TraceRecord;

pub fn record_trace(
    semantic_response: SemanticResponse,
    proposed_sequence: ProposedEventSequence,
    policy_result: PolicyEvaluationResult,
    execution_plan: ExecutionPlan,
    execution_result: ExecutionPlanResult,
) -> TraceRecord {
    TraceRecord::new(
        semantic_response,
        proposed_sequence,
        policy_result,
        execution_plan,
        execution_result,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::semantic_interpreter::engine::interpret;
    use crate::cognition::event_translator::engine::translate;
    use crate::cognition::policy::engine::evaluate;
    use crate::cognition::policy::types::PolicyRule;
    use crate::cognition::execution_adapter::engine::{build_plan, execute_plan};

    #[test]
    fn test_record_trace_creates_record() {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let rule = PolicyRule::new("R001");
        let policy = evaluate(seq.clone(), vec![rule]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        assert!(trace.trace_id.starts_with("int_"));
        assert_eq!(trace.raw_input, "find nodes");
    }

    #[test]
    fn test_record_trace_does_not_modify_inputs() {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let rule = PolicyRule::new("R001");
        let policy = evaluate(seq.clone(), vec![rule]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let original_id = seq.intent.id.clone();
        let _trace = record_trace(resp, seq.clone(), policy.clone(), plan.clone(), result);
        // Inputs are unchanged after recording
        assert_eq!(seq.intent.id, original_id);
        assert_eq!(policy.decisions.len(), seq.events.len());
    }

    #[test]
    fn test_record_trace_preserves_all_fields() {
        let resp = interpret("describe node 42");
        let seq = translate(resp.clone());
        let rule = PolicyRule::new("R001");
        let policy = evaluate(seq.clone(), vec![rule]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        assert_eq!(trace.proposed_sequence.events.len(), trace.execution_plan.commands.len());
        assert_eq!(trace.proposed_sequence.intent.intent_type, trace.semantic_response.intent.intent_type);
    }

    #[test]
    fn test_trace_id_consistency() {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let rule = PolicyRule::new("R001");
        let policy = evaluate(seq.clone(), vec![rule]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq, policy, plan, result);
        let intent_id = trace.semantic_response.intent.id.clone();
        assert_eq!(trace.trace_id, intent_id);
        // All stage outputs share the trace_id
        assert_eq!(trace.proposed_sequence.intent.id, intent_id);
        assert_eq!(trace.policy_result.sequence.intent.id, intent_id);
    }

    #[test]
    fn test_deterministic_record() {
        let a = {
            let resp = interpret("find nodes");
            let seq = translate(resp.clone());
            let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
            let plan = build_plan(seq.clone(), policy.clone());
            let result = execute_plan(plan.clone());
            record_trace(resp, seq, policy, plan, result)
        };
        let b = {
            let resp = interpret("find nodes");
            let seq = translate(resp.clone());
            let policy = evaluate(seq.clone(), vec![PolicyRule::new("R001")]);
            let plan = build_plan(seq.clone(), policy.clone());
            let result = execute_plan(plan.clone());
            record_trace(resp, seq, policy, plan, result)
        };
        assert_eq!(a, b);
    }

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

    #[test]
    fn test_modifying_inputs_after_trace_no_effect() {
        let resp = interpret("find nodes");
        let seq = translate(resp.clone());
        let rule = PolicyRule::new("R001");
        let policy = evaluate(seq.clone(), vec![rule]);
        let plan = build_plan(seq.clone(), policy.clone());
        let result = execute_plan(plan.clone());
        let trace = record_trace(resp, seq.clone(), policy.clone(), plan.clone(), result);
        // Modify originals after recording
        let mut seq2 = seq.clone();
        seq2.events.clear();
        assert_eq!(trace.proposed_sequence.events.len(), 1);
        assert_ne!(seq2.events.len(), trace.proposed_sequence.events.len());
    }
}
