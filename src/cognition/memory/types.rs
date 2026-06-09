use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::cognition::semantic_interpreter::types::IntentType;
use crate::cognition::trace::types::TraceRecord;
use crate::cognition::coherence::types::CoherenceReport;
use crate::cognition::system::policy::types::PolicyDecision;

fn f64_eq(a: &f64, b: &f64) -> bool {
    a.to_bits() == b.to_bits()
}

fn f64_map_eq(a: &BTreeMap<String, f64>, b: &BTreeMap<String, f64>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (k, va) in a {
        match b.get(k) {
            Some(vb) if f64_eq(va, vb) => {}
            _ => return false,
        }
    }
    true
}

fn f64_intent_map_eq(a: &BTreeMap<IntentType, f64>, b: &BTreeMap<IntentType, f64>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (k, va) in a {
        match b.get(k) {
            Some(vb) if f64_eq(va, vb) => {}
            _ => return false,
        }
    }
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveMemoryState {
    pub intent_success_counts: BTreeMap<IntentType, u64>,
    pub intent_failure_counts: BTreeMap<IntentType, u64>,
    pub command_success_counts: BTreeMap<String, u64>,
    pub command_failure_counts: BTreeMap<String, u64>,
    pub ontology_alignment_scores: BTreeMap<String, f64>,
    pub last_updated_trace_id: String,
    pub update_counter: u64,
}

impl PartialEq for CognitiveMemoryState {
    fn eq(&self, other: &Self) -> bool {
        self.intent_success_counts == other.intent_success_counts
            && self.intent_failure_counts == other.intent_failure_counts
            && self.command_success_counts == other.command_success_counts
            && self.command_failure_counts == other.command_failure_counts
            && f64_map_eq(&self.ontology_alignment_scores, &other.ontology_alignment_scores)
            && self.last_updated_trace_id == other.last_updated_trace_id
            && self.update_counter == other.update_counter
    }
}

impl Eq for CognitiveMemoryState {}

impl PartialOrd for CognitiveMemoryState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CognitiveMemoryState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.intent_success_counts
            .len()
            .cmp(&other.intent_success_counts.len())
            .then_with(|| self.update_counter.cmp(&other.update_counter))
            .then_with(|| self.last_updated_trace_id.cmp(&other.last_updated_trace_id))
    }
}

impl CognitiveMemoryState {
    pub fn empty() -> Self {
        CognitiveMemoryState {
            intent_success_counts: BTreeMap::new(),
            intent_failure_counts: BTreeMap::new(),
            command_success_counts: BTreeMap::new(),
            command_failure_counts: BTreeMap::new(),
            ontology_alignment_scores: BTreeMap::new(),
            last_updated_trace_id: String::new(),
            update_counter: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryUpdateEvent {
    pub trace_id: String,
    pub intent_type: IntentType,
    pub execution_result_summary: String,
    pub policy_decision_summary: String,
    pub coherence_valid: bool,
    pub command_list: Vec<String>,
}

impl MemoryUpdateEvent {
    pub fn from_trace_and_coherence(
        trace: &TraceRecord,
        coherence: &CoherenceReport,
        decision: &PolicyDecision,
    ) -> Self {
        let trace_id = trace.trace_id.clone();
        let intent_type = trace.semantic_response.intent.intent_type.clone();
        let all_success = trace.execution_result.results.iter().all(|r| r.success);
        let execution_result_summary = if all_success { "all_success".to_string() } else { "partial_failure".to_string() };
        let approved = decision.ranked.iter().filter(|c| c.score.final_score > 0.0).count();
        let total = decision.ranked.len();
        let policy_decision_summary = format!("ranked={};approved={}", total, approved);
        let coherence_valid = coherence.is_valid;
        let command_list: Vec<String> = trace.execution_plan.commands.iter().map(|c| c.command_type.clone()).collect();
        MemoryUpdateEvent {
            trace_id,
            intent_type,
            execution_result_summary,
            policy_decision_summary,
            coherence_valid,
            command_list,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub state: CognitiveMemoryState,
    pub success_rate_per_intent: BTreeMap<IntentType, f64>,
    pub failure_rate_per_intent: BTreeMap<IntentType, f64>,
    pub command_reliability_scores: BTreeMap<String, f64>,
}

impl PartialEq for MemorySnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
            && f64_intent_map_eq(&self.success_rate_per_intent, &other.success_rate_per_intent)
            && f64_intent_map_eq(&self.failure_rate_per_intent, &other.failure_rate_per_intent)
            && f64_map_eq(&self.command_reliability_scores, &other.command_reliability_scores)
    }
}

impl Eq for MemorySnapshot {}

impl MemorySnapshot {
    pub fn from_state(state: CognitiveMemoryState) -> Self {
        let mut success_rate_per_intent = BTreeMap::new();
        let mut failure_rate_per_intent = BTreeMap::new();
        let mut command_reliability_scores = BTreeMap::new();

        for (intent_type, success_count) in &state.intent_success_counts {
            let failure_count = state.intent_failure_counts.get(intent_type).copied().unwrap_or(0);
            let total = success_count + failure_count;
            let rate = if total > 0 {
                *success_count as f64 / total as f64
            } else {
                0.0
            };
            success_rate_per_intent.insert(intent_type.clone(), rate);
        }

        for (intent_type, failure_count) in &state.intent_failure_counts {
            let success_count = state.intent_success_counts.get(intent_type).copied().unwrap_or(0);
            let total = success_count + failure_count;
            let rate = if total > 0 {
                *failure_count as f64 / total as f64
            } else {
                0.0
            };
            failure_rate_per_intent.insert(intent_type.clone(), rate);
        }

        let all_intents: BTreeMap<IntentType, u64> = state.intent_success_counts
            .iter()
            .chain(state.intent_failure_counts.iter())
            .map(|(k, v)| (k.clone(), *v))
            .fold(BTreeMap::new(), |mut acc, (k, v)| {
                *acc.entry(k).or_insert(0) += v;
                acc
            });

        for intent_type in all_intents.keys() {
            if !success_rate_per_intent.contains_key(intent_type) {
                success_rate_per_intent.insert(intent_type.clone(), 0.0);
            }
            if !failure_rate_per_intent.contains_key(intent_type) {
                failure_rate_per_intent.insert(intent_type.clone(), 0.0);
            }
        }

        for (command_type, success_count) in &state.command_success_counts {
            let failure_count = state.command_failure_counts.get(command_type).copied().unwrap_or(0);
            let total = success_count + failure_count;
            let reliability = if total > 0 {
                *success_count as f64 / total as f64
            } else {
                0.0
            };
            command_reliability_scores.insert(command_type.clone(), reliability);
        }

        MemorySnapshot {
            state,
            success_rate_per_intent,
            failure_rate_per_intent,
            command_reliability_scores,
        }
    }
}
