use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cognition::semantic_interpreter::types::SemanticIntent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyScore {
    pub intent_id: String,
    pub base_score: i64,
    pub semantic_alignment: i64,
    pub ontology_match: i64,
    pub rule_alignment: i64,
    pub historical_weight: i64,
    pub memory_weight: f64,
    pub final_score: f64,
}

impl PartialEq for PolicyScore {
    fn eq(&self, other: &Self) -> bool {
        self.intent_id == other.intent_id
            && self.base_score == other.base_score
            && self.semantic_alignment == other.semantic_alignment
            && self.ontology_match == other.ontology_match
            && self.rule_alignment == other.rule_alignment
            && self.historical_weight == other.historical_weight
            && self.memory_weight.to_bits() == other.memory_weight.to_bits()
            && self.final_score.to_bits() == other.final_score.to_bits()
    }
}

impl Eq for PolicyScore {}

impl PartialOrd for PolicyScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PolicyScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.final_score
            .to_bits()
            .cmp(&other.final_score.to_bits())
            .then_with(|| self.intent_id.cmp(&other.intent_id))
    }
}

impl PolicyScore {
    pub fn final_score(&self) -> f64 {
        self.final_score
    }

    pub fn compute(base_score: i64, semantic_alignment: i64, ontology_match: i64, rule_alignment: i64, historical_weight: i64, memory_weight: f64, intent_id: &str) -> Self {
        let final_score = (base_score as f64 * 1.0)
            + (semantic_alignment as f64 * 1.2)
            + (ontology_match as f64 * 1.2)
            + (rule_alignment as f64 * 1.0)
            + (historical_weight as f64 * 0.8)
            + (memory_weight * 1.5);
        PolicyScore {
            intent_id: intent_id.to_string(),
            base_score,
            semantic_alignment,
            ontology_match,
            rule_alignment,
            historical_weight,
            memory_weight,
            final_score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyCandidate {
    pub intent: SemanticIntent,
    pub score: PolicyScore,
}

impl PartialOrd for PolicyCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PolicyCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .cmp(&other.score)
            .then_with(|| self.intent.cmp(&other.intent))
    }
}

impl PolicyCandidate {
    pub fn new(intent: SemanticIntent, score: PolicyScore) -> Self {
        PolicyCandidate { intent, score }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProfile {
    pub intent_id: String,
    pub success_count: i64,
    pub failure_count: i64,
    pub success_rate: i64,
    pub frequency_weight: i64,
    pub ontology_affinity_map: BTreeMap<String, i64>,
    pub rule_affinity_map: BTreeMap<String, i64>,
}

impl PartialOrd for MemoryProfile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MemoryProfile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.intent_id.cmp(&other.intent_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryInfluence {
    pub intent_id: String,
    pub success_rate_contribution: i64,
    pub frequency_contribution: i64,
    pub ontology_contribution: i64,
    pub rule_contribution: i64,
    pub total_bias: i64,
}

impl PartialOrd for MemoryInfluence {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MemoryInfluence {
    fn cmp(&self, other: &Self) -> Ordering {
        self.intent_id.cmp(&other.intent_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub selected: PolicyCandidate,
    pub ranked: Vec<PolicyCandidate>,
    pub memory_profiles: BTreeMap<String, MemoryProfile>,
    pub memory_influences: BTreeMap<String, MemoryInfluence>,
    pub explanation: String,
}

impl PartialOrd for PolicyDecision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PolicyDecision {
    fn cmp(&self, other: &Self) -> Ordering {
        self.explanation.cmp(&other.explanation)
    }
}

impl PolicyDecision {
    pub fn new(
        selected: PolicyCandidate,
        ranked: Vec<PolicyCandidate>,
        memory_profiles: BTreeMap<String, MemoryProfile>,
        memory_influences: BTreeMap<String, MemoryInfluence>,
    ) -> Self {
        let mem_info = memory_influences.get(&selected.score.intent_id);
        let memory_part = match mem_info {
            Some(mi) => format!(
                "memory={}|{}|{}|{}",
                mi.success_rate_contribution,
                mi.frequency_contribution,
                mi.ontology_contribution,
                mi.rule_contribution,
            ),
            None => "memory=0|0|0|0".to_string(),
        };
        let explanation = format!(
            "intent={};score={};components={}|{}|{}|{}|{}|{};{}",
            selected.score.intent_id,
            format_f64(selected.score.final_score),
            selected.score.base_score,
            selected.score.semantic_alignment,
            selected.score.ontology_match,
            selected.score.rule_alignment,
            selected.score.historical_weight,
            format_f64(selected.score.memory_weight),
            memory_part,
        );
        PolicyDecision {
            selected,
            ranked,
            memory_profiles,
            memory_influences,
            explanation,
        }
    }
}

fn format_f64(v: f64) -> String {
    let bits = v.to_bits();
    format!("{}", bits)
}
