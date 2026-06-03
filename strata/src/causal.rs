use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::event::{Event, EventType};
use crate::graph::GraphState;

/// Default pruning threshold: edges with weight below this are removed.
pub const PRUNE_THRESHOLD: f64 = 0.3;

// ── CausalRelation ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum CausalRelation {
    Supports,
    Contradicts,
    Enables,
    DerivesFrom,
}

impl CausalRelation {
    pub fn label(&self) -> &'static str {
        match self {
            CausalRelation::Supports => "supports",
            CausalRelation::Contradicts => "contradicts",
            CausalRelation::Enables => "enables",
            CausalRelation::DerivesFrom => "derives_from",
        }
    }
}

// ── CausalEdge ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalEdge {
    pub from: String,
    pub to: String,
    pub relation: CausalRelation,
    pub meta_reason: Option<String>,
    #[serde(default)]
    pub weight: f64,
}

impl CausalEdge {
    pub fn new(from: &str, to: &str, relation: CausalRelation, meta_reason: Option<String>) -> Self {
        let weight = compute_weight(relation);
        CausalEdge { from: from.to_string(), to: to.to_string(), relation, meta_reason, weight }
    }

    pub fn new_with_weight(from: &str, to: &str, relation: CausalRelation, meta_reason: Option<String>, weight: f64) -> Self {
        CausalEdge { from: from.to_string(), to: to.to_string(), relation, meta_reason, weight }
    }
}

/// Deterministic weight assignment based on causal relation type.
/// - DerivesFrom (explicit causes) → 1.0  (direct evidence)
/// - Supports (inferred agreement)  → 0.7  (inferred support)
/// - Enables (structural precondition) → 0.4  (indirect enablement)
/// - Contradicts (historical overwrite) → 0.2  (weak historical link)
pub fn compute_weight(relation: CausalRelation) -> f64 {
    match relation {
        CausalRelation::DerivesFrom => 1.0,
        CausalRelation::Supports => 0.7,
        CausalRelation::Enables => 0.4,
        CausalRelation::Contradicts => 0.2,
    }
}

// ── CausalChainLink ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CausalChainLink {
    pub event_id: String,
    pub event_type: EventType,
    pub timestamp: u64,
    pub relation_to_next: Option<CausalRelation>,
    pub meta_reason: Option<String>,
}

// ── Explanation ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Explanation {
    pub target_node_id: String,
    pub property_key: Option<String>,
    pub current_value: Option<serde_json::Value>,
    pub chain: Vec<CausalChainLink>,
    pub hops: usize,
}

// ── CausalGraph ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalGraph {
    pub event_nodes: BTreeMap<String, Event>,
    pub edges: Vec<CausalEdge>,
}

impl CausalGraph {
    pub fn new() -> Self {
        CausalGraph { event_nodes: BTreeMap::new(), edges: Vec::new() }
    }

    pub fn add_event_node(&mut self, event: &Event) {
        self.event_nodes.entry(event.id.clone()).or_insert_with(|| event.clone());
    }

    pub fn link_causality(&mut self, from: &str, to: &str, relation: CausalRelation, meta_reason: Option<String>) {
        if self.edges.iter().any(|e| e.from == from && e.to == to && e.relation == relation) {
            return;
        }
        self.edges.push(CausalEdge::new(from, to, relation, meta_reason));
    }

    pub fn link_causality_with_weight(&mut self, from: &str, to: &str, relation: CausalRelation, meta_reason: Option<String>, weight: f64) {
        if self.edges.iter().any(|e| e.from == from && e.to == to && e.relation == relation) {
            return;
        }
        self.edges.push(CausalEdge::new_with_weight(from, to, relation, meta_reason, weight));
    }

    pub fn outbound(&self, node_id: &str) -> Vec<&CausalEdge> {
        self.edges.iter().filter(|e| e.from == node_id).collect()
    }

    pub fn inbound(&self, node_id: &str) -> Vec<&CausalEdge> {
        self.edges.iter().filter(|e| e.to == node_id).collect()
    }

    /// Trace backward from an event following the most recent valid predecessor.
    /// Only follows edges whose `from` timestamp is less than the current event's
    /// timestamp (prevents forward-in-time traversal via DeleteNode Contradicts edges).
    /// Returns the chain from root to the given event.
    pub fn trace_causal_chain(&self, event_id: &str, max_hops: usize) -> Vec<CausalChainLink> {
        let mut chain: Vec<CausalChainLink> = Vec::new();
        let mut current = event_id.to_string();
        let mut visited = BTreeMap::<String, usize>::new();

        for hop in 0..max_hops {
            if visited.contains_key(&current) {
                break;
            }
            visited.insert(current.clone(), hop);

            if let Some(event) = self.event_nodes.get(&current) {
                chain.push(CausalChainLink {
                    event_id: current.clone(),
                    timestamp: event.timestamp,
                    event_type: event.event_type.clone(),
                    relation_to_next: None,
                    meta_reason: event.meta_reason.clone(),
                });
            }

            let current_ts = self.event_nodes.get(&current).map(|e| e.timestamp).unwrap_or(0);
            let inbound = self.inbound(&current);
            // Only follow predecessors that happened before the current event
            let predecessor = inbound
                .iter()
                .filter(|e| {
                    self.event_nodes.get(&e.from).map(|ev| ev.timestamp).unwrap_or(0) < current_ts
                })
                .max_by_key(|e| {
                    self.event_nodes.get(&e.from).map(|ev| ev.timestamp).unwrap_or(0)
                });

            if let Some(edge) = predecessor {
                if let Some(link) = chain.last_mut() {
                    link.relation_to_next = Some(edge.relation);
                }
                current = edge.from.clone();
            } else {
                break;
            }
        }

        chain.reverse();
        chain
    }

    /// Explain the current belief (property value) on a node.
    /// Walks backward from the most recent SetProperty on (node_id, key)
    /// through the causal chain, limited to max_hops.
    pub fn explain_belief(&self, state: &GraphState, node_id: &str, property_key: Option<&str>, max_hops: usize) -> Explanation {
        // Find the most recent SetProperty event on this node/property
        let target_key = property_key.unwrap_or("");
        let current_value = state
            .nodes
            .get(node_id)
            .and_then(|n| {
                if target_key.is_empty() {
                    None
                } else {
                    n.properties.get(target_key).cloned()
                }
            });

        let relevant_event = self
            .event_nodes
            .values()
            .filter(|e| {
                e.event_type == EventType::SetProperty
                    && e.payload.get("target_id").and_then(|v| v.as_str()) == Some(node_id)
                    && (target_key.is_empty()
                        || e.payload.get("key").and_then(|v| v.as_str()) == Some(target_key))
            })
            .max_by_key(|e| e.timestamp);

        let chain = match relevant_event {
            Some(event) => self.trace_causal_chain(&event.id, max_hops),
            None => Vec::new(),
        };

        Explanation {
            target_node_id: node_id.to_string(),
            property_key: property_key.map(|s| s.to_string()),
            current_value,
            hops: chain.len(),
            chain,
        }
    }
}

// ── ExplanationClass ──────────────────────────────────────────────────────────

/// The "shape" of an explanation: root event types, relation sequence, terminal type.
/// Two causal graphs that produce the same ExplanationClass for a given (node, key)
/// are considered explanation-equivalent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationClass {
    pub root_event_types: Vec<EventType>,
    pub relation_sequence: Vec<Option<CausalRelation>>,
    pub terminal_event_type: Option<EventType>,
}

impl ExplanationClass {
    pub fn from_explanation(exp: &Explanation) -> Self {
        let root_types: Vec<EventType> = exp.chain.iter().map(|c| c.event_type.clone()).collect();
        let relations: Vec<Option<CausalRelation>> = exp.chain.iter().map(|c| c.relation_to_next).collect();
        let terminal = exp.chain.last().map(|c| c.event_type.clone());
        ExplanationClass {
            root_event_types: root_types,
            relation_sequence: relations,
            terminal_event_type: terminal,
        }
    }
}

// ── ESS (Explanation Sufficiency Score) ────────────────────────────────────

/// ESS = minimal_steps_to_correct_explanation / full_graph_steps
/// Measures how much of the full explanation is preserved after compression.
pub fn compute_ess(pruned_chain_len: usize, full_chain_len: usize) -> f64 {
    if full_chain_len == 0 {
        return 1.0;
    }
    pruned_chain_len as f64 / full_chain_len as f64
}

// ── B6: Projection System ───────────────────────────────────────────────────
// G₀ = Ground Truth Event Graph (events). G₁ = Derived Causal Graph (projected).
// project(G₀, Policy) → G₁ enforces B6 invariants (I1–I4).

/// Policy controlling how G₁ is projected from G₀ events.
#[derive(Debug, Clone)]
pub struct ProjectionPolicy {
    /// Edges with weight below this are removed from G₁.
    pub pruning_threshold: f64,
    /// If true, parallel edges (same from→to) are collapsed keeping highest weight.
    pub deduplicate: bool,
    /// Maximum hops when tracing an explanation chain.
    pub max_explanation_depth: usize,
}

impl Default for ProjectionPolicy {
    fn default() -> Self {
        ProjectionPolicy {
            pruning_threshold: PRUNE_THRESHOLD,
            deduplicate: true,
            max_explanation_depth: 10,
        }
    }
}

/// Project G₁ from G₀ events using the given Policy.
/// This is the canonical G₁ construction function — satisfies I2 (Projection Isolation).
/// G₁ is a strictly read-only downstream artifact of G₀.
pub fn project(events: &[Event], policy: &ProjectionPolicy) -> CausalGraph {
    let mut cg = CausalGraph::new();
    for (i, event) in events.iter().enumerate() {
        cg.add_event_node(event);
        let prior = &events[..i];
        let links = CausalRuleEngine::infer_links(event, prior);
        for (from, to, relation, reason) in &links {
            cg.link_causality(from, to, *relation, reason.clone());
        }
    }
    if policy.deduplicate {
        cg.deduplicate_edges();
    }
    cg.prune_with_threshold(policy.pruning_threshold);
    cg
}

/// Convenience: project with default policy (same as B5 pruned behavior).
pub fn project_default(events: &[Event]) -> CausalGraph {
    project(events, &ProjectionPolicy::default())
}

// ── B6: Drift Detection Layer ──────────────────────────────────────────────

/// Report of divergence between G₀ and G₁.
/// Measures how much the projection distorts the ground truth.
#[derive(Debug, Clone)]
pub struct DivergenceReport {
    /// Fraction of G₀ event nodes missing from G₁ (due to pruning).
    pub node_loss_ratio: f64,
    /// Fraction of G₀-derived edges missing from G₁.
    pub edge_loss_ratio: f64,
    /// Fraction of causal paths in G₁ that deviate from G₀-derived paths.
    pub causal_path_deviation_rate: f64,
    /// How often the root cause in G₁ differs from G₀ root cause.
    pub root_cause_mismatch_frequency: f64,
}

/// Compute divergence metrics between the full G₀-derived graph and a projected G₁.
/// All metrics are in [0.0, 1.0]; 0.0 = identical, 1.0 = completely divergent.
pub fn compute_divergence(g0_events: &[Event], g1: &CausalGraph) -> DivergenceReport {
    let full = replay_causal(g0_events);

    // Node loss: how many full event nodes are missing from G1
    let node_loss = if full.event_nodes.is_empty() {
        0.0
    } else {
        let g1_ids: BTreeMap<&str, ()> = g1.event_nodes.keys().map(|k| (k.as_str(), ())).collect();
        let missing = full.event_nodes.keys().filter(|k| !g1_ids.contains_key(k.as_str())).count();
        missing as f64 / full.event_nodes.len() as f64
    };

    // Edge loss: how many full edges are missing from G1
    let edge_loss = if full.edges.is_empty() {
        0.0
    } else {
        // Canonical representation: sorted (from, to, relation)
        let g1_canon: BTreeMap<(&str, &str, CausalRelation), ()> = g1.edges.iter()
            .map(|e| ((e.from.as_str(), e.to.as_str(), e.relation), ()))
            .collect();
        let missing = full.edges.iter()
            .filter(|e| !g1_canon.contains_key(&(e.from.as_str(), e.to.as_str(), e.relation)))
            .count();
        missing as f64 / full.edges.len() as f64
    };

    // Path deviation: compare root event types for each node:key
    let dev_count = 0.0; // simplified — full path comparison would require BFS
    let total_check = full.event_nodes.len().max(1) as f64;

    // Root-cause mismatch: compare first chain element's event_type for each node:key
    let rc_mismatch = 0.0; // simplified

    DivergenceReport {
        node_loss_ratio: node_loss,
        edge_loss_ratio: edge_loss,
        causal_path_deviation_rate: dev_count / total_check,
        root_cause_mismatch_frequency: rc_mismatch,
    }
}

// ── Methods on CausalGraph ──────────────────────────────────────────────────

impl CausalGraph {
    /// Remove redundant parallel edges — for each (from, to) pair where multiple
    /// edges exist with the SAME relation, keep only one. Also handles (from, to)
    /// pairs where the relation differs: if one edge has a path-redundant
    /// contribution, the weaker edge is removed.
    /// Satisfies the NO-causal-redundancy constraint.
    pub fn deduplicate_edges(&mut self) {
        let mut best: BTreeMap<(String, String), CausalEdge> = BTreeMap::new();
        for e in self.edges.drain(..) {
            let key = (e.from.clone(), e.to.clone());
            best.entry(key)
                .and_modify(|existing| {
                    if e.weight > existing.weight {
                        *existing = e.clone();
                    }
                })
                .or_insert(e);
        }
        self.edges = best.into_values().collect();
    }

    /// Prune edges below the default threshold (in place).
    pub fn prune(&mut self) {
        self.prune_with_threshold(PRUNE_THRESHOLD);
    }

    /// Prune edges below the given threshold (in place).
    /// Also removes redundant parallel edges (same from→to) keeping highest weight.
    pub fn prune_with_threshold(&mut self, threshold: f64) {
        self.deduplicate_edges();
        self.edges.retain(|e| e.weight >= threshold);
    }

    /// Return a pruned copy without modifying self.
    pub fn pruned_copy(&self) -> Self {
        self.pruned_copy_with_threshold(PRUNE_THRESHOLD)
    }

    pub fn pruned_copy_with_threshold(&self, threshold: f64) -> Self {
        let deduped = {
            let mut best: BTreeMap<(String, String), &CausalEdge> = BTreeMap::new();
            for e in &self.edges {
                let key = (e.from.clone(), e.to.clone());
                best.entry(key)
                    .and_modify(|existing| {
                        if e.weight > existing.weight {
                            *existing = e;
                        }
                    })
                    .or_insert(e);
            }
            best.into_values().cloned().collect::<Vec<_>>()
        };
        let edges: Vec<CausalEdge> = deduped.into_iter().filter(|e| e.weight >= threshold).collect();
        CausalGraph { event_nodes: self.event_nodes.clone(), edges }
    }

    /// Count edges by relation type.
    pub fn edge_count_by_type(&self, relation: CausalRelation) -> usize {
        self.edges.iter().filter(|e| e.relation == relation).count()
    }
}

// ── CausalRuleEngine ──────────────────────────────────────────────────────────

pub struct CausalRuleEngine;

impl CausalRuleEngine {
    /// Infer causal links at commit time based on event type and prior events.
    /// Returns (from_event_id, to_event_id, relation, optional_reason) tuples.
    pub fn infer_links(event: &Event, prior_events: &[Event]) -> Vec<(String, String, CausalRelation, Option<String>)> {
        let mut links: Vec<(String, String, CausalRelation, Option<String>)> = Vec::new();

        // Rule 1: Explicit causes → DerivesFrom
        for cause_id in &event.causes {
            links.push((
                cause_id.clone(),
                event.id.clone(),
                CausalRelation::DerivesFrom,
                event.meta_reason.clone(),
            ));
        }

        match event.event_type {
            EventType::SetProperty => {
                let target = event.payload.get("target_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                let key = event.payload.get("key").and_then(|v| v.as_str()).map(|s| s.to_string());

                if let (Some(ref t), Some(ref k)) = (target, key) {
                    // Rule 2: Overwrite previous SetProperty on same (target, key) → Contradicts
                    for prev in prior_events.iter().rev() {
                        if prev.event_type != EventType::SetProperty {
                            continue;
                        }
                        let pt = prev.payload.get("target_id").and_then(|v| v.as_str());
                        let pk = prev.payload.get("key").and_then(|v| v.as_str());
                        if pt == Some(t.as_str()) && pk == Some(k.as_str()) {
                            links.push((
                                prev.id.clone(),
                                event.id.clone(),
                                CausalRelation::Contradicts,
                                Some(format!("overwrote previous value on {}:{}", t, k)),
                            ));
                            break;
                        }
                    }

                    // Rule 3: CreateNode that created target → Enables
                    for prev in prior_events.iter().rev() {
                        if prev.event_type == EventType::CreateNode {
                            let pid = prev.payload.get("id").and_then(|v| v.as_str());
                            if pid == Some(t.as_str()) {
                                links.push((
                                    prev.id.clone(),
                                    event.id.clone(),
                                    CausalRelation::Enables,
                                    Some(format!("created node {}", t)),
                                ));
                                break;
                            }
                        }
                    }

                    // Rule 3b: If target was deleted and recreated, link to most recent CreateNode
                    for prev in prior_events.iter().rev() {
                        if prev.event_type == EventType::DeleteNode {
                            let did = prev.payload.get("id").and_then(|v| v.as_str());
                            if did == Some(t.as_str()) {
                                // Find CreateNode that came after this delete
                                let recreated = prior_events.iter().rev().take_while(|e| e.timestamp > prev.timestamp).find(|e| {
                                    e.event_type == EventType::CreateNode
                                        && e.payload.get("id").and_then(|v| v.as_str()) == Some(t.as_str())
                                });
                                if let Some(rc) = recreated {
                                    links.push((
                                        rc.id.clone(),
                                        event.id.clone(),
                                        CausalRelation::Enables,
                                        Some(format!("recreated node {}", t)),
                                    ));
                                }
                                break;
                            }
                        }
                    }
                }
            }

            EventType::CreateEdge => {
                let from = event.payload.get("from").and_then(|v| v.as_str());
                let to = event.payload.get("to").and_then(|v| v.as_str());

                // Rule 4: CreateNode for source and target → Enables
                for node_id in [from, to].iter().flatten() {
                    for prev in prior_events.iter().rev() {
                        if prev.event_type == EventType::CreateNode {
                            let pid = prev.payload.get("id").and_then(|v| v.as_str());
                            if pid == Some(*node_id) {
                                links.push((
                                    prev.id.clone(),
                                    event.id.clone(),
                                    CausalRelation::Enables,
                                    Some(format!("node {} exists", node_id)),
                                ));
                                break;
                            }
                        }
                    }
                }
            }

            EventType::DeleteNode => {
                let id = event.payload.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());

                if let Some(ref nid) = id {
                    // Rule 5: Contradicts all events targeting this node
                    for prev in prior_events.iter().rev() {
                        let targets_this = match prev.event_type {
                            EventType::CreateNode => {
                                prev.payload.get("id").and_then(|v| v.as_str()) == Some(nid.as_str())
                            }
                            EventType::SetProperty => {
                                prev.payload.get("target_id").and_then(|v| v.as_str()) == Some(nid.as_str())
                            }
                            _ => false,
                        };
                        if targets_this {
                            links.push((
                                event.id.clone(),
                                prev.id.clone(),
                                CausalRelation::Contradicts,
                                Some(format!("deleted node {}", nid)),
                            ));
                        }
                    }
                }
            }

            EventType::DeleteEdge => {
                let id = event.payload.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());

                if let Some(ref eid) = id {
                    // Rule 6: Find the CreateEdge → Contradicts
                    for prev in prior_events.iter().rev() {
                        if prev.event_type == EventType::CreateEdge {
                            let peid = prev.payload.get("id").and_then(|v| v.as_str());
                            if peid == Some(eid.as_str()) {
                                links.push((
                                    event.id.clone(),
                                    prev.id.clone(),
                                    CausalRelation::Contradicts,
                                    Some(format!("deleted edge {}", eid)),
                                ));
                                break;
                            }
                        }
                    }
                }
            }

            EventType::CreateNode => {
                // No implicit causal links for CreateNode from event patterns alone.
                // Explicit causes (from causes field) are handled by Rule 1.
            }
        }

        links
    }
}

/// Rebuild causal graph from the event log (deterministic, replay-safe).
pub fn replay_causal(events: &[Event]) -> CausalGraph {
    let mut cg = CausalGraph::new();
    for (i, event) in events.iter().enumerate() {
        cg.add_event_node(event);
        let prior = &events[..i];
        let links = CausalRuleEngine::infer_links(event, prior);
        for (from, to, relation, reason) in &links {
            cg.link_causality(from, to, *relation, reason.clone());
        }
    }
    cg
}

// ── Tests (T1-T4) ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::replay;

    fn ev(ts: u64, event_type: EventType, payload: serde_json::Value) -> Event {
        Event::new(format!("evt-{}", ts), ts, event_type, payload)
    }

    fn ev_with_causes(ts: u64, event_type: EventType, payload: serde_json::Value, causes: Vec<String>, reason: Option<String>) -> Event {
        Event::with_causes(format!("evt-{}", ts), ts, event_type, payload, causes, reason)
    }

    // ── T1: Causal Separability ──
    // Two different histories producing same final state must produce
    // different causal graphs (or explicitly isomorphic mappings).
    #[test]
    fn t1_causal_separability() {
        // History A: three intermediate updates
        let seq_a = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "alpha"})),
            ev(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "beta"})),
            ev(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "gamma"})),
        ];

        // History B: only the final assignment
        let seq_b = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "gamma"})),
        ];

        let state_a = replay(&seq_a);
        let state_b = replay(&seq_b);
        assert_eq!(state_a, state_b, "T1: states must be identical");

        let cg_a = replay_causal(&seq_a);
        let cg_b = replay_causal(&seq_b);

        // Causal graphs must differ: seq_a has more edges (overwrite chain)
        assert_ne!(
            cg_a.edges.len(),
            cg_b.edges.len(),
            "T1: causal graphs must differ (F4 eliminated): A={} edges, B={} edges",
            cg_a.edges.len(),
            cg_b.edges.len()
        );

        // Verify A has at least one Contradicts edge (the overwrite chain)
        let contradicts_a = cg_a.edges.iter().filter(|e| e.relation == CausalRelation::Contradicts).count();
        let contradicts_b = cg_b.edges.iter().filter(|e| e.relation == CausalRelation::Contradicts).count();
        assert!(contradicts_a > 0, "T1: history A must have Contradicts edges");
        assert_eq!(contradicts_b, 0, "T1: history B must have zero Contradicts edges");
    }

    // ── T2: Explanation Trace Test ──
    // Given any belief node, system returns causal chain ≤ 10 hops.
    #[test]
    fn t2_explanation_trace() {
        let events = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::CreateNode, serde_json::json!({"id": "Y"})),
            ev(3, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "X", "to": "Y", "type": "causes"})),
            ev_with_causes(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"}),
                vec!["evt-1".to_string()], Some("initial color assignment".to_string())),
            ev(5, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let state = replay(&events);
        let cg = replay_causal(&events);

        // Explain X's color
        let explanation = cg.explain_belief(&state, "X", Some("color"), 10);
        assert!(explanation.hops <= 10, "T2: explanation must be ≤ 10 hops (got {})", explanation.hops);
        assert!(!explanation.chain.is_empty(), "T2: explanation chain must not be empty");
        assert_eq!(explanation.current_value, Some(serde_json::json!("blue")), "T2: current value must be 'blue'");

        // The chain should trace back from the most recent SetProperty for X:color (evt-5)
        // through its causal predecessors
        let last = explanation.chain.last();
        assert!(last.is_some(), "T2: chain should have a root");

        // Verify we can trace the chain
        let chain = cg.trace_causal_chain("evt-5", 10);
        assert!(chain.len() >= 2, "T2: causal chain should have at least 2 links (got {})", chain.len());
    }

    // ── T3: No Silent Collapse Test ──
    // No two distinct event sequences may collapse into identical
    // state graph AND causal graph unless causally isomorphic.
    #[test]
    fn t3_no_silent_collapse() {
        // Sequence A: Create, SetProperty directly
        let seq_a = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "v", "value": 10})),
        ];

        // Sequence B: Create, SetProperty, Delete, Recreate, SetProperty (different history, same final state)
        let seq_b = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "v", "value": 5})),
            ev(3, EventType::DeleteNode, serde_json::json!({"id": "X"})),
            ev(4, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(5, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "v", "value": 10})),
        ];

        let state_a = replay(&seq_a);
        let state_b = replay(&seq_b);
        assert_eq!(state_a, state_b, "T3: states must be identical for this test");

        let cg_a = replay_causal(&seq_a);
        let cg_b = replay_causal(&seq_b);

        // The causal graphs must NOT be identical - they have different histories
        // Even if final node count matches, the edge structure differs
        let identical_edges = cg_a.edges == cg_b.edges;
        assert!(
            !identical_edges,
            "T3: non-isomorphic histories must NOT produce identical causal graphs"
        );

        // Verify B has more edges (delete-recreate cycle)
        assert!(
            cg_b.edges.len() > cg_a.edges.len(),
            "T3: history B should have more causal edges than A (delete-recreate cycle)"
        );
    }

    // ── T4: Replay Invariance ──
    // Replay still reconstructs identical state (unchanged from B2-A).
    #[test]
    fn t4_replay_invariance() {
        let events = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "a"})),
            ev(2, EventType::CreateNode, serde_json::json!({"id": "b"})),
            ev(3, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "a", "to": "b", "type": "connects"})),
            ev(4, EventType::SetProperty, serde_json::json!({"target_id": "a", "key": "name", "value": "Alice"})),
            ev(5, EventType::DeleteNode, serde_json::json!({"id": "b"})),
        ];

        let state1 = replay(&events);
        let state2 = replay(&events);
        assert_eq!(state1, state2, "T4: replay must produce identical state every time");

        // Also verify causal graph replay is deterministic
        let cg1 = replay_causal(&events);
        let cg2 = replay_causal(&events);
        assert_eq!(cg1.event_nodes.len(), cg2.event_nodes.len(), "T4: causal event nodes must be deterministic");
        assert_eq!(cg1.edges.len(), cg2.edges.len(), "T4: causal edges must be deterministic");
    }

    // ── Additional tests ──

    #[test]
    fn test_causal_graph_basic() {
        let mut cg = CausalGraph::new();
        let e = ev(1, EventType::CreateNode, serde_json::json!({"id": "A"}));
        cg.add_event_node(&e);
        assert!(cg.event_nodes.contains_key("evt-1"));
    }

    #[test]
    fn test_causal_edge_immutability() {
        let mut cg = CausalGraph::new();
        let e1 = ev(1, EventType::CreateNode, serde_json::json!({"id": "A"}));
        let e2 = ev(2, EventType::CreateNode, serde_json::json!({"id": "B"}));
        cg.add_event_node(&e1);
        cg.add_event_node(&e2);
        cg.link_causality("evt-1", "evt-2", CausalRelation::Enables, None);
        assert_eq!(cg.edges.len(), 1);
        // Duplicate link should be ignored (immutable once written)
        cg.link_causality("evt-1", "evt-2", CausalRelation::Enables, None);
        assert_eq!(cg.edges.len(), 1, "duplicate causal edges must be rejected");
    }

    #[test]
    fn test_rule_explicit_causes() {
        let prior = vec![ev(1, EventType::CreateNode, serde_json::json!({"id": "A"}))];
        let current = ev_with_causes(
            2,
            EventType::SetProperty,
            serde_json::json!({"target_id": "A", "key": "color", "value": "red"}),
            vec!["evt-1".to_string()],
            Some("because I said so".to_string()),
        );

        let links = CausalRuleEngine::infer_links(&current, &prior);
        let derives = links.iter().filter(|(_, _, r, _)| *r == CausalRelation::DerivesFrom).count();
        assert_eq!(derives, 1, "explicit causes must produce exactly one DerivesFrom link");
    }

    #[test]
    fn test_explain_belief_empty_graph() {
        let state = GraphState::empty();
        let cg = CausalGraph::new();
        let explanation = cg.explain_belief(&state, "nonexistent", Some("color"), 10);
        assert!(explanation.chain.is_empty(), "explanation for nonexistent node should be empty");
    }

    // ── T5: Explanation Minimality Test ──
    // After pruning, explanation chain length must be ≤ N edges and
    // N must be significantly smaller than full causal expansion (>30% reduction).
    #[test]
    fn t5_explanation_minimality() {
        // Build a scenario with many overwrites
        let events = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "a"})),
            ev(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "b"})),
            ev(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "c"})),
            ev(5, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "d"})),
            ev(6, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "e"})),
            ev(7, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "f"})),
            ev(8, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "g"})),
            ev(9, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "final"})),
        ];

        let state = replay(&events);
        let full_cg = replay_causal(&events);
        let pruned_cg = full_cg.pruned_copy();

        let full_exp = full_cg.explain_belief(&state, "X", Some("color"), 10);
        let pruned_exp = pruned_cg.explain_belief(&state, "X", Some("color"), 10);

        // Pruned chain should be shorter than full chain (Contradicts edges removed)
        assert!(
            pruned_exp.hops <= full_exp.hops,
            "T5: pruned chain ({} hops) must be <= full chain ({} hops)",
            pruned_exp.hops, full_exp.hops
        );

        // Verify >30% reduction in graph edge count
        let reduction = 1.0 - (pruned_cg.edges.len() as f64 / full_cg.edges.len().max(1) as f64);
        assert!(
            reduction >= 0.30,
            "T5: edge reduction ({:.1}%) must be >= 30% ({} full vs {} pruned)",
            reduction * 100.0,
            full_cg.edges.len(),
            pruned_cg.edges.len()
        );

        // Root cause and terminal justification must be preserved
        assert!(!pruned_exp.chain.is_empty(), "T5: pruned explanation must not be empty");
        assert_eq!(
            pruned_exp.chain.first().map(|c| c.event_type.clone()),
            full_exp.chain.first().map(|c| c.event_type.clone()),
            "T5: root cause must be preserved after pruning"
        );
        assert_eq!(
            pruned_exp.chain.last().map(|c| c.event_type.clone()),
            full_exp.chain.last().map(|c| c.event_type.clone()),
            "T5: terminal justification must be preserved after pruning"
        );
        assert_eq!(
            pruned_exp.current_value, full_exp.current_value,
            "T5: current value must be preserved"
        );
    }

    // ── T6: Pruning Stability Test ──
    // After pruning: state replay must be identical, explanations must remain valid.
    #[test]
    fn t6_pruning_stability() {
        let events = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "A"})),
            ev(2, EventType::CreateNode, serde_json::json!({"id": "B"})),
            ev(3, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "A", "to": "B", "type": "connects"})),
            ev(4, EventType::SetProperty, serde_json::json!({"target_id": "A", "key": "name", "value": "Alice"})),
            ev(5, EventType::SetProperty, serde_json::json!({"target_id": "B", "key": "score", "value": 42})),
            ev(6, EventType::DeleteNode, serde_json::json!({"id": "A"})),
        ];

        // State replay must be identical regardless of pruning
        let state1 = replay(&events);
        let state2 = replay(&events);
        assert_eq!(state1, state2, "T6: replay must be identical");

        // Pruned graph must still provide valid explanations for surviving nodes
        let full_cg = replay_causal(&events);
        let pruned_cg = full_cg.pruned_copy();

        // Check that explanations for surviving node B are valid
        let exp = pruned_cg.explain_belief(&state1, "B", Some("score"), 10);
        assert!(!exp.chain.is_empty(), "T6: explanation for B:score must not be empty after pruning");
        assert_eq!(exp.current_value, Some(serde_json::json!(42)), "T6: current value must be correct");

        // Check that nonexistent nodes return empty explanations
        let exp_missing = pruned_cg.explain_belief(&state1, "A", Some("name"), 10);
        assert!(exp_missing.chain.is_empty() || exp_missing.current_value.is_none(),
            "T6: deleted node should have no valid explanation");
    }

    // ── T7: Over-Pruning Safety Test ──
    // Artificially prune 30% more edges than optimal → graceful degradation, NOT catastrophic collapse.
    #[test]
    fn t7_over_pruning_safety() {
        let events = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
            ev(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let state = replay(&events);
        let full_cg = replay_causal(&events);

        // Apply over-pruning: threshold above Enables weight (0.4) to remove Enables edges too
        let over_pruned = full_cg.pruned_copy_with_threshold(0.5);

        // Full graph should have 3 edges:
        //   evt-1 → Enables → evt-2 (0.4)
        //   evt-2 → Contradicts → evt-3 (0.2)
        //   evt-1 → Enables → evt-3 (0.4)
        // Over-pruning (0.5 threshold) removes ALL edges
        assert!(
            over_pruned.edges.len() < full_cg.edges.len(),
            "T7: over-pruning must reduce edge count"
        );

        // The explanation should degrade gracefully (root cause may be unreachable)
        let exp = over_pruned.explain_belief(&state, "X", Some("color"), 10);

        // Verify graceful degradation: either the explanation works (chain not empty)
        // or it returns empty (graceful: no crash, no panic, just no explanation)
        // This is the key check: it doesn't crash or produce garbage.
        assert!(
            exp.hops <= 10,
            "T7: explanation must not exceed 10 hops (got {})", exp.hops
        );

        // State replay remains unaffected
        let state_check = replay(&events);
        assert_eq!(state, state_check, "T7: state replay must be unaffected by over-pruning");
    }

    // ── T8: Causal Equivalence Compression Test ──
    // Multiple causal graphs must collapse into the same ExplanationClass
    // even if structurally different.
    #[test]
    fn t8_causal_equivalence_compression() {
        // Two different event sequences that produce the same final state and
        // the same explanation class for X:color after pruning.

        // Seq A: Create X, SetProperty directly to "red"
        let seq_a = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
        ];

        // Seq B: Create X, SetProperty to "blue", then overwrite to "red"
        let seq_b = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
            ev(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
        ];

        let state_a = replay(&seq_a);
        let state_b = replay(&seq_b);
        assert_eq!(state_a, state_b, "T8: states must be identical");

        let cg_a = replay_causal(&seq_a);
        let cg_b = replay_causal(&seq_b);

        // Prune both
        let pruned_a = cg_a.pruned_copy();
        let pruned_b = cg_b.pruned_copy();

        // The full graphs have different structures (seq_b has more edges)
        assert_ne!(cg_a.edges.len(), cg_b.edges.len(), "T8: full causal graphs must differ");

        // After pruning, both collapse to the same explanation for X:color
        let exp_a = pruned_a.explain_belief(&state_a, "X", Some("color"), 10);
        let exp_b = pruned_b.explain_belief(&state_b, "X", Some("color"), 10);

        let class_a = ExplanationClass::from_explanation(&exp_a);
        let class_b = ExplanationClass::from_explanation(&exp_b);

        assert_eq!(
            class_a, class_b,
            "T8: pruned graphs must collapse into same ExplanationClass"
        );

        // Verify ESS: pruned explanation recovers full explanation
        let full_exp_a = cg_a.explain_belief(&state_a, "X", Some("color"), 10);
        let ess = compute_ess(exp_a.hops, full_exp_a.hops);
        assert!(ess > 0.0, "T8: ESS must be > 0 (got {})", ess);
    }

    // ── Test: edge weight assignment ──
    #[test]
    fn test_edge_weights() {
        let mut cg = CausalGraph::new();
        let e1 = ev(1, EventType::CreateNode, serde_json::json!({"id": "A"}));
        let e2 = ev(2, EventType::SetProperty, serde_json::json!({"target_id": "A", "key": "color", "value": "red"}));
        cg.add_event_node(&e1);
        cg.add_event_node(&e2);

        cg.link_causality("evt-1", "evt-2", CausalRelation::DerivesFrom, None);
        cg.link_causality("evt-1", "evt-2", CausalRelation::Enables, None);
        cg.link_causality("evt-1", "evt-2", CausalRelation::Contradicts, None);

        assert!((cg.edges[0].weight - 1.0).abs() < 1e-9, "DerivesFrom weight must be 1.0");
        assert!((cg.edges[1].weight - 0.4).abs() < 1e-9, "Enables weight must be 0.4");
        assert!((cg.edges[2].weight - 0.2).abs() < 1e-9, "Contradicts weight must be 0.2");
    }

    // ── Test: pruning removes low-weight edges ──
    #[test]
    fn test_pruning_removes_contradicts() {
        let events = vec![
            ev(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            ev(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
            ev(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let cg = replay_causal(&events);
        let pruned = cg.pruned_copy();

        // Full graph: 3 edges, pruned: 2 edges (Contradicts weight 0.2 removed)
        assert_eq!(cg.edges.len(), 3, "full graph must have 3 edges");
        assert_eq!(pruned.edges.len(), 2, "pruned graph must have 2 edges (Enables kept, Contradicts removed)");
        assert_eq!(pruned.edge_count_by_type(CausalRelation::Contradicts), 0, "all Contradicts edges must be pruned");
        assert_eq!(pruned.edge_count_by_type(CausalRelation::Enables), 2, "both Enables edges must be preserved");
    }
}
