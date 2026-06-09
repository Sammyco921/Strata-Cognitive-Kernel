use std::collections::{BTreeMap, BTreeSet};

use crate::cognition::memory::types::CognitiveMemoryState;
use crate::cognition::policy::types::PolicyRule;
use crate::cognition::semantic_interpreter::types::{IntentType, SemanticIntent, SemanticResponse};
use crate::cognition::trace::types::TraceRecord;
use crate::ontology::OntologyRegistry;

use super::types::*;

pub const SUCCESS_RATE_WEIGHT: i64 = 1;
pub const FREQUENCY_WEIGHT: i64 = 1;
pub const ONTOLOGY_WEIGHT: i64 = 1;
pub const RULE_WEIGHT: i64 = 1;

pub struct PolicyContext<'a> {
    pub ontology_registry: &'a OntologyRegistry,
    pub policy_rules: &'a [PolicyRule],
    pub cognitive_memory: &'a CognitiveMemoryState,
}

fn base_score_for_intent(intent_type: &IntentType) -> i64 {
    match intent_type {
        IntentType::QueryGraph => 50,
        IntentType::QueryOntology => 40,
        IntentType::QuerySemantic => 35,
        IntentType::DescribeNode => 45,
        IntentType::DescribeGraph => 55,
        IntentType::Unknown => 10,
    }
}

fn semantic_alignment(intent: &SemanticIntent, ontology: &OntologyRegistry) -> i64 {
    let mut count: i64 = 0;
    for entity_key in intent.extracted_entities.keys() {
        if ontology.entity_types.contains_key(entity_key) {
            count += 1;
        }
    }
    count
}

fn ontology_match(intent: &SemanticIntent, ontology: &OntologyRegistry) -> i64 {
    let mut count: i64 = 0;
    for prop_key in intent.extracted_properties.keys() {
        if ontology.property_types.contains_key(prop_key) {
            count += 1;
        }
    }
    count
}

fn rule_alignment(intent: &SemanticIntent, rules: &[PolicyRule]) -> i64 {
    let mut count: i64 = 0;
    let intent_type_str = format!("{:?}", intent.intent_type);
    for rule in rules {
        if rule.matches_intent_type(&intent_type_str) {
            count += 1;
        }
    }
    count
}

fn deterministic_hash(s: &str) -> i64 {
    let mut h: i64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as i64);
    }
    h.abs()
}

fn historical_weight(intent_id: &str) -> i64 {
    deterministic_hash(intent_id) % 21
}

fn approx_log2(x: u64) -> i64 {
    if x == 0 {
        return 0;
    }
    let mut n = 0;
    let mut y = x;
    while y > 1 {
        y >>= 1;
        n += 1;
    }
    n
}

fn compute_memory_profiles(traces: &[TraceRecord]) -> BTreeMap<String, MemoryProfile> {
    let mut profiles: BTreeMap<String, MemoryProfile> = BTreeMap::new();

    for trace in traces {
        let intent_id = trace.semantic_response.intent.id.clone();

        let is_success = trace
            .execution_result
            .results
            .iter()
            .all(|r| r.success)
            ;

        let entry = profiles.entry(intent_id.clone()).or_insert(MemoryProfile {
            intent_id: intent_id.clone(),
            success_count: 0,
            failure_count: 0,
            success_rate: 0,
            frequency_weight: 0,
            ontology_affinity_map: BTreeMap::new(),
            rule_affinity_map: BTreeMap::new(),
        });

        if is_success {
            entry.success_count += 1;
        } else {
            entry.failure_count += 1;
        }

        for event in &trace.proposed_sequence.events {
            for key in event.payload.keys() {
                *entry.ontology_affinity_map.entry(key.clone()).or_insert(0) += 1;
            }
        }

        for decision in &trace.policy_result.decisions {
            *entry
                .rule_affinity_map
                .entry(decision.reason.clone())
                .or_insert(0) += 1;
        }
    }

    for profile in profiles.values_mut() {
        let total = profile.success_count + profile.failure_count;
        profile.success_rate = if total == 0 {
            0
        } else {
            (profile.success_count * 100) / total
        };
        profile.frequency_weight = profile.success_count + profile.failure_count;
    }

    profiles
}

fn compute_memory_influence(profile: &MemoryProfile) -> MemoryInfluence {
    let sr_contrib = SUCCESS_RATE_WEIGHT * profile.success_rate;
    let freq_contrib = FREQUENCY_WEIGHT * approx_log2((profile.frequency_weight + 1) as u64);
    let best_ontology_hit = profile
        .ontology_affinity_map
        .values()
        .cloned()
        .max()
        .unwrap_or(0);
    let ont_contrib = ONTOLOGY_WEIGHT * best_ontology_hit;
    let best_rule_hit = profile
        .rule_affinity_map
        .values()
        .cloned()
        .max()
        .unwrap_or(0);
    let rule_contrib = RULE_WEIGHT * best_rule_hit;
    let total = sr_contrib
        .wrapping_add(freq_contrib)
        .wrapping_add(ont_contrib)
        .wrapping_add(rule_contrib);

    MemoryInfluence {
        intent_id: profile.intent_id.clone(),
        success_rate_contribution: sr_contrib,
        frequency_contribution: freq_contrib,
        ontology_contribution: ont_contrib,
        rule_contribution: rule_contrib,
        total_bias: total,
    }
}

fn memory_weight(memory: &CognitiveMemoryState, intent_type: &IntentType) -> f64 {
    let success = memory.intent_success_counts.get(intent_type).copied().unwrap_or(0);
    let failure = memory.intent_failure_counts.get(intent_type).copied().unwrap_or(0);
    let ratio = (success + 1) as f64 / (failure + 1) as f64;
    ratio.clamp(0.1, 2.0)
}

pub fn score_intent_with_memory(
    intent: &SemanticIntent,
    context: &PolicyContext,
) -> PolicyScore {
    let base = base_score_for_intent(&intent.intent_type);
    let sa = semantic_alignment(intent, context.ontology_registry);
    let om = ontology_match(intent, context.ontology_registry);
    let ra = rule_alignment(intent, context.policy_rules);
    let hw = historical_weight(&intent.id);
    let mw = memory_weight(context.cognitive_memory, &intent.intent_type);

    PolicyScore::compute(base, sa, om, ra, hw, mw, &intent.id)
}

pub fn rank_candidates(mut candidates: Vec<PolicyCandidate>) -> Vec<PolicyCandidate> {
    candidates.sort_by(|a, b| {
        b.score.final_score
            .to_bits()
            .cmp(&a.score.final_score.to_bits())
            .then_with(|| a.intent.id.cmp(&b.intent.id))
            .then_with(|| a.intent.intent_type.cmp(&b.intent.intent_type))
    });
    candidates
}

pub fn select_candidate(candidates: Vec<PolicyCandidate>) -> PolicyCandidate {
    let mut ranked = rank_candidates(candidates);
    ranked.remove(0)
}

pub fn build_policy_decision(
    candidates: Vec<PolicyCandidate>,
    memory_profiles: BTreeMap<String, MemoryProfile>,
    memory_influences: BTreeMap<String, MemoryInfluence>,
) -> PolicyDecision {
    let ranked = rank_candidates(candidates);
    let selected = ranked[0].clone();
    PolicyDecision::new(selected, ranked, memory_profiles, memory_influences)
}

pub fn generate_candidates(response: &SemanticResponse) -> Vec<SemanticIntent> {
    let original = response.intent.clone();
    let mut intents: Vec<SemanticIntent> = Vec::new();
    intents.push(original.clone());

    let variant_types = variant_intent_types(&original.intent_type);
    for vt in variant_types {
        let variant_input = format!("{} [variant:{:?}]", original.raw_input, vt);
        let variant = SemanticIntent::new(
            &variant_input,
            vt,
            original.extracted_entities.clone(),
            original.extracted_properties.clone(),
        );
        intents.push(variant);
    }

    intents.sort_by(|a, b| a.id.cmp(&b.id));
    intents.dedup_by_key(|i| i.id.clone());
    intents
}

fn variant_intent_types(intent_type: &IntentType) -> BTreeSet<IntentType> {
    let mut variants = BTreeSet::new();
    match intent_type {
        IntentType::QueryGraph => {
            variants.insert(IntentType::QuerySemantic);
            variants.insert(IntentType::DescribeGraph);
        }
        IntentType::QueryOntology => {
            variants.insert(IntentType::QuerySemantic);
        }
        IntentType::QuerySemantic => {
            variants.insert(IntentType::QueryGraph);
        }
        IntentType::DescribeNode => {
            variants.insert(IntentType::QueryGraph);
        }
        IntentType::DescribeGraph => {
            variants.insert(IntentType::QueryGraph);
        }
        IntentType::Unknown => {
            variants.insert(IntentType::QueryGraph);
            variants.insert(IntentType::QuerySemantic);
        }
    }
    variants
}

pub fn run_policy_layer(
    response: &SemanticResponse,
    ontology_registry: &OntologyRegistry,
    policy_rules: &[PolicyRule],
    historical_traces: &[TraceRecord],
    cognitive_memory: &CognitiveMemoryState,
) -> PolicyDecision {
    let memory_profiles = compute_memory_profiles(historical_traces);
    let memory_influences: BTreeMap<String, MemoryInfluence> = memory_profiles
        .values()
        .map(|p| {
            let mi = compute_memory_influence(p);
            (mi.intent_id.clone(), mi)
        })
        .collect();

    let candidates = generate_candidates(response);
    let context = PolicyContext {
        ontology_registry,
        policy_rules,
        cognitive_memory,
    };

    let scored: Vec<PolicyCandidate> = candidates
        .into_iter()
        .map(|intent| {
            let score = score_intent_with_memory(&intent, &context);
            PolicyCandidate::new(intent, score)
        })
        .collect();

    build_policy_decision(scored, memory_profiles, memory_influences)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::cognition::execution_adapter::types::{ExecutionPlan, ExecutionPlanResult, ExecutionResult, KernelCommand};
    use crate::cognition::event_translator::types::{ProposedEvent, ProposedEventSequence};
    use crate::cognition::policy::types::{PolicyDecision as M3Decision, PolicyEvaluationResult, PolicyStatus};
    use crate::cognition::memory::types::CognitiveMemoryState;
    use crate::cognition::semantic_interpreter::types::SemanticQuery;

    fn make_intent(raw: &str, it: IntentType) -> SemanticIntent {
        SemanticIntent::new(raw, it, BTreeMap::new(), BTreeMap::new())
    }

    fn make_empty_response(raw: &str, it: IntentType) -> SemanticResponse {
        SemanticResponse {
            intent: make_intent(raw, it),
            query: None,
            explanation: "test".to_string(),
        }
    }

    fn empty_memory() -> CognitiveMemoryState {
        CognitiveMemoryState::empty()
    }

    fn empty_context<'a>(registry: &'a OntologyRegistry, rules: &'a [PolicyRule], memory: &'a CognitiveMemoryState) -> PolicyContext<'a> {
        PolicyContext {
            ontology_registry: registry,
            policy_rules: rules,
            cognitive_memory: memory,
        }
    }

    fn context_with_memory<'a>(
        registry: &'a OntologyRegistry,
        rules: &'a [PolicyRule],
        memory: &'a CognitiveMemoryState,
    ) -> PolicyContext<'a> {
        PolicyContext {
            ontology_registry: registry,
            policy_rules: rules,
            cognitive_memory: memory,
        }
    }

    fn make_trace(intent_id: &str, success: bool) -> TraceRecord {
        let intent = SemanticIntent::new(
            &format!("input_for_{}", intent_id),
            IntentType::QueryGraph,
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let query = Some(SemanticQuery::new());
        let sr = SemanticResponse {
            intent: intent.clone(),
            query: query.clone(),
            explanation: "test".to_string(),
        };
        let event = ProposedEvent::new("evt_0", "GraphQueryRequested", BTreeMap::new(), &intent.id);
        let seq = ProposedEventSequence::new(intent.clone(), query, vec![event]);
        let decisions = vec![M3Decision::new("evt_0", PolicyStatus::Approved, "match")];
        let pr = PolicyEvaluationResult::new(seq.clone(), decisions);
        let cmd = KernelCommand::new("cmd:evt_0", "QueryGraph", BTreeMap::new(), &intent.id, "evt_0");
        let plan = ExecutionPlan::new(&intent.id, vec![cmd]);
        let result = ExecutionResult::new(
            "cmd:evt_0",
            success,
            if success { "ok" } else { "fail" },
            if success { None } else { Some("error".to_string()) },
        );
        let er = ExecutionPlanResult::new(plan.clone(), vec![result]);
        TraceRecord::new(sr, seq, pr, plan, er)
    }

    // ── Memory Profile ────────────────────────────────────────────────

    #[test]
    fn test_empty_trace_history() {
        let profiles = compute_memory_profiles(&[]);
        assert!(profiles.is_empty());
    }

    fn intent_id_of(traces: &[TraceRecord]) -> String {
        traces[0].semantic_response.intent.id.clone()
    }

    #[test]
    fn test_success_counting() {
        let traces = vec![make_trace("int_a", true)];
        let key = intent_id_of(&traces);
        let profiles = compute_memory_profiles(&traces);
        let p = profiles.get(&key).unwrap();
        assert_eq!(p.success_count, 1);
        assert_eq!(p.failure_count, 0);
        assert_eq!(p.success_rate, 100);
    }

    #[test]
    fn test_failure_counting() {
        let traces = vec![make_trace("int_a", false)];
        let key = intent_id_of(&traces);
        let profiles = compute_memory_profiles(&traces);
        let p = profiles.get(&key).unwrap();
        assert_eq!(p.success_count, 0);
        assert_eq!(p.failure_count, 1);
        assert_eq!(p.success_rate, 0);
    }

    #[test]
    fn test_mixed_success_failure() {
        let traces = vec![
            make_trace("int_a", true),
            make_trace("int_a", false),
            make_trace("int_a", true),
        ];
        let key = intent_id_of(&traces);
        let profiles = compute_memory_profiles(&traces);
        let p = profiles.get(&key).unwrap();
        assert_eq!(p.success_count, 2);
        assert_eq!(p.failure_count, 1);
        assert_eq!(p.success_rate, 66);
    }

    #[test]
    fn test_frequency_weight() {
        let traces = vec![
            make_trace("int_a", true),
            make_trace("int_a", true),
            make_trace("int_a", true),
        ];
        let key = intent_id_of(&traces);
        let profiles = compute_memory_profiles(&traces);
        let p = profiles.get(&key).unwrap();
        assert_eq!(p.frequency_weight, 3);
    }

    #[test]
    fn test_ontology_affinity_map() {
        let intent = SemanticIntent::new(
            "test",
            IntentType::QueryGraph,
            BTreeMap::new(),
            BTreeMap::new(),
            );
        let sr = SemanticResponse {
            intent: intent.clone(),
            query: None,
            explanation: "test".to_string(),
        };
        let mut payload = BTreeMap::new();
        payload.insert("Person".to_string(), "entity".to_string());
        let event = ProposedEvent::new("evt_0", "GraphQueryRequested", payload, &intent.id);
        let seq = ProposedEventSequence::new(intent.clone(), None, vec![event]);
        let decisions = vec![M3Decision::new("evt_0", PolicyStatus::Approved, "R001")];
        let pr = PolicyEvaluationResult::new(seq.clone(), decisions);
        let cmd = KernelCommand::new("cmd:evt_0", "QueryGraph", BTreeMap::new(), &intent.id, "evt_0");
        let plan = ExecutionPlan::new(&intent.id, vec![cmd]);
        let result = ExecutionResult::new("cmd:evt_0", true, "ok", None);
        let er = ExecutionPlanResult::new(plan.clone(), vec![result]);
        let trace = TraceRecord::new(sr, seq, pr, plan, er);

        let profiles = compute_memory_profiles(&[trace]);
        let p = profiles.get(&intent.id).unwrap();
        assert_eq!(p.ontology_affinity_map.get("Person").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_rule_affinity_map() {
        let intent = make_intent("test", IntentType::QueryGraph);
        let sr = SemanticResponse {
            intent: intent.clone(),
            query: None,
            explanation: "test".to_string(),
        };
        let event = ProposedEvent::new("evt_0", "GraphQueryRequested", BTreeMap::new(), &intent.id);
        let seq = ProposedEventSequence::new(intent.clone(), None, vec![event]);
        let decisions = vec![M3Decision::new("evt_0", PolicyStatus::Approved, "policy_match")];
        let pr = PolicyEvaluationResult::new(seq.clone(), decisions);
        let cmd = KernelCommand::new("cmd:evt_0", "QueryGraph", BTreeMap::new(), &intent.id, "evt_0");
        let plan = ExecutionPlan::new(&intent.id, vec![cmd]);
        let result = ExecutionResult::new("cmd:evt_0", true, "ok", None);
        let er = ExecutionPlanResult::new(plan.clone(), vec![result]);
        let trace = TraceRecord::new(sr, seq, pr, plan, er);

        let profiles = compute_memory_profiles(&[trace]);
        let p = profiles.get(&intent.id).unwrap();
        assert_eq!(p.rule_affinity_map.get("policy_match").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_multiple_intents_separate_profiles() {
        let a = make_trace("int_a", true);
        let b = make_trace("int_b", true);
        let profiles = compute_memory_profiles(&[a, b]);
        assert_eq!(profiles.len(), 2);
    }

    #[test]
    fn test_deterministic_profiles_100_runs() {
        let traces = vec![
            make_trace("int_a", true),
            make_trace("int_a", false),
            make_trace("int_b", true),
        ];
        let first = compute_memory_profiles(&traces);
        for _ in 0..100 {
            let next = compute_memory_profiles(&traces);
            assert_eq!(first, next);
        }
    }

    // ── Memory Influence ──────────────────────────────────────────────

    #[test]
    fn test_memory_influence_perfect_has_bias() {
        let profile = MemoryProfile {
            intent_id: "test".to_string(),
            success_count: 10,
            failure_count: 0,
            success_rate: 100,
            frequency_weight: 10,
            ontology_affinity_map: BTreeMap::from([("Person".to_string(), 5)]),
            rule_affinity_map: BTreeMap::from([("match".to_string(), 3)]),
        };
        let mi = compute_memory_influence(&profile);
        assert!(mi.total_bias > 0);
        assert_eq!(mi.success_rate_contribution, 100);
    }

    #[test]
    fn test_memory_influence_zero_if_no_history() {
        let profile = MemoryProfile {
            intent_id: "test".to_string(),
            success_count: 0,
            failure_count: 0,
            success_rate: 0,
            frequency_weight: 0,
            ontology_affinity_map: BTreeMap::new(),
            rule_affinity_map: BTreeMap::new(),
        };
        let mi = compute_memory_influence(&profile);
        assert_eq!(mi.total_bias, 0);
    }

    #[test]
    fn test_memory_influence_all_failures() {
        let profile = MemoryProfile {
            intent_id: "test".to_string(),
            success_count: 0,
            failure_count: 5,
            success_rate: 0,
            frequency_weight: 5,
            ontology_affinity_map: BTreeMap::from([("Node".to_string(), 2)]),
            rule_affinity_map: BTreeMap::new(),
        };
        let mi = compute_memory_influence(&profile);
        assert_eq!(mi.success_rate_contribution, 0);
        assert!(mi.frequency_contribution > 0);
    }

    #[test]
    fn test_memory_influence_deterministic() {
        let profile = MemoryProfile {
            intent_id: "test".to_string(),
            success_count: 3,
            failure_count: 1,
            success_rate: 75,
            frequency_weight: 4,
            ontology_affinity_map: BTreeMap::from([("Person".to_string(), 5)]),
            rule_affinity_map: BTreeMap::from([("R001".to_string(), 2)]),
        };
        let a = compute_memory_influence(&profile);
        for _ in 0..100 {
            let b = compute_memory_influence(&profile);
            assert_eq!(a, b);
        }
    }

    // ── Approx Log ────────────────────────────────────────────────────

    #[test]
    fn test_approx_log2_zero() {
        assert_eq!(approx_log2(0), 0);
    }

    #[test]
    fn test_approx_log2_one() {
        assert_eq!(approx_log2(1), 0);
    }

    #[test]
    fn test_approx_log2_powers() {
        assert_eq!(approx_log2(2), 1);
        assert_eq!(approx_log2(4), 2);
        assert_eq!(approx_log2(8), 3);
        assert_eq!(approx_log2(16), 4);
    }

    #[test]
    fn test_approx_log2_non_powers() {
        assert_eq!(approx_log2(3), 1);
        assert_eq!(approx_log2(7), 2);
        assert_eq!(approx_log2(15), 3);
    }

    // ── Determinism ──────────────────────────────────────────────────

    #[test]
    fn test_deterministic_identical_inputs_100_runs_with_memory() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_a", true)];
        let first = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        for _ in 0..100 {
            let next = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_stable_ranking_under_shuffled_input_with_memory() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let _traces = vec![make_trace("int_a", true)];
        let empty_mem = empty_memory();
        let ctx = empty_context(&local_registry, &local_rules, &empty_mem);
        let candidates = generate_candidates(&response);
        let scored: Vec<PolicyCandidate> = candidates
            .into_iter()
            .map(|intent| {
                let score = score_intent_with_memory(&intent, &ctx);
                PolicyCandidate::new(intent, score)
            })
            .collect();

        let ranked_forward = rank_candidates(scored.clone());
        let mut reversed = scored.clone();
        reversed.reverse();
        let ranked_reversed = rank_candidates(reversed);
        assert_eq!(ranked_forward, ranked_reversed);
    }

    #[test]
    fn test_tie_breaking_consistency_with_memory() {
        let a = make_intent("input A", IntentType::QueryGraph);
        let b = make_intent("input B", IntentType::DescribeNode);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let empty_mem = empty_memory();
        let ctx = empty_context(&local_registry, &local_rules, &empty_mem);
        let sa = score_intent_with_memory(&a, &ctx);
        let sb = score_intent_with_memory(&b, &ctx);

        let ca = PolicyCandidate::new(a.clone(), sa);
        let cb = PolicyCandidate::new(b.clone(), sb);

        let ranked_ab = rank_candidates(vec![ca.clone(), cb.clone()]);
        let ranked_ba = rank_candidates(vec![cb, ca]);
        assert_eq!(ranked_ab, ranked_ba);
    }

    // ── Scoring ───────────────────────────────────────────────────────

    #[test]
    fn test_base_score_query_graph() {
        assert_eq!(base_score_for_intent(&IntentType::QueryGraph), 50);
    }

    #[test]
    fn test_base_score_unknown() {
        assert_eq!(base_score_for_intent(&IntentType::Unknown), 10);
    }

    #[test]
    fn test_base_score_all_types_unique() {
        let types = [
            IntentType::QueryGraph,
            IntentType::QueryOntology,
            IntentType::QuerySemantic,
            IntentType::DescribeNode,
            IntentType::DescribeGraph,
            IntentType::Unknown,
        ];
        let scores: BTreeSet<i64> = types.iter().map(|t| base_score_for_intent(t)).collect();
        assert_eq!(scores.len(), types.len());
    }

    #[test]
    fn test_semantic_alignment_matches_ontology() {
        let mut registry = OntologyRegistry::empty();
        registry.entity_types.insert("Person".to_string(), crate::ontology::types::EntityType {
            name: "Person".to_string(),
            description: Some("".to_string()),
        });
        let mut entities = BTreeMap::new();
        entities.insert("Person".to_string(), "entity".to_string());
        let intent = SemanticIntent::new("find Person", IntentType::QueryGraph, entities, BTreeMap::new());
        let count = semantic_alignment(&intent, &registry);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_semantic_alignment_no_match() {
        let registry = OntologyRegistry::empty();
        let mut entities = BTreeMap::new();
        entities.insert("Foo".to_string(), "entity".to_string());
        let intent = SemanticIntent::new("find Foo", IntentType::QueryGraph, entities, BTreeMap::new());
        let count = semantic_alignment(&intent, &registry);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_ontology_match_property_types() {
        let mut registry = OntologyRegistry::empty();
        registry.property_types.insert("age".to_string(), crate::ontology::types::PropertyType {
            name: "age".to_string(),
            value_type: "Integer".to_string(),
            description: Some("".to_string()),
        });
        let mut props = BTreeMap::new();
        props.insert("age".to_string(), "30".to_string());
        let intent = SemanticIntent::new("find age:30", IntentType::QueryGraph, BTreeMap::new(), props);
        let count = ontology_match(&intent, &registry);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_rule_alignment_count() {
        let rules = vec![
            PolicyRule::new("R001"),
            PolicyRule::new("R002"),
        ];
        let intent = make_intent("test", IntentType::QueryGraph);
        let count = rule_alignment(&intent, &rules);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_rule_alignment_with_filter() {
        let mut rule = PolicyRule::new("R_GRAPH");
        rule.applies_to_intent_type = Some("QueryGraph".to_string());
        let rules = vec![rule];
        let intent = make_intent("test", IntentType::QueryGraph);
        assert_eq!(rule_alignment(&intent, &rules), 1);

        let intent_unknown = make_intent("test", IntentType::Unknown);
        assert_eq!(rule_alignment(&intent_unknown, &rules), 0);
    }

    #[test]
    fn test_historical_hash_stability() {
        let id = "int_abc123".to_string();
        let a = historical_weight(&id);
        for _ in 0..100 {
            assert_eq!(a, historical_weight(&id));
        }
    }

    #[test]
    fn test_historical_hash_in_range() {
        let ids = ["int_a", "int_b", "int_c", "int_long_id_here"];
        for id in &ids {
            let w = historical_weight(id);
            assert!(w >= 0 && w <= 20, "weight {} out of range for id {}", w, id);
        }
    }

    #[test]
    fn test_score_intent_basic() {
        let intent = make_intent("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let empty_mem = empty_memory();
        let ctx = empty_context(&local_registry, &local_rules, &empty_mem);
        let score = score_intent_with_memory(&intent, &ctx);
        assert_eq!(score.base_score, 50);
        assert_eq!(score.intent_id, intent.id);
        assert!(score.final_score >= score.base_score as f64);
    }

    #[test]
    fn test_score_includes_memory_weight() {
        let intent = make_intent("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let empty_mem = CognitiveMemoryState::empty();
        let ctx_neutral = context_with_memory(&local_registry, &local_rules, &empty_mem);
        let score_neutral = score_intent_with_memory(&intent, &ctx_neutral);

        let mut high_success_mem = CognitiveMemoryState::empty();
        high_success_mem.intent_success_counts.insert(IntentType::QueryGraph, 100);
        let ctx_high = context_with_memory(&local_registry, &local_rules, &high_success_mem);
        let score_high = score_intent_with_memory(&intent, &ctx_high);
        assert!(score_high.final_score > score_neutral.final_score);

        let mut high_fail_mem = CognitiveMemoryState::empty();
        high_fail_mem.intent_failure_counts.insert(IntentType::QueryGraph, 100);
        let ctx_low = context_with_memory(&local_registry, &local_rules, &high_fail_mem);
        let score_low = score_intent_with_memory(&intent, &ctx_low);
        assert!(score_neutral.final_score > score_low.final_score);
    }

    // ── Selection ────────────────────────────────────────────────────

    #[test]
    fn test_select_candidate_top() {
        let candidates = vec![
            PolicyCandidate::new(
                make_intent("low", IntentType::Unknown),
                PolicyScore::compute(10, 0, 0, 0, 0, 0.0, "low"),
            ),
            PolicyCandidate::new(
                make_intent("high", IntentType::QueryGraph),
                PolicyScore::compute(50, 0, 0, 0, 0, 0.0, "high"),
            ),
        ];
        let selected = select_candidate(candidates);
        assert_eq!(selected.intent.intent_type, IntentType::QueryGraph);
    }

    #[test]
    fn test_deterministic_tie_resolution() {
        let candidates = vec![
            PolicyCandidate::new(
                make_intent("A", IntentType::QueryGraph),
                PolicyScore::compute(50, 0, 0, 0, 5, 0.0, "A"),
            ),
            PolicyCandidate::new(
                make_intent("B", IntentType::QueryGraph),
                PolicyScore::compute(50, 0, 0, 0, 5, 0.0, "B"),
            ),
        ];
        let a = select_candidate(candidates.clone());
        let b = select_candidate(candidates);
        assert_eq!(a, b);
    }

    #[test]
    fn test_multi_candidate_ranking_order() {
        let candidates = vec![
            PolicyCandidate::new(
                make_intent("C", IntentType::QueryGraph),
                PolicyScore::compute(50, 0, 0, 0, 0, 0.0, "C"),
            ),
            PolicyCandidate::new(
                make_intent("A", IntentType::QueryGraph),
                PolicyScore::compute(40, 0, 0, 0, 0, 0.0, "A"),
            ),
            PolicyCandidate::new(
                make_intent("B", IntentType::DescribeGraph),
                PolicyScore::compute(55, 0, 0, 0, 0, 0.0, "B"),
            ),
        ];
        let ranked = rank_candidates(candidates);
        assert_eq!(ranked.len(), 3);
        assert!(ranked[0].score.final_score >= ranked[1].score.final_score);
        assert!(ranked[1].score.final_score >= ranked[2].score.final_score);
    }

    #[test]
    fn test_empty_candidate_list_panics() {
        let candidates: Vec<PolicyCandidate> = vec![];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            select_candidate(candidates);
        }));
        assert!(result.is_err());
    }

    // ── Candidate Generation ─────────────────────────────────────────

    #[test]
    fn test_generate_candidates_includes_original() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let candidates = generate_candidates(&response);
        assert!(candidates.iter().any(|i| i.intent_type == IntentType::QueryGraph));
    }

    #[test]
    fn test_generate_candidates_produces_multiple() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let candidates = generate_candidates(&response);
        assert!(candidates.len() >= 2);
    }

    #[test]
    fn test_generate_candidates_deterministic() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let a = generate_candidates(&response);
        let b = generate_candidates(&response);
        assert_eq!(a, b);
    }

    // ── Policy Decision ────────────────────────────────────────────────

    #[test]
    fn test_build_policy_decision_explanation_format() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_x", true)];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        assert!(decision.explanation.starts_with("intent="));
        assert!(decision.explanation.contains(";score="));
        assert!(decision.explanation.contains(";components="));
        assert!(decision.explanation.contains(";memory="));
    }

    #[test]
    fn test_build_policy_decision_ranked_not_empty() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        assert!(!decision.ranked.is_empty());
    }

    #[test]
    fn test_build_policy_decision_selected_matches_ranked_first() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        assert_eq!(decision.selected.score.intent_id, decision.ranked[0].score.intent_id);
    }

    // ── Serialization ──────────────────────────────────────────────────

    #[test]
    fn test_serialization_roundtrip() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_x", true)];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        let json = serde_json::to_string(&decision).unwrap();
        let parsed: PolicyDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decision, parsed);
    }

    #[test]
    fn test_serialization_deterministic() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_x", true)];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        let first = serde_json::to_string(&decision).unwrap();
        for _ in 0..100 {
            assert_eq!(first, serde_json::to_string(&decision).unwrap());
        }
    }

    // ── Policy Score ──────────────────────────────────────────────────

    #[test]
    fn test_policy_score_final_is_sum() {
        let score = PolicyScore::compute(10, 20, 30, 40, 50, 60.0, "test");
        assert!((score.final_score - 240.0).abs() < 1e-9);
    }

    #[test]
    fn test_policy_score_eq() {
        let a = PolicyScore::compute(10, 0, 0, 0, 0, 0.0, "id");
        let b = PolicyScore::compute(10, 0, 0, 0, 0, 0.0, "id");
        assert_eq!(a, b);
    }

    #[test]
    fn test_policy_score_neq() {
        let a = PolicyScore::compute(10, 0, 0, 0, 0, 0.0, "id_a");
        let b = PolicyScore::compute(20, 0, 0, 0, 0, 0.0, "id_b");
        assert_ne!(a, b);
    }

    // ── Pipeline Integration ──────────────────────────────────────────

    #[test]
    fn test_run_policy_layer_returns_decision() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_x", true)];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        assert!(!decision.explanation.is_empty());
        assert!(!decision.ranked.is_empty());
    }

    #[test]
    fn test_run_policy_layer_deterministic() {
        let response = make_empty_response("describe node 42", IntentType::DescribeNode);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_x", true)];
        let a = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        let b = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        assert_eq!(a, b);
    }

    #[test]
    fn test_run_policy_layer_with_ontology_affects_scores() {
        let mut registry = OntologyRegistry::empty();
        registry.entity_types.insert("Person".to_string(), crate::ontology::types::EntityType {
            name: "Person".to_string(),
            description: Some("".to_string()),
        });
        let mut entities = BTreeMap::new();
        entities.insert("Person".to_string(), "entity".to_string());
        let intent = SemanticIntent::new("find Person", IntentType::QueryGraph, entities, BTreeMap::new());
        let response = SemanticResponse {
            intent,
            query: None,
            explanation: "test".to_string(),
        };
        let empty_registry = OntologyRegistry::empty();
        let empty_rules: Vec<PolicyRule> = vec![];
        let traces = vec![];
        let empty_mem = empty_memory();
        let ctx_no = PolicyContext {
            ontology_registry: &empty_registry,
            policy_rules: &empty_rules,
            cognitive_memory: &empty_mem,
        };
        let ctx_ont = PolicyContext {
            ontology_registry: &registry,
            policy_rules: &[],
            cognitive_memory: &empty_mem,
        };
        let decision_no = run_policy_layer(&response, ctx_no.ontology_registry, ctx_no.policy_rules, &traces, &empty_memory());
        let decision_ont = run_policy_layer(&response, ctx_ont.ontology_registry, ctx_ont.policy_rules, &traces, &empty_memory());
        assert_eq!(decision_no.selected.score.semantic_alignment, 0);
        assert_eq!(decision_ont.selected.score.semantic_alignment, 1);
    }

    #[test]
    fn test_memory_influences_ranking() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_high_success", true)];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        assert!(!decision.memory_profiles.is_empty());
        // Memory influences should be <= memory profiles
        assert!(decision.memory_influences.len() <= decision.memory_profiles.len());
    }

    #[test]
    fn test_memory_does_not_override_base_scoring() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_a", true)];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        // Base score (50) should still be the dominant component
        assert!(decision.selected.score.base_score >= 10);
        assert!(decision.selected.score.final_score >= decision.selected.score.base_score as f64);
    }

    #[test]
    fn test_memory_profiles_empty_with_no_traces() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces: Vec<TraceRecord> = vec![];
        let decision = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        assert!(decision.memory_profiles.is_empty());
        assert!(decision.memory_influences.is_empty());
    }

    #[test]
    fn test_score_100_run_stability_with_memory() {
        let response = make_empty_response("find nodes", IntentType::QueryGraph);
        let local_registry = OntologyRegistry::empty();
        let local_rules: Vec<PolicyRule> = vec![];
        let traces = vec![make_trace("int_a", true)];
        let first = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
        for _ in 0..100 {
            let next = run_policy_layer(&response, &local_registry, &local_rules, &traces, &empty_memory());
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_high_success_intent_prioritized_by_memory() {
        let traces = vec![
            make_trace("high_success_int", true),
            make_trace("high_success_int", true),
            make_trace("high_success_int", true),
            make_trace("low_success_int", false),
            make_trace("low_success_int", false),
        ];

        let profiles = compute_memory_profiles(&traces);
        let high_key = traces[0].semantic_response.intent.id.clone();
        let low_key = traces[3].semantic_response.intent.id.clone();
        let high_profile = profiles.get(&high_key).unwrap();
        let low_profile = profiles.get(&low_key).unwrap();
        assert!(high_profile.success_rate > low_profile.success_rate);
    }

    #[test]
    fn test_failed_intents_penalized() {
        let traces = vec![
            make_trace("failing", false),
            make_trace("failing", false),
            make_trace("failing", false),
        ];
        let profiles = compute_memory_profiles(&traces);
        let key = traces[0].semantic_response.intent.id.clone();
        let p = profiles.get(&key).unwrap();
        assert_eq!(p.success_rate, 0);
        assert_eq!(p.failure_count, 3);
    }
}
