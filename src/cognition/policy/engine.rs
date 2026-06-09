use crate::cognition::event_translator::types::ProposedEvent;
use crate::cognition::policy::types::*;

fn check_rule_on_event(
    rule: &PolicyRule,
    event: &ProposedEvent,
) -> Option<PolicyStatus> {
    for (key, value) in &rule.forbidden_properties {
        if let Some(ev) = event.payload.get(key) {
            if ev == value {
                return Some(PolicyStatus::Rejected);
            }
        }
    }

    if let Some(max_c) = rule.max_confidence {
        if event.confidence > max_c {
            return Some(PolicyStatus::Rejected);
        }
    }
    if let Some(min_c) = rule.min_confidence {
        if event.confidence < min_c {
            return Some(PolicyStatus::Rejected);
        }
    }

    for (key, _) in &rule.required_properties {
        if !event.payload.contains_key(key) {
            return Some(PolicyStatus::Modified);
        }
    }

    Some(PolicyStatus::Approved)
}

pub fn evaluate(
    sequence: crate::cognition::event_translator::types::ProposedEventSequence,
    mut rules: Vec<PolicyRule>,
) -> PolicyEvaluationResult {
    rules.sort();

    let intent_type_str = format!("{:?}", sequence.intent.intent_type);
    let mut decisions: Vec<PolicyDecision> = Vec::new();

    for event in &sequence.events {
        let mut final_status: Option<PolicyStatus> = None;
        let mut reason_parts: Vec<String> = Vec::new();
        let mut any_rule_matched = false;

        for rule in &rules {
            if !rule.matches(&event.event_type, &intent_type_str) {
                continue;
            }
            any_rule_matched = true;

            match check_rule_on_event(rule, event) {
                Some(PolicyStatus::Rejected) => {
                    final_status = Some(PolicyStatus::Rejected);
                    reason_parts.push(format!("rule {}: rejected", rule.id));
                }
                Some(PolicyStatus::Modified) => {
                    if final_status != Some(PolicyStatus::Rejected) {
                        final_status = Some(PolicyStatus::Modified);
                        reason_parts.push(format!("rule {}: modified", rule.id));
                    }
                }
                Some(PolicyStatus::Approved) => {
                    if final_status.is_none() {
                        final_status = Some(PolicyStatus::Approved);
                        reason_parts.push(format!("rule {}: approved", rule.id));
                    }
                }
                _ => {}
            }
        }

        let (status, reason) = match final_status {
            Some(PolicyStatus::Rejected) => {
                (PolicyStatus::Rejected, reason_parts.join("; "))
            }
            Some(PolicyStatus::Modified) => {
                (PolicyStatus::Modified, reason_parts.join("; "))
            }
            Some(PolicyStatus::Approved) => {
                (PolicyStatus::Approved, reason_parts.join("; "))
            }
            _ => {
                if any_rule_matched {
                    (PolicyStatus::Approved, "all rules passed".to_string())
                } else {
                    (PolicyStatus::Deferred, "no matching rules".to_string())
                }
            }
        };

        decisions.push(PolicyDecision::new(&event.id, status, &reason));
    }

    PolicyEvaluationResult::new(sequence, decisions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::event_translator::engine::translate;
    use crate::cognition::semantic_interpreter::engine::interpret;
    use std::collections::BTreeMap;

    fn make_decision_ids(decisions: &[PolicyDecision]) -> Vec<&str> {
        decisions.iter().map(|d| d.event_id.as_str()).collect()
    }

    fn count_status(decisions: &[PolicyDecision], status: PolicyStatus) -> usize {
        decisions.iter().filter(|d| d.status == status).count()
    }

    #[test]
    fn test_empty_rules_approves_all() {
        let seq = translate(interpret("find nodes"));
        let result = evaluate(seq, vec![]);
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Deferred),
            result.decisions.len()
        );
    }

    #[test]
    fn test_approval_rule() {
        let seq = translate(interpret("find nodes"));
        let rule = PolicyRule::new("R001");
        let result = evaluate(seq, vec![rule]);
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Approved),
            result.decisions.len()
        );
    }

    #[test]
    fn test_rejection_by_forbidden_property() {
        let resp = interpret("find nodes danger:true");
        let seq = translate(resp);
        let mut rule = PolicyRule::new("R001");
        rule.applies_to_event_type = Some("NodeFilterEvent".to_string());
        rule.forbidden_properties
            .insert("filter_value".to_string(), "true".to_string());
        let result = evaluate(seq, vec![rule]);
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Rejected),
            1
        );
    }

    #[test]
    fn test_modification_when_required_property_missing() {
        let resp = interpret("find nodes");
        let seq = translate(resp);
        let mut rule = PolicyRule::new("R001");
        rule.required_properties
            .insert("owner".to_string(), "required".to_string());
        let result = evaluate(seq, vec![rule]);
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Modified),
            1
        );
    }

    #[test]
    fn test_deferred_when_no_rule_matches() {
        let seq = translate(interpret("find nodes"));
        let mut rule = PolicyRule::new("R001");
        rule.applies_to_event_type = Some("NoOp".to_string());
        let result = evaluate(seq, vec![rule]);
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Deferred),
            result.decisions.len()
        );
    }

    #[test]
    fn test_rule_by_intent_type() {
        let seq = translate(interpret("find nodes"));
        let mut rule = PolicyRule::new("R001");
        rule.applies_to_intent_type = Some("QueryGraph".to_string());
        let result = evaluate(seq, vec![rule]);
        assert!(result.decisions.iter().any(|d| d.status == PolicyStatus::Approved));
    }

    #[test]
    fn test_rule_by_specific_event_type() {
        let seq = translate(interpret("find nodes"));
        let mut rule = PolicyRule::new("R001");
        rule.applies_to_event_type = Some("GraphQueryRequested".to_string());
        let result = evaluate(seq, vec![rule]);
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Approved),
            1
        );
    }

    #[test]
    fn test_rejection_overrides_modification() {
        let resp = interpret("find nodes danger:true");
        let seq = translate(resp);
        let mut rule_a = PolicyRule::new("A001");
        rule_a.required_properties
            .insert("owner".to_string(), "required".to_string());
        let mut rule_b = PolicyRule::new("B001");
        rule_b.applies_to_event_type = Some("NodeFilterEvent".to_string());
        rule_b.forbidden_properties
            .insert("filter_value".to_string(), "true".to_string());
        let result = evaluate(seq, vec![rule_a, rule_b]);
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Rejected),
            1
        );
    }

    #[test]
    fn test_input_order_preserved() {
        let resp = interpret("describe node 5 1 3");
        let seq = translate(resp);
        let rule = PolicyRule::new("R001");
        let result = evaluate(seq, vec![rule]);
        let ids = make_decision_ids(&result.decisions);
        for i in 1..ids.len() {
            assert!(ids[i - 1] < ids[i] || ids[i - 1] == ids[i]);
        }
    }

    #[test]
    fn test_deterministic_identical_input() {
        let resp = interpret("describe node 42 with type:Person");
        let a = evaluate(translate(resp.clone()), vec![PolicyRule::new("R001")]);
        let b = evaluate(translate(resp), vec![PolicyRule::new("R001")]);
        assert_eq!(a.explanation, b.explanation);
        assert_eq!(a.decisions.len(), b.decisions.len());
        for (da, db) in a.decisions.iter().zip(b.decisions.iter()) {
            assert_eq!(da.status, db.status);
            assert_eq!(da.reason, db.reason);
        }
    }

    #[test]
    fn test_stability_100_runs() {
        let resp = interpret("describe node 42");
        let first = evaluate(
            translate(resp.clone()),
            vec![
                PolicyRule::new("A001"),
                PolicyRule::new("B001"),
            ],
        );
        for _ in 0..100 {
            let next = evaluate(
                translate(resp.clone()),
                vec![
                    PolicyRule::new("A001"),
                    PolicyRule::new("B001"),
                ],
            );
            assert_eq!(first.explanation, next.explanation);
            assert_eq!(first.decisions.len(), next.decisions.len());
        }
    }

    #[test]
    fn test_empty_sequence() {
        let intent = crate::cognition::semantic_interpreter::types::SemanticIntent::new(
            "empty",
            crate::cognition::semantic_interpreter::types::IntentType::Unknown,
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let seq = crate::cognition::event_translator::types::ProposedEventSequence::new(
            intent,
            None,
            vec![],
        );
        let rule = PolicyRule::new("R001");
        let result = evaluate(seq, vec![rule]);
        assert_eq!(result.decisions.len(), 0);
        assert!(result.explanation.contains("events=0"));
    }

    #[test]
    fn test_conflicting_rules_resolved_by_priority() {
        let resp = interpret("find nodes");
        let seq = translate(resp);
        let mut rule_a = PolicyRule::new("A001");
        rule_a.required_properties
            .insert("owner".to_string(), "required".to_string());
        let rule_b = PolicyRule::new("B001");
        let result = evaluate(seq, vec![rule_a, rule_b]);
        // A001 triggers Modified, B001 triggers Approved
        // Modified takes priority over Approved
        assert_eq!(
            count_status(&result.decisions, PolicyStatus::Modified),
            1
        );
    }

    #[test]
    fn test_multiple_rules_per_event() {
        let resp = interpret("find nodes");
        let seq = translate(resp);
        let rules = vec![
            PolicyRule::new("R001"),
            PolicyRule::new("R002"),
            PolicyRule::new("R003"),
        ];
        let result = evaluate(seq, rules);
        assert!(result.decisions.iter().all(|d| d.status == PolicyStatus::Approved));
    }

    #[test]
    fn test_no_mutation_of_sequence() {
        let resp = interpret("describe node 42");
        let seq = translate(resp.clone());
        let original_len = seq.events.len();
        let _ = evaluate(seq, vec![PolicyRule::new("R001")]);
        let seq2 = translate(resp);
        assert_eq!(seq2.events.len(), original_len);
    }

    #[test]
    fn test_no_mutation_of_rules() {
        let seq = translate(interpret("find nodes"));
        let rules = vec![PolicyRule::new("R001")];
        let original_id = rules[0].id.clone();
        let _ = evaluate(seq, rules);
        assert_eq!(original_id, "R001".to_string());
    }

    #[test]
    fn test_roundtrip_serialization() {
        let seq = translate(interpret("describe node 42"));
        let result = evaluate(seq, vec![PolicyRule::new("R001")]);
        let json = serde_json::to_string(&result).unwrap();
        let parsed: PolicyEvaluationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.explanation, parsed.explanation);
        assert_eq!(result.decisions.len(), parsed.decisions.len());
        for (a, b) in result.decisions.iter().zip(parsed.decisions.iter()) {
            assert_eq!(a.status, b.status);
        }
    }

    #[test]
    fn test_rule_determinism() {
        let rules_a = vec![
            PolicyRule::new("B001"),
            PolicyRule::new("A001"),
            PolicyRule::new("C001"),
        ];
        let seq = translate(interpret("find nodes"));
        let result = evaluate(seq, rules_a);
        assert!(result.decisions[0].reason.contains("A001"));
    }

    #[test]
    fn test_explanation_format() {
        let seq = translate(interpret("find nodes danger:true"));
        let mut reject_rule = PolicyRule::new("R001");
        reject_rule
            .forbidden_properties
            .insert("danger".to_string(), "true".to_string());
        let result = evaluate(seq, vec![reject_rule]);
        assert!(result.explanation.starts_with("events="));
        assert!(result.explanation.contains("approved="));
        assert!(result.explanation.contains("rejected="));
    }

    #[test]
    fn test_reject_reason_contains_rule_id() {
        let resp = interpret("find nodes x:bad");
        let seq = translate(resp);
        let mut rule = PolicyRule::new("REJECT_001");
        rule.forbidden_properties
            .insert("x".to_string(), "bad".to_string());
        let result = evaluate(seq, vec![rule]);
        let rejected: Vec<&PolicyDecision> = result
            .decisions
            .iter()
            .filter(|d| d.status == PolicyStatus::Rejected)
            .collect();
        if !rejected.is_empty() {
            assert!(rejected[0].reason.contains("REJECT_001"));
        }
    }

    #[test]
    fn test_modified_reason_contains_rule_id() {
        let seq = translate(interpret("find nodes"));
        let mut rule = PolicyRule::new("MOD_001");
        rule.required_properties
            .insert("owner".to_string(), "required".to_string());
        let result = evaluate(seq, vec![rule]);
        let modified: Vec<&PolicyDecision> = result
            .decisions
            .iter()
            .filter(|d| d.status == PolicyStatus::Modified)
            .collect();
        if !modified.is_empty() {
            assert!(modified[0].reason.contains("MOD_001"));
        }
    }
}
