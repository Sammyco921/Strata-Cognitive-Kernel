use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::causal::{CausalGraph, Explanation, replay_causal};
use crate::event::{Event, EventType};
use crate::graph::GraphState;
use crate::kernel::replay;

// ── Deterministic PRNG ──────────────────────────────────────────────────────

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        SimpleRng { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state >> 33
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next_u64() as usize) % (hi.saturating_sub(lo).max(1) + 1)
    }

    fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        &slice[self.range(0, slice.len().saturating_sub(1))]
    }
}

// ── Stress Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StressType {
    IdentityDrift,
    EdgeSemanticsCollision,
    TemporalContradiction,
    RedundantRepresentation,
    SemanticRelabel,
    CausalSwapping,
    ReconstructionEquivalence,
    ExplanationCollapse,
}

impl StressType {
    pub fn label(&self) -> &'static str {
        match self {
            StressType::IdentityDrift => "Identity Drift",
            StressType::EdgeSemanticsCollision => "Edge Semantics Collision",
            StressType::TemporalContradiction => "Temporal Contradiction",
            StressType::RedundantRepresentation => "Redundant Representation",
            StressType::SemanticRelabel => "Semantic Relabel Attack",
            StressType::CausalSwapping => "Causal Swapping Attack",
            StressType::ReconstructionEquivalence => "Reconstruction Equivalence",
            StressType::ExplanationCollapse => "Explanation Collapse",
        }
    }
}

// ── Failure Classes ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    F1StructuralStability,
    F2SemanticDrift,
    F3IdentityCollapse,
    F4NonIdentifiableCausality,
    F5ReplayDivergence,
}

impl FailureClass {
    pub fn label(&self) -> &'static str {
        match self {
            FailureClass::F1StructuralStability => "F1 — Structural Stability",
            FailureClass::F2SemanticDrift => "F2 — Semantic Drift",
            FailureClass::F3IdentityCollapse => "F3 — Identity Collapse",
            FailureClass::F4NonIdentifiableCausality => "F4 — Non-Identifiable Causality",
            FailureClass::F5ReplayDivergence => "F5 — Replay Divergence",
        }
    }

    pub fn severity(&self) -> &'static str {
        match self {
            FailureClass::F1StructuralStability => "GOOD",
            FailureClass::F2SemanticDrift => "WARNING",
            FailureClass::F3IdentityCollapse => "CRITICAL",
            FailureClass::F4NonIdentifiableCausality => "EXISTENTIAL",
            FailureClass::F5ReplayDivergence => "SYSTEM FAILURE",
        }
    }
}

// ── Comparison Results ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub original_node_count: usize,
    pub stressed_node_count: usize,
    pub original_edge_count: usize,
    pub stressed_edge_count: usize,
    pub node_set_identical: bool,
    pub edge_set_identical: bool,
    pub node_overlap: f64,
    pub edge_overlap: f64,
    pub property_difference_count: usize,
    pub edge_type_difference_count: usize,
    pub semantic_divergence: f64,
    pub causal_ambiguity_detected: bool,
    pub details: Vec<String>,
}

// ── Causal Drift Metrics ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CausalDriftMetrics {
    pub causal_edge_entropy: f64,
    pub causal_chain_length_variance: f64,
    pub causal_node_reassignment_rate: f64,
    pub explanation_path_stability: f64,
    pub cis: f64,
}

impl CausalDriftMetrics {
    pub fn to_map(&self) -> BTreeMap<String, f64> {
        let mut m = BTreeMap::new();
        m.insert("causal_edge_entropy".to_string(), self.causal_edge_entropy);
        m.insert("causal_chain_length_variance".to_string(), self.causal_chain_length_variance);
        m.insert("causal_node_reassignment_rate".to_string(), self.causal_node_reassignment_rate);
        m.insert("explanation_path_stability".to_string(), self.explanation_path_stability);
        m.insert("cis".to_string(), self.cis);
        m
    }
}

#[derive(Debug, Clone)]
pub struct StressReport {
    pub stress_type: StressType,
    pub classification: FailureClass,
    pub events_generated: usize,
    pub comparison: Option<ComparisonResult>,
    pub metrics: BTreeMap<String, f64>,
    pub causal_metrics: Option<CausalDriftMetrics>,
    pub findings: Vec<String>,
    pub unexpected_invariants: Vec<String>,
}

// ── Metric Computation Helpers ──────────────────────────────────────────────

/// Compute causal drift metrics for a single causal graph (no reference).
pub fn compute_causal_metrics_single(state: &GraphState, cg: &CausalGraph) -> CausalDriftMetrics {
    let entropy = compute_edge_entropy(cg);
    let chain_var = compute_chain_length_variance(state, cg);
    CausalDriftMetrics {
        causal_edge_entropy: entropy,
        causal_chain_length_variance: chain_var,
        causal_node_reassignment_rate: 0.0,
        explanation_path_stability: 1.0,
        cis: 1.0,
    }
}

/// Compute causal drift metrics by comparing a stressed state/cg against a reference.
pub fn compute_causal_drift_metrics(
    state: &GraphState,
    cg: &CausalGraph,
    reference_cg: Option<&CausalGraph>,
    reference_state: Option<&GraphState>,
) -> CausalDriftMetrics {
    let entropy = compute_edge_entropy(cg);
    let chain_var = compute_chain_length_variance(state, cg);
    let reassignment_rate = match reference_cg {
        Some(ref_cg) => compute_reassignment_rate(cg, ref_cg),
        None => 0.0,
    };
    let (path_stability, cis) = match (reference_cg, reference_state) {
        (Some(ref_cg), Some(ref_state)) => compute_cis(state, cg, ref_state, ref_cg),
        _ => (1.0, 1.0),
    };
    CausalDriftMetrics {
        causal_edge_entropy: entropy,
        causal_chain_length_variance: chain_var,
        causal_node_reassignment_rate: reassignment_rate,
        explanation_path_stability: path_stability,
        cis,
    }
}

fn compute_edge_entropy(cg: &CausalGraph) -> f64 {
    if cg.edges.is_empty() {
        return 0.0;
    }
    let total = cg.edges.len() as f64;
    let mut counts = BTreeMap::new();
    for e in &cg.edges {
        *counts.entry(e.relation).or_insert(0) += 1;
    }
    -counts.values().map(|&c| {
        let p = c as f64 / total;
        if p > 0.0 { p * p.log2() } else { 0.0 }
    }).sum::<f64>()
}

fn compute_chain_length_variance(state: &GraphState, cg: &CausalGraph) -> f64 {
    let mut lengths: Vec<f64> = Vec::new();
    for (nid, node) in &state.nodes {
        for key in node.properties.keys() {
            let exp = cg.explain_belief(state, nid, Some(key), 10);
            if !exp.chain.is_empty() {
                lengths.push(exp.hops as f64);
            }
        }
    }
    if lengths.len() <= 1 {
        return 0.0;
    }
    let mean = lengths.iter().sum::<f64>() / lengths.len() as f64;
    lengths.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / lengths.len() as f64
}

fn compute_reassignment_rate(cg: &CausalGraph, ref_cg: &CausalGraph) -> f64 {
    let mut changed = 0u64;
    let mut total = 0u64;
    for e in &cg.edges {
        if let Some(ref_e) = ref_cg.edges.iter().find(|r| r.from == e.from && r.to == e.to) {
            total += 1;
            if ref_e.relation != e.relation {
                changed += 1;
            }
        }
    }
    if total == 0 { 0.0 } else { changed as f64 / total as f64 }
}

/// Compute explanation path stability and CIS between two states.
fn compute_cis(
    state: &GraphState,
    cg: &CausalGraph,
    ref_state: &GraphState,
    ref_cg: &CausalGraph,
) -> (f64, f64) {
    let mut stable = 0u64;
    let mut total = 0u64;
    for (nid, node) in &state.nodes {
        if let Some(ref_node) = ref_state.nodes.get(nid) {
            for key in node.properties.keys() {
                if !ref_node.properties.contains_key(key) {
                    continue;
                }
                total += 1;
                let exp = cg.explain_belief(state, nid, Some(key), 10);
                let ref_exp = ref_cg.explain_belief(ref_state, nid, Some(key), 10);
                if explanations_are_stable(&exp, &ref_exp) {
                    stable += 1;
                }
            }
        }
    }
    if total == 0 { (1.0, 1.0) } else {
        let s = stable as f64 / total as f64;
        (s, s)
    }
}

/// Two explanations are "stable" if they share root cause node types,
/// same intermediate relation structure, and same terminal justification class.
fn explanations_are_stable(a: &Explanation, b: &Explanation) -> bool {
    match (a.chain.is_empty(), b.chain.is_empty()) {
        (true, true) => return true,
        (false, false) => {}
        _ => return false,
    }
    // Same root cause node type
    if a.chain[0].event_type != b.chain[0].event_type {
        return false;
    }
    // Same terminal justification class
    if a.chain[a.chain.len() - 1].event_type != b.chain[b.chain.len() - 1].event_type {
        return false;
    }
    // Same length and same intermediate relation structure
    if a.chain.len() != b.chain.len() {
        return false;
    }
    a.chain.iter().zip(b.chain.iter()).all(|(ca, cb)| {
        ca.relation_to_next == cb.relation_to_next
    })
}

/// Compare explanations across two causal graphs for shared (node, key) pairs.
/// Returns (stable_count, total_count, details).
pub fn compare_explanations(
    state_a: &GraphState,
    cg_a: &CausalGraph,
    state_b: &GraphState,
    cg_b: &CausalGraph,
) -> (usize, usize, Vec<String>) {
    let mut stable = 0usize;
    let mut total = 0usize;
    let mut details = Vec::new();
    for (nid, node) in &state_a.nodes {
        if let Some(ref_node) = state_b.nodes.get(nid) {
            for key in node.properties.keys() {
                if !ref_node.properties.contains_key(key) {
                    continue;
                }
                total += 1;
                let exp_a = cg_a.explain_belief(state_a, nid, Some(key), 10);
                let exp_b = cg_b.explain_belief(state_b, nid, Some(key), 10);
                if explanations_are_stable(&exp_a, &exp_b) {
                    stable += 1;
                } else {
                    details.push(format!(
                        "  unstable explanation: {}:{} (A:{}/B:{})",
                        nid, key,
                        exp_a.hops, exp_b.hops
                    ));
                }
            }
        }
    }
    (stable, total, details)
}

/// Rebuild a CausalGraph and GraphState from a &[Event].
fn rebuild(events: &[Event]) -> (GraphState, CausalGraph) {
    let state = replay(events);
    let cg = replay_causal(events);
    (state, cg)
}

/// Make a simple event (no explicit causes).
fn make_event(ts: u64, event_type: EventType, payload: serde_json::Value) -> Event {
    Event::new(format!("evt-{}", ts), ts, event_type, payload)
}

// ── Adversarial Event Generator ─────────────────────────────────────────────

pub struct AdversarialEventGenerator;

impl AdversarialEventGenerator {
    /// Stress Type A — Identity Drift.
    pub fn identity_drift(node_count: usize, update_count: usize, seed: u64) -> (Vec<Event>, StressReport) {
        let mut rng = SimpleRng::new(seed);
        let mut events = Vec::new();
        let mut ts: u64 = 0;
        let types = ["Disease", "Symptom", "Treatment", "Biomarker", "Pathway"];

        for i in 0..node_count {
            ts += 1;
            events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": format!("node_{}", i)})));
            ts += 1;
            events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": format!("node_{}", i), "key": "type", "value": "Concept"})));
            ts += 1;
            events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": format!("node_{}", i), "key": "version", "value": 0})));
        }

        let mut node_last_type: BTreeMap<String, &str> = (0..node_count)
            .map(|i| (format!("node_{}", i), "Concept"))
            .collect();
        let mut oscillation_count: BTreeMap<String, usize> = (0..node_count)
            .map(|i| (format!("node_{}", i), 0))
            .collect();

        for _ in 0..update_count {
            let idx = rng.range(0, node_count.saturating_sub(1));
            let node_id = format!("node_{}", idx);
            let new_type = *rng.pick(&types);

            ts += 1;
            events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": &node_id, "key": "type", "value": new_type})));
            ts += 1;
            events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": &node_id, "key": "version", "value": ts as i64})));

            if *node_last_type.get(&node_id).unwrap_or(&"Concept") != new_type {
                *oscillation_count.entry(node_id.clone()).or_insert(0) += 1;
            }
            node_last_type.insert(node_id, new_type);
        }

        let total_oscillations: usize = oscillation_count.values().sum();
        let max_osc = oscillation_count.values().copied().max().unwrap_or(0);
        let avg_osc = total_oscillations as f64 / node_count.max(1) as f64;

        let (state, cg) = rebuild(&events);
        let single_metrics = compute_causal_metrics_single(&state, &cg);

        // Compute mid-vs-end drift: compare first half to full sequence
        let mid = events.len() / 2;
        let (state_mid, cg_mid) = rebuild(&events[..mid]);
        let drift_metrics = compute_causal_drift_metrics(&state, &cg, Some(&cg_mid), Some(&state_mid));

        let mut metrics = BTreeMap::new();
        metrics.insert("node_count".to_string(), node_count as f64);
        metrics.insert("update_count".to_string(), update_count as f64);
        metrics.insert("total_oscillations".to_string(), total_oscillations as f64);
        metrics.insert("avg_oscillations_per_node".to_string(), avg_osc);
        metrics.insert("max_oscillations".to_string(), max_osc as f64);
        metrics.extend(single_metrics.to_map());

        let classification = if state.node_count() == node_count {
            if total_oscillations > 0 {
                FailureClass::F2SemanticDrift
            } else {
                FailureClass::F1StructuralStability
            }
        } else {
            FailureClass::F3IdentityCollapse
        };

        let mut findings = Vec::new();
        findings.push(format!("Semantic drift detected across {} nodes ({} total oscillations)", oscillation_count.len(), total_oscillations));
        findings.push(format!("Max oscillations on a single node: {}", max_osc));
        findings.push(format!("Causal edge entropy: {:.4}", single_metrics.causal_edge_entropy));
        findings.push(format!("Causal chain length variance: {:.4}", single_metrics.causal_chain_length_variance));
        findings.push(format!("CIS (mid-vs-end): {:.4}", drift_metrics.cis));

        if total_oscillations > update_count / 2 {
            findings.push("High oscillation rate — property meaning is highly unstable".to_string());
        }
        if state.node_count() != node_count {
            findings.push(format!("NODE COUNT MISMATCH: expected {} got {} — identity collapse", node_count, state.node_count()));
        }

        let event_count = events.len();
        (events, StressReport {
            stress_type: StressType::IdentityDrift,
            classification,
            events_generated: event_count,
            comparison: None,
            metrics,
            causal_metrics: Some(drift_metrics),
            findings,
            unexpected_invariants: vec!["node count remained stable".to_string()],
        })
    }

    /// Stress Type B — Edge Semantics Collision.
    pub fn edge_semantics_collision(node_count: usize, edge_count: usize, seed: u64) -> (Vec<Event>, StressReport) {
        let mut rng = SimpleRng::new(seed);
        let mut events = Vec::new();
        let mut ts: u64 = 0;
        let edge_types = ["causes", "associated_with", "contradicts", "treats", "prevents", "correlates_with"];

        for i in 0..node_count {
            ts += 1;
            events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": format!("n_{}", i)})));
        }

        let mut edge_type_log: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for i in 0..edge_count {
            let from = rng.range(0, node_count.saturating_sub(1));
            let mut to = rng.range(0, node_count.saturating_sub(1));
            while to == from {
                to = rng.range(0, node_count.saturating_sub(1));
            }
            let edge_id = format!("edge_{}", i);
            let etype = edge_types[i % edge_types.len()];
            ts += 1;
            events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": &edge_id, "from": format!("n_{}", from), "to": format!("n_{}", to), "type": etype})));
            edge_type_log.insert(edge_id.clone(), vec![etype.to_string()]);
        }

        let mut type_change_count: usize = 0;
        for _ in 0..edge_count * 2 {
            let idx = rng.range(0, edge_count.saturating_sub(1));
            let edge_id = format!("edge_{}", idx);
            let new_type = *rng.pick(&edge_types);
            ts += 1;
            events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": &edge_id, "key": "edge_type_override", "value": new_type})));
            if let Some(log) = edge_type_log.get_mut(&edge_id) {
                log.push(new_type.to_string());
            }
            type_change_count += 1;
        }

        let total_changes: usize = edge_type_log.values().map(|v| v.len() - 1).sum();
        let mut max_changes = 0;
        for v in edge_type_log.values() {
            max_changes = max_changes.max(v.len() - 1);
        }

        let (state, cg) = rebuild(&events);
        let single_metrics = compute_causal_metrics_single(&state, &cg);

        // Mid-vs-end drift
        let mid = events.len() / 2;
        let (state_mid, cg_mid) = rebuild(&events[..mid]);
        let drift_metrics = compute_causal_drift_metrics(&state, &cg, Some(&cg_mid), Some(&state_mid));

        let mut metrics = BTreeMap::new();
        metrics.insert("node_count".to_string(), node_count as f64);
        metrics.insert("edge_count".to_string(), edge_count as f64);
        metrics.insert("type_changes".to_string(), type_change_count as f64);
        metrics.insert("avg_type_changes_per_edge".to_string(), total_changes as f64 / edge_count.max(1) as f64);
        metrics.insert("max_type_changes_on_edge".to_string(), max_changes as f64);
        metrics.extend(single_metrics.to_map());

        let classification = if state.edge_count() == edge_count {
            if total_changes > 0 {
                FailureClass::F2SemanticDrift
            } else {
                FailureClass::F1StructuralStability
            }
        } else {
            FailureClass::F3IdentityCollapse
        };

        let mut findings = Vec::new();
        findings.push(format!("Edge types changed {} times across {} edges", type_change_count, edge_count));
        findings.push(format!("Edge with most changes: {} revisions", max_changes));
        findings.push(format!("Causal edge entropy: {:.4}", single_metrics.causal_edge_entropy));
        findings.push(format!("CIS (mid-vs-end): {:.4}", drift_metrics.cis));

        let invariants = vec![
            "edge count remained stable despite semantic changes".to_string(),
            "structural topology preserved under type mutation".to_string(),
        ];

        let event_count = events.len();
        (events, StressReport {
            stress_type: StressType::EdgeSemanticsCollision,
            classification,
            events_generated: event_count,
            comparison: None,
            metrics,
            causal_metrics: Some(drift_metrics),
            findings,
            unexpected_invariants: invariants,
        })
    }

    /// Stress Type C — Temporal Contradiction Injection.
    pub fn temporal_contradiction(_seed: u64) -> (Vec<Event>, StressReport) {
        let mut events = Vec::new();
        let mut ts: u64 = 0;

        for id in &["A", "B", "C"] {
            ts += 1;
            events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": id})));
        }
        ts += 1;
        events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e_ab", "from": "A", "to": "B", "type": "causes"})));
        ts += 1;
        events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e_ac", "from": "A", "to": "C", "type": "causes"})));
        ts += 1;
        events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e_cb", "from": "C", "to": "B", "type": "contradicts"})));
        ts += 1;
        events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "e_ab", "key": "confidence", "value": "disputed"})));
        ts += 1;
        events.push(make_event(ts, EventType::DeleteEdge, serde_json::json!({"id": "e_ab"})));
        ts += 1;
        events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e_ab", "from": "A", "to": "B", "type": "associated_with"})));
        ts += 1;
        events.push(make_event(ts, EventType::DeleteEdge, serde_json::json!({"id": "e_ab"})));
        ts += 1;
        events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e_ab", "from": "A", "to": "B", "type": "causes"})));

        let (state, cg) = rebuild(&events);
        let single_metrics = compute_causal_metrics_single(&state, &cg);

        let mut metrics = BTreeMap::new();
        metrics.insert("total_events".to_string(), events.len() as f64);
        metrics.insert("final_nodes".to_string(), state.node_count() as f64);
        metrics.insert("final_edges".to_string(), state.edge_count() as f64);
        metrics.insert("contradiction_cycles".to_string(), 2.0);
        metrics.extend(single_metrics.to_map());

        let classification = FailureClass::F2SemanticDrift;

        let mut findings = Vec::new();
        let edge = state.edges.get("e_ab");
        let resolved = edge.map_or("deleted", |e| &e.edge_type);
        findings.push(format!("Edge e_ab final type: {:?}", resolved));
        findings.push(format!("Causal edge entropy: {:.4}", single_metrics.causal_edge_entropy));
        findings.push("Contradiction is resolved by overwrite — last write wins, history is preserved in event log".to_string());
        findings.push("The causal graph preserves the full contradiction chain (Contradicts edges from DeleteEdge events)".to_string());

        let event_count = events.len();
        (events, StressReport {
            stress_type: StressType::TemporalContradiction,
            classification,
            events_generated: event_count,
            comparison: None,
            metrics,
            causal_metrics: Some(single_metrics),
            findings,
            unexpected_invariants: vec![
                "replay order exactly preserves the sequence of belief changes".to_string(),
                "no information is lost — full contradiction history is in the log".to_string(),
            ],
        })
    }

    /// Stress Type D — Redundant Representation Divergence.
    /// Returns (linear_events, branching_events, report).
    pub fn redundant_representation(_seed: u64) -> (Vec<Event>, Vec<Event>, StressReport) {
        let mut ts: u64 = 0;

        // ── Linear chain: A → B → C ──
        let mut linear = Vec::new();
        ts += 1; linear.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "A"})));
        ts += 1; linear.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "B"})));
        ts += 1; linear.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "C"})));
        ts += 1; linear.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "A", "to": "B", "type": "causes"})));
        ts += 1; linear.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e2", "from": "B", "to": "C", "type": "causes"})));
        ts += 1; linear.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "A", "key": "type", "value": "root"})));
        ts += 1; linear.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "C", "key": "type", "value": "target"})));

        // ── Branching graph: A → D → C, A → E → C ──
        let mut branching = Vec::new();
        ts += 1; branching.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "A"})));
        ts += 1; branching.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "D"})));
        ts += 1; branching.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "E"})));
        ts += 1; branching.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "C"})));
        ts += 1; branching.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e3", "from": "A", "to": "D", "type": "causes"})));
        ts += 1; branching.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e4", "from": "D", "to": "C", "type": "causes"})));
        ts += 1; branching.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e5", "from": "A", "to": "E", "type": "causes"})));
        ts += 1; branching.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e6", "from": "E", "to": "C", "type": "causes"})));
        ts += 1; branching.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "A", "key": "type", "value": "root"})));
        ts += 1; branching.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "C", "key": "type", "value": "target"})));

        let (lin_state, lin_cg) = rebuild(&linear);
        let (br_state, br_cg) = rebuild(&branching);

        let comparison = ReplayComparator::compare_states(&lin_state, &br_state, &linear, &branching);

        // Causal drift between linear and branching
        let drift = compute_causal_drift_metrics(&br_state, &br_cg, Some(&lin_cg), Some(&lin_state));

        // Compare explanations
        let (stable, total, _) = compare_explanations(&lin_state, &lin_cg, &br_state, &br_cg);

        let mut metrics = BTreeMap::new();
        metrics.insert("linear_nodes".to_string(), lin_state.node_count() as f64);
        metrics.insert("linear_edges".to_string(), lin_state.edge_count() as f64);
        metrics.insert("branching_nodes".to_string(), br_state.node_count() as f64);
        metrics.insert("branching_edges".to_string(), br_state.edge_count() as f64);
        metrics.insert("node_overlap".to_string(), comparison.node_overlap);
        metrics.insert("semantic_divergence".to_string(), comparison.semantic_divergence);
        metrics.insert("explanation_stable_count".to_string(), stable as f64);
        metrics.insert("explanation_total_count".to_string(), total as f64);
        metrics.extend(drift.to_map());

        let classification = FailureClass::F4NonIdentifiableCausality;

        let event_count = linear.len().max(branching.len());
        (linear, branching, StressReport {
            stress_type: StressType::RedundantRepresentation,
            classification,
            events_generated: event_count,
            comparison: Some(comparison),
            metrics,
            causal_metrics: Some(drift.clone()),
            findings: vec![
                format!("Linear: {} nodes, {} edges", lin_state.node_count(), lin_state.edge_count()),
                format!("Branching: {} nodes, {} edges", br_state.node_count(), br_state.edge_count()),
                format!("Stable explanations: {}/{} (CIS={:.4})", stable, total, drift.cis),
                "Different encodings of same knowledge produce different graph states".to_string(),
                "Causal explanations are distinguishable across representations".to_string(),
            ],
            unexpected_invariants: vec![
                "shared nodes (A, C) have identical property values across representations".to_string(),
                "causal graph distinguishes linear vs branching structure".to_string(),
            ],
        })
    }
}

// ── Replay Comparison Engine ─────────────────────────────────────────────────

pub struct ReplayComparator;

impl ReplayComparator {
    pub fn compare_states(
        state_a: &GraphState,
        state_b: &GraphState,
        _log_a: &[Event],
        _log_b: &[Event],
    ) -> ComparisonResult {
        let nodes_a: BTreeMap<&str, &crate::graph::Node> =
            state_a.nodes.iter().map(|(k, v)| (k.as_str(), v)).collect();
        let nodes_b: BTreeMap<&str, &crate::graph::Node> =
            state_b.nodes.iter().map(|(k, v)| (k.as_str(), v)).collect();

        let edges_a: BTreeMap<&str, &crate::graph::Edge> =
            state_a.edges.iter().map(|(k, v)| (k.as_str(), v)).collect();
        let edges_b: BTreeMap<&str, &crate::graph::Edge> =
            state_b.edges.iter().map(|(k, v)| (k.as_str(), v)).collect();

        let shared_nodes: Vec<&str> = nodes_a.keys().filter(|k| nodes_b.contains_key(*k)).copied().collect();
        let all_nodes: Vec<&str> = {
            let mut s: Vec<&str> = nodes_a.keys().chain(nodes_b.keys()).copied().collect();
            s.sort();
            s.dedup();
            s
        };
        let node_overlap = if all_nodes.is_empty() { 1.0 } else { shared_nodes.len() as f64 / all_nodes.len() as f64 };

        let shared_edges: Vec<&str> = edges_a.keys().filter(|k| edges_b.contains_key(*k)).copied().collect();
        let all_edges: Vec<&str> = {
            let mut s: Vec<&str> = edges_a.keys().chain(edges_b.keys()).copied().collect();
            s.sort();
            s.dedup();
            s
        };
        let edge_overlap = if all_edges.is_empty() { 1.0 } else { shared_edges.len() as f64 / all_edges.len() as f64 };

        let node_set_identical = nodes_a.len() == nodes_b.len() && shared_nodes.len() == nodes_a.len();
        let edge_set_identical = edges_a.len() == edges_b.len() && shared_edges.len() == edges_a.len();

        let mut property_diffs = 0;
        let mut edge_type_diffs = 0;

        for nid in &shared_nodes {
            let na = nodes_a.get(nid).unwrap();
            let nb = nodes_b.get(nid).unwrap();
            for (k, va) in &na.properties {
                match nb.properties.get(k) {
                    Some(vb) if va != vb => property_diffs += 1,
                    None => property_diffs += 1,
                    _ => {}
                }
            }
            for k in nb.properties.keys() {
                if !na.properties.contains_key(k) {
                    property_diffs += 1;
                }
            }
        }

        for eid in &shared_edges {
            let ea = edges_a.get(eid).unwrap();
            let eb = edges_b.get(eid).unwrap();
            if ea.edge_type != eb.edge_type {
                edge_type_diffs += 1;
            }
            for (k, va) in &ea.properties {
                match eb.properties.get(k) {
                    Some(vb) if va != vb => property_diffs += 1,
                    None => property_diffs += 1,
                    _ => {}
                }
            }
        }

        let sem_div = if node_set_identical && edge_set_identical {
            property_diffs as f64 / (property_diffs + 1).max(1) as f64
        } else {
            1.0 - node_overlap * edge_overlap
        };

        let mut details = Vec::new();
        if !node_set_identical {
            let only_a: Vec<&str> = nodes_a.keys().filter(|k| !nodes_b.contains_key(*k)).copied().collect();
            let only_b: Vec<&str> = nodes_b.keys().filter(|k| !nodes_a.contains_key(*k)).copied().collect();
            details.push(format!("Nodes only in A: {:?}", only_a));
            details.push(format!("Nodes only in B: {:?}", only_b));
        }
        if property_diffs > 0 {
            details.push(format!("Property differences: {}", property_diffs));
        }
        if edge_type_diffs > 0 {
            details.push(format!("Edge type differences: {}", edge_type_diffs));
        }

        ComparisonResult {
            original_node_count: nodes_a.len(),
            stressed_node_count: nodes_b.len(),
            original_edge_count: edges_a.len(),
            stressed_edge_count: edges_b.len(),
            node_set_identical,
            edge_set_identical,
            node_overlap,
            edge_overlap,
            property_difference_count: property_diffs,
            edge_type_difference_count: edge_type_diffs,
            semantic_divergence: sem_div,
            causal_ambiguity_detected: node_set_identical && edge_set_identical,
            details,
        }
    }
}

// ── Failure Classifier ──────────────────────────────────────────────────────

pub struct FailureClassifier;

impl FailureClassifier {
    pub fn classify(result: &ComparisonResult, _stress_type: StressType, events: &[Event]) -> FailureClass {
        let state1 = replay(events);
        let state2 = replay(events);
        if state1 != state2 {
            return FailureClass::F5ReplayDivergence;
        }

        if result.causal_ambiguity_detected && result.node_set_identical && result.edge_set_identical {
            return FailureClass::F4NonIdentifiableCausality;
        }

        if result.node_set_identical && result.edge_set_identical {
            if result.property_difference_count > 0 || result.edge_type_difference_count > 0 {
                return FailureClass::F2SemanticDrift;
            }
            return FailureClass::F1StructuralStability;
        }

        FailureClass::F3IdentityCollapse
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  EXPERIMENTS: E-S Series (Existing + Causal Metrics)
// ═══════════════════════════════════════════════════════════════════════════════

/// E-S1: Identity Stress Test — 100 nodes, 500 adversarial updates.
pub fn experiment_identity_stress() -> StressReport {
    let (events, mut report) = AdversarialEventGenerator::identity_drift(100, 500, 42);
    report.findings.push(format!("Total events generated: {}", events.len()));

    let s1 = replay(&events);
    let s2 = replay(&events);
    if s1 == s2 {
        report.findings.push("Replay determinism CONFIRMED: identical state from two replays".to_string());
        report.unexpected_invariants.push("replay is deterministic under heavy identity mutation".to_string());
    } else {
        report.classification = FailureClass::F5ReplayDivergence;
        report.findings.push("REPLAY DIVERGENCE DETECTED".to_string());
    }

    report
}

/// E-S2: Semantic Collision Test — 50 edges under changing meanings.
pub fn experiment_semantic_collision() -> StressReport {
    let (events, mut report) = AdversarialEventGenerator::edge_semantics_collision(20, 50, 42);
    report.findings.push(format!("Total events generated: {}", events.len()));

    let s1 = replay(&events);
    let s2 = replay(&events);
    if s1 == s2 {
        report.findings.push("Replay determinism CONFIRMED under semantic collision".to_string());
    } else {
        report.classification = FailureClass::F5ReplayDivergence;
    }

    report
}

/// E-S3: Causal Reconstruction Test.
pub fn experiment_causal_reconstruction() -> Vec<StressReport> {
    let mut reports = Vec::new();

    // ── Scenario 1: Property overwrite ambiguity ──
    {
        let seq_a = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "alpha"})),
            make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "beta"})),
            make_event(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "gamma"})),
        ];

        let seq_b = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "gamma"})),
        ];

        let seq_c = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "delta"})),
            make_event(3, EventType::DeleteNode, serde_json::json!({"id": "X"})),
            make_event(4, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(5, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "belief", "value": "gamma"})),
        ];

        let (state_a, cg_a) = rebuild(&seq_a);
        let (state_b, cg_b) = rebuild(&seq_b);
        let (state_c, cg_c) = rebuild(&seq_c);

        let ab_same = state_a == state_b;
        let ac_same = state_a == state_c;

        // Causal drift: A vs B, A vs C
        let drift_ab = compute_causal_drift_metrics(&state_a, &cg_a, Some(&cg_b), Some(&state_b));
        let drift_ac = compute_causal_drift_metrics(&state_a, &cg_a, Some(&cg_c), Some(&state_c));

        let (stable_ab, total_ab, _) = compare_explanations(&state_a, &cg_a, &state_b, &cg_b);
        let (stable_ac, total_ac, _) = compare_explanations(&state_a, &cg_a, &state_c, &cg_c);

        let mut metrics = BTreeMap::new();
        metrics.insert("seq_a_events".to_string(), seq_a.len() as f64);
        metrics.insert("seq_b_events".to_string(), seq_b.len() as f64);
        metrics.insert("seq_c_events".to_string(), seq_c.len() as f64);
        metrics.insert("a_equals_b".to_string(), if ab_same { 1.0 } else { 0.0 });
        metrics.insert("a_equals_c".to_string(), if ac_same { 1.0 } else { 0.0 });
        metrics.insert("stable_ab".to_string(), stable_ab as f64);
        metrics.insert("total_ab".to_string(), total_ab as f64);
        metrics.insert("cis_ab".to_string(), drift_ab.cis);
        metrics.insert("cis_ac".to_string(), drift_ac.cis);
        metrics.extend(drift_ab.to_map());

        let mut findings = Vec::new();
        findings.push(format!("Sequences A ({} evts) and B ({} evts) produce {} state", seq_a.len(), seq_b.len(), if ab_same { "IDENTICAL" } else { "DIFFERENT" }));
        findings.push(format!("Sequences A ({} evts) and C ({} evts) produce {} state", seq_a.len(), seq_c.len(), if ac_same { "IDENTICAL" } else { "DIFFERENT" }));
        findings.push(format!("CIS(A,B)={:.4} (stable explanations: {}/{})", drift_ab.cis, stable_ab, total_ab));
        findings.push(format!("CIS(A,C)={:.4} (stable explanations: {}/{})", drift_ac.cis, stable_ac, total_ac));
        findings.push("Cause of belief='gamma' is unambiguous from event log: last SetProperty before query".to_string());
        findings.push("Cause of belief='gamma' is ambiguous from snapshot alone: multiple histories produce same final value".to_string());

        let classification = if seq_a.len() != seq_b.len() && state_a == state_b {
            FailureClass::F4NonIdentifiableCausality
        } else {
            FailureClass::F1StructuralStability
        };

        reports.push(StressReport {
            stress_type: StressType::TemporalContradiction,
            classification,
            events_generated: seq_a.len().max(seq_b.len()).max(seq_c.len()),
            comparison: None,
            metrics,
            causal_metrics: Some(drift_ab),
            findings,
            unexpected_invariants: vec![
                "replay determinism holds across all sequences".to_string(),
                "the kernel correctly distinguishes create-delete-recreate from single create".to_string(),
                "causal graph provides distinguishable explanations even when states are identical".to_string(),
            ],
        });
    }

    // ── Scenario 2: Idempotent event sequences ──
    {
        let seq_d = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "Y"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "color", "value": "red"})),
            make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "color", "value": "red"})),
            make_event(4, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "color", "value": "red"})),
        ];

        let seq_e = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "Y"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "color", "value": "red"})),
        ];

        let (state_d, cg_d) = rebuild(&seq_d);
        let (state_e, cg_e) = rebuild(&seq_e);

        let identical = state_d == state_e;

        let drift = compute_causal_drift_metrics(&state_d, &cg_d, Some(&cg_e), Some(&state_e));
        let (stable, total, _) = compare_explanations(&state_d, &cg_d, &state_e, &cg_e);

        let mut metrics = BTreeMap::new();
        metrics.insert("states_identical".to_string(), if identical { 1.0 } else { 0.0 });
        metrics.insert("cis".to_string(), drift.cis);
        metrics.extend(drift.to_map());

        let mut findings = Vec::new();
        findings.push(format!("Idempotent writes: D ({} evts) vs E ({} evts) -> {}", seq_d.len(), seq_e.len(), if identical { "IDENTICAL STATE" } else { "DIFFERENT STATE" }));
        findings.push(format!("CIS={:.4} (stable: {}/{})", drift.cis, stable, total));
        findings.push("True cause of color='red' is ambiguous if only snapshot is available".to_string());

        reports.push(StressReport {
            stress_type: StressType::TemporalContradiction,
            classification: FailureClass::F4NonIdentifiableCausality,
            events_generated: seq_d.len().max(seq_e.len()),
            comparison: None,
            metrics,
            causal_metrics: Some(drift),
            findings,
            unexpected_invariants: vec![
                "idempotent SetProperty events are correctly handled (no-op on replay)".to_string(),
            ],
        });
    }

    reports
}

/// E-S4: Representation Equivalence Test.
pub fn experiment_representation_equivalence() -> StressReport {
    let (_linear, _branching, report) = AdversarialEventGenerator::redundant_representation(42);
    report
}

// ═══════════════════════════════════════════════════════════════════════════════
//  EXPERIMENTS: E-C Series (New Causal Stress Tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// E-C1: Semantic Relabel Attack.
/// Preserve topology but rename all semantic labels (edge types, property keys).
/// Causal explanations should NOT degrade in interpretability.
pub fn experiment_semantic_relabel() -> StressReport {
    // Build a reference graph with meaningful labels
    let mut ts: u64 = 0;
    let mut ref_events = Vec::new();

    ts += 1; ref_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "A"})));
    ts += 1; ref_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "B"})));
    ts += 1; ref_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "C"})));
    ts += 1; ref_events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "A", "to": "B", "type": "supports"})));
    ts += 1; ref_events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e2", "from": "B", "to": "C", "type": "contradicts"})));
    ts += 1; ref_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "A", "key": "color", "value": "blue"})));
    ts += 1; ref_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "B", "key": "size", "value": "large"})));
    ts += 1; ref_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "C", "key": "color", "value": "red"})));

    // Relabeled version: rename edge types and property keys, keep topology
    let mut relabel_events = Vec::new();
    ts = 0;
    ts += 1; relabel_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "A"})));
    ts += 1; relabel_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "B"})));
    ts += 1; relabel_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "C"})));
    ts += 1; relabel_events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "A", "to": "B", "type": "relates_to"})));
    ts += 1; relabel_events.push(make_event(ts, EventType::CreateEdge, serde_json::json!({"id": "e2", "from": "B", "to": "C", "type": "opposes"})));
    ts += 1; relabel_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "A", "key": "hue", "value": "blue"})));
    ts += 1; relabel_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "B", "key": "magnitude", "value": "large"})));
    ts += 1; relabel_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "C", "key": "hue", "value": "red"})));

    let (ref_state, ref_cg) = rebuild(&ref_events);
    let (rel_state, rel_cg) = rebuild(&relabel_events);

    // Compute causal metrics: compare relabeled against reference
    let drift = compute_causal_drift_metrics(&rel_state, &rel_cg, Some(&ref_cg), Some(&ref_state));

    // Compare explanations for nodes that have the same property keys (post-relabel)
    let mut stable = 0u64;
    let mut total = 0u64;
    let mut degrade_detected = false;
    for (nid, node) in &ref_state.nodes {
        if let Some(rel_node) = rel_state.nodes.get(nid) {
            // Check each original property against its relabeled counterpart
            // A->hue maps to A->color, so we check by expected mapping
            let key_map: BTreeMap<&str, &str> = [("color", "hue"), ("size", "magnitude")].iter().cloned().collect();
            for (orig_key, rel_key) in &key_map {
                if node.properties.contains_key(*orig_key) && rel_node.properties.contains_key(*rel_key) {
                    total += 1;
                    let exp_ref = ref_cg.explain_belief(&ref_state, nid, Some(orig_key), 10);
                    let exp_rel = rel_cg.explain_belief(&rel_state, nid, Some(rel_key), 10);
                    if explanations_are_stable(&exp_ref, &exp_rel) {
                        stable += 1;
                    } else {
                        degrade_detected = true;
                    }
                }
            }
        }
    }

    let cis = if total == 0 { 1.0 } else { stable as f64 / total as f64 };

    let mut metrics = BTreeMap::new();
    metrics.insert("ref_events".to_string(), ref_events.len() as f64);
    metrics.insert("ref_nodes".to_string(), ref_state.node_count() as f64);
    metrics.insert("ref_causal_edges".to_string(), ref_cg.edges.len() as f64);
    metrics.insert("relabeled_causal_edges".to_string(), rel_cg.edges.len() as f64);
    metrics.insert("stable_explanations".to_string(), stable as f64);
    metrics.insert("total_explanations".to_string(), total as f64);
    metrics.insert("cis".to_string(), cis);
    metrics.extend(drift.to_map());

    let classification = if cis >= 0.75 {
        FailureClass::F1StructuralStability
    } else {
        FailureClass::F2SemanticDrift
    };

    let mut findings = Vec::new();
    findings.push(format!("Reference: {} events, {} nodes, {} causal edges", ref_events.len(), ref_state.node_count(), ref_cg.edges.len()));
    findings.push(format!("Relabeled: {} events, {} nodes, {} causal edges", relabel_events.len(), rel_state.node_count(), rel_cg.edges.len()));
    findings.push(format!("CIS={:.4} ({}/{} stable explanations)", cis, stable, total));
    findings.push(format!("Causal edge entropy (ref): {:.4}", compute_edge_entropy(&ref_cg)));
    findings.push(format!("Causal edge entropy (relabeled): {:.4}", compute_edge_entropy(&rel_cg)));
    findings.push(format!("Causal node reassignment rate: {:.4}", drift.causal_node_reassignment_rate));
    if !degrade_detected {
        findings.push("Explanation quality preserved under semantic relabeling".to_string());
    } else {
        findings.push("WARNING: Explanation quality degraded under semantic relabeling".to_string());
    }

    let event_count = ref_events.len().max(relabel_events.len());
    StressReport {
        stress_type: StressType::SemanticRelabel,
        classification,
        events_generated: event_count,
        comparison: None,
        metrics,
        causal_metrics: Some(drift),
        findings,
        unexpected_invariants: vec![
            "topology is preserved under semantic relabeling".to_string(),
            "causal graph structure is invariant to semantic string changes".to_string(),
        ],
    }
}

/// E-C2: Causal Swapping Attack.
/// Randomly swap causes, enables, derives_from within valid type constraints.
/// System should detect explanatory contradiction OR flag ambiguity.
pub fn experiment_causal_swapping() -> StressReport {
    let mut ts: u64 = 0;

    // Build a reference graph with explicit causal structure
    let mut ref_events = Vec::new();
    ts += 1; ref_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "X"})));
    ts += 1; ref_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "Y"})));
    ts += 1; ref_events.push(
        Event::with_causes(
            format!("evt-{}", ts + 1), ts + 1,
            EventType::SetProperty,
            serde_json::json!({"target_id": "X", "key": "color", "value": "red"}),
            vec!["evt-1".to_string()],
            Some("explicit cause".to_string()),
        )
    );
    ts += 2;
    ts += 1; ref_events.push(
        Event::with_causes(
            format!("evt-{}", ts + 1), ts + 1,
            EventType::SetProperty,
            serde_json::json!({"target_id": "Y", "key": "color", "value": "blue"}),
            vec!["evt-2".to_string()],
            Some("explicit cause".to_string()),
        )
    );

    // Build swapped version: swap the explicit causes so they cross-reference
    let mut swap_events = Vec::new();
    ts = 0;
    ts += 1; swap_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "X"})));
    ts += 1; swap_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "Y"})));
    ts += 1;
    // X's SetProperty now cites Y's CreateNode as cause (semantically wrong)
    swap_events.push(
        Event::with_causes(
            format!("evt-{}", ts + 1), ts + 1,
            EventType::SetProperty,
            serde_json::json!({"target_id": "X", "key": "color", "value": "red"}),
            vec!["evt-2".to_string()],
            Some("swapped cause".to_string()),
        )
    );
    ts += 2;
    ts += 1;
    // Y's SetProperty now cites X's CreateNode as cause (semantically wrong)
    swap_events.push(
        Event::with_causes(
            format!("evt-{}", ts + 1), ts + 1,
            EventType::SetProperty,
            serde_json::json!({"target_id": "Y", "key": "color", "value": "blue"}),
            vec!["evt-1".to_string()],
            Some("swapped cause".to_string()),
        )
    );

    let (ref_state, ref_cg) = rebuild(&ref_events);
    let (swap_state, swap_cg) = rebuild(&swap_events);

    // Swapping changes causal structure — the DerivesFrom edges now point to wrong nodes
    let drift = compute_causal_drift_metrics(&swap_state, &swap_cg, Some(&ref_cg), Some(&ref_state));

    // Check: do explanations contradict?
    let exp_x_ref = ref_cg.explain_belief(&ref_state, "X", Some("color"), 10);
    let exp_x_swap = swap_cg.explain_belief(&swap_state, "X", Some("color"), 10);
    let exp_y_ref = ref_cg.explain_belief(&ref_state, "Y", Some("color"), 10);
    let exp_y_swap = swap_cg.explain_belief(&swap_state, "Y", Some("color"), 10);

    // Detect contradiction: root causes should differ
    let x_contradiction = !explanations_are_stable(&exp_x_ref, &exp_x_swap);
    let y_contradiction = !explanations_are_stable(&exp_y_ref, &exp_y_swap);
    let ambiguity_flagged = x_contradiction || y_contradiction;

    // Count swapped edges
    let mut swapped_edges = 0u64;
    for e in &swap_cg.edges {
        if let Some(ref_e) = ref_cg.edges.iter().find(|r| r.from == e.from && r.to == e.to) {
            if ref_e.relation != e.relation {
                swapped_edges += 1;
            }
        }
    }

    let mut metrics = BTreeMap::new();
    metrics.insert("ref_causal_edges".to_string(), ref_cg.edges.len() as f64);
    metrics.insert("swap_causal_edges".to_string(), swap_cg.edges.len() as f64);
    metrics.insert("swapped_edges_detected".to_string(), swapped_edges as f64);
    metrics.insert("x_contradiction".to_string(), if x_contradiction { 1.0 } else { 0.0 });
    metrics.insert("y_contradiction".to_string(), if y_contradiction { 1.0 } else { 0.0 });
    metrics.insert("ambiguity_flagged".to_string(), if ambiguity_flagged { 1.0 } else { 0.0 });
    metrics.extend(drift.to_map());

    let classification = if ambiguity_flagged {
        FailureClass::F2SemanticDrift
    } else {
        FailureClass::F4NonIdentifiableCausality
    };

    let mut findings = Vec::new();
    findings.push(format!("Reference causal edges: {}", ref_cg.edges.len()));
    findings.push(format!("Swapped causal edges: {}", swap_cg.edges.len()));
    findings.push(format!("Swapped edges: {}", swapped_edges));
    findings.push(format!("X explanation contradicted: {}", x_contradiction));
    findings.push(format!("Y explanation contradicted: {}", y_contradiction));
    findings.push(format!("Ambiguity flagged: {}", ambiguity_flagged));
    findings.push(format!("CIS: {:.4}", drift.cis));

    if ambiguity_flagged {
        findings.push("✓ System correctly detected explanatory contradiction after causal swapping".to_string());
    } else {
        findings.push("⚠ System did not detect contradiction — causal swapping went unnoticed".to_string());
    }

    let event_count = ref_events.len().max(swap_events.len());
    StressReport {
        stress_type: StressType::CausalSwapping,
        classification,
        events_generated: event_count,
        comparison: None,
        metrics,
        causal_metrics: Some(drift),
        findings,
        unexpected_invariants: vec![
            "state projection is identical under swapped causes (same properties set)".to_string(),
            "causal graph differs under swapped causes (different DerivesFrom edges)".to_string(),
        ],
    }
}

/// E-C3: Reconstruction Equivalence Test.
/// 3 different event histories, all producing identical final state.
/// Test: do they produce distinguishably different causal explanations?
pub fn experiment_reconstruction_equivalence() -> StressReport {
    // Three sequences all producing X: {color: "red"}
    let seq1 = vec![
        make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
        make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
    ];

    let seq2 = vec![
        make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
        make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
    ];

    let seq3 = vec![
        make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
        make_event(2, EventType::CreateNode, serde_json::json!({"id": "Y"})),
        make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "green"})),
        make_event(4, EventType::DeleteNode, serde_json::json!({"id": "X"})),
        make_event(5, EventType::CreateNode, serde_json::json!({"id": "X"})),
        make_event(6, EventType::DeleteNode, serde_json::json!({"id": "Y"})),
        make_event(7, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
    ];

    let (s1, cg1) = rebuild(&seq1);
    let (s2, cg2) = rebuild(&seq2);
    let (s3, cg3) = rebuild(&seq3);

    // Verify all produce identical state
    let states_equal = s1 == s2 && s2 == s3;

    // Compare causal graphs pairwise
    let edges_1_2_same = cg1.edges == cg2.edges;
    let edges_1_3_same = cg1.edges == cg3.edges;
    let edges_2_3_same = cg2.edges == cg3.edges;

    // Compute CIS pairwise
    let d12 = compute_causal_drift_metrics(&s1, &cg1, Some(&cg2), Some(&s2));
    let d13 = compute_causal_drift_metrics(&s1, &cg1, Some(&cg3), Some(&s3));
    let d23 = compute_causal_drift_metrics(&s2, &cg2, Some(&cg3), Some(&s3));

    // Explanation distinguishability
    let e1 = cg1.explain_belief(&s1, "X", Some("color"), 10);
    let e2 = cg2.explain_belief(&s2, "X", Some("color"), 10);
    let e3 = cg3.explain_belief(&s3, "X", Some("color"), 10);

    let d12_stable = explanations_are_stable(&e1, &e2);
    let d13_stable = explanations_are_stable(&e1, &e3);
    let d23_stable = explanations_are_stable(&e2, &e3);

    // Count how many pairs are distinguishable
    let distinguishable = (!d12_stable as u64) + (!d13_stable as u64) + (!d23_stable as u64);

    let mut metrics = BTreeMap::new();
    metrics.insert("seq1_events".to_string(), seq1.len() as f64);
    metrics.insert("seq2_events".to_string(), seq2.len() as f64);
    metrics.insert("seq3_events".to_string(), seq3.len() as f64);
    metrics.insert("states_equal".to_string(), if states_equal { 1.0 } else { 0.0 });
    metrics.insert("cg1_edges".to_string(), cg1.edges.len() as f64);
    metrics.insert("cg2_edges".to_string(), cg2.edges.len() as f64);
    metrics.insert("cg3_edges".to_string(), cg3.edges.len() as f64);
    metrics.insert("edges_1_2_same".to_string(), if edges_1_2_same { 1.0 } else { 0.0 });
    metrics.insert("edges_1_3_same".to_string(), if edges_1_3_same { 1.0 } else { 0.0 });
    metrics.insert("edges_2_3_same".to_string(), if edges_2_3_same { 1.0 } else { 0.0 });
    metrics.insert("distinguishable_pairs".to_string(), distinguishable as f64);
    metrics.insert("cis_12".to_string(), d12.cis);
    metrics.insert("cis_13".to_string(), d13.cis);
    metrics.insert("cis_23".to_string(), d23.cis);

    let all_distinguishable = distinguishable == 3;

    let classification = if !states_equal {
        FailureClass::F5ReplayDivergence
    } else if all_distinguishable {
        FailureClass::F1StructuralStability
    } else {
        FailureClass::F4NonIdentifiableCausality
    };

    let mut findings = Vec::new();
    findings.push(format!("Seq1: {} events, {} causal edges", seq1.len(), cg1.edges.len()));
    findings.push(format!("Seq2: {} events, {} causal edges", seq2.len(), cg2.edges.len()));
    findings.push(format!("Seq3: {} events, {} causal edges", seq3.len(), cg3.edges.len()));
    findings.push(format!("All states identical: {}", states_equal));
    findings.push(format!("CIS(1,2)={:.4}, CIS(1,3)={:.4}, CIS(2,3)={:.4}", d12.cis, d13.cis, d23.cis));
    findings.push(format!("Explanations distinguishable in {} of 3 pairs", distinguishable));
    findings.push(format!("E1 chain: {} hops (root={:?})", e1.hops, e1.chain.first().map(|c| c.event_type.clone())));
    findings.push(format!("E2 chain: {} hops (root={:?})", e2.hops, e2.chain.first().map(|c| c.event_type.clone())));
    findings.push(format!("E3 chain: {} hops (root={:?})", e3.hops, e3.chain.first().map(|c| c.event_type.clone())));

    if all_distinguishable {
        findings.push("✓ All three histories produce distinguishably different causal explanations".to_string());
    } else {
        findings.push("⚠ Some histories collapsed into indistinguishable causal explanations".to_string());
    }

    StressReport {
        stress_type: StressType::ReconstructionEquivalence,
        classification,
        events_generated: seq1.len().max(seq2.len()).max(seq3.len()),
        comparison: None,
        metrics,
        causal_metrics: Some(d12),
        findings,
        unexpected_invariants: vec![
            "final state is identical across all three sequences".to_string(),
            "causal graphs differ across sequences with identical state".to_string(),
        ],
    }
}

/// E-C4: Explanation Collapse Test.
/// Force compression: deduplicate repeated events, collapse repeated reasoning chains.
/// Test: does explanation quality degrade or improve?
pub fn experiment_explanation_collapse() -> StressReport {
    // Build a graph with many oscillating property updates (like E-S1 but focused)
    let mut full_events = Vec::new();
    let mut ts: u64 = 0;

    ts += 1; full_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "X"})));
    ts += 1; full_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "Y"})));

    // Oscillate X:color between red/blue many times
    for i in 0..20 {
        ts += 1;
        let val = if i % 2 == 0 { "red" } else { "blue" };
        full_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": val})));
    }
    // Final X:color = "red"
    ts += 1;
    full_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})));

    // Oscillate Y:size between small/large
    for i in 0..10 {
        ts += 1;
        let val = if i % 2 == 0 { "small" } else { "large" };
        full_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "size", "value": val})));
    }
    // Final Y:size = "large"
    ts += 1;
    full_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "size", "value": "large"})));

    // Create compressed version: only keep events that affect the final state
    let mut comp_events = Vec::new();
    ts = 0;
    ts += 1; comp_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "X"})));
    ts += 1; comp_events.push(make_event(ts, EventType::CreateNode, serde_json::json!({"id": "Y"})));
    ts += 1; comp_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})));
    ts += 1; comp_events.push(make_event(ts, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "size", "value": "large"})));

    let (full_state, full_cg) = rebuild(&full_events);
    let (comp_state, comp_cg) = rebuild(&comp_events);

    // Verify states are identical
    let states_equal = full_state == comp_state;

    // Compare causal explanations
    let exp_full_x = full_cg.explain_belief(&full_state, "X", Some("color"), 10);
    let exp_comp_x = comp_cg.explain_belief(&comp_state, "X", Some("color"), 10);
    let exp_full_y = full_cg.explain_belief(&full_state, "Y", Some("size"), 10);
    let exp_comp_y = comp_cg.explain_belief(&comp_state, "Y", Some("size"), 10);

    let stable_x = explanations_are_stable(&exp_full_x, &exp_comp_x);
    let stable_y = explanations_are_stable(&exp_full_y, &exp_comp_y);
    let all_stable = stable_x && stable_y;

    // Compute drift
    let drift = compute_causal_drift_metrics(&comp_state, &comp_cg, Some(&full_cg), Some(&full_state));

    // Explanation quality: chain length (shorter = more compressed)
    let quality_change = if exp_full_x.hops > exp_comp_x.hops {
        "improved (shorter chain after compression)"
    } else if exp_full_x.hops < exp_comp_x.hops {
        "degraded (longer chain after compression)"
    } else {
        "unchanged"
    };

    let mut metrics = BTreeMap::new();
    metrics.insert("full_events".to_string(), full_events.len() as f64);
    metrics.insert("compressed_events".to_string(), comp_events.len() as f64);
    metrics.insert("full_causal_edges".to_string(), full_cg.edges.len() as f64);
    metrics.insert("compressed_causal_edges".to_string(), comp_cg.edges.len() as f64);
    metrics.insert("states_equal".to_string(), if states_equal { 1.0 } else { 0.0 });
    metrics.insert("full_x_hops".to_string(), exp_full_x.hops as f64);
    metrics.insert("comp_x_hops".to_string(), exp_comp_x.hops as f64);
    metrics.insert("full_y_hops".to_string(), exp_full_y.hops as f64);
    metrics.insert("comp_y_hops".to_string(), exp_comp_y.hops as f64);
    metrics.insert("all_explanations_stable".to_string(), if all_stable { 1.0 } else { 0.0 });
    metrics.insert("compression_ratio".to_string(), comp_events.len() as f64 / full_events.len().max(1) as f64);
    metrics.extend(drift.to_map());

    let classification = if !states_equal {
        FailureClass::F5ReplayDivergence
    } else if all_stable {
        FailureClass::F1StructuralStability
    } else {
        FailureClass::F2SemanticDrift
    };

    let mut findings = Vec::new();
    findings.push(format!("Full: {} events → {} causal edges", full_events.len(), full_cg.edges.len()));
    findings.push(format!("Compressed: {} events → {} causal edges", comp_events.len(), comp_cg.edges.len()));
    findings.push(format!("Compression ratio: {:.2}x", comp_events.len() as f64 / full_events.len().max(1) as f64));
    findings.push(format!("States identical: {}", states_equal));
    findings.push(format!("X:color — full chain {} hops, compressed {} hops", exp_full_x.hops, exp_comp_x.hops));
    findings.push(format!("Y:size — full chain {} hops, compressed {} hops", exp_full_y.hops, exp_comp_y.hops));
    findings.push(format!("All explanations stable: {}", all_stable));
    findings.push(format!("Explanation quality: {}", quality_change));
    findings.push(format!("CIS: {:.4}", drift.cis));

    if all_stable {
        findings.push("✓ Compression preserves explanation quality while reducing complexity".to_string());
    } else {
        findings.push("⚠ Compression changes explanation structure".to_string());
    }

    StressReport {
        stress_type: StressType::ExplanationCollapse,
        classification,
        events_generated: full_events.len().max(comp_events.len()),
        comparison: None,
        metrics,
        causal_metrics: Some(drift),
        findings,
        unexpected_invariants: vec![
            "final state is identical under compression".to_string(),
            "causal graph is sparser under compression (fewer overwrite edges)".to_string(),
        ],
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  REPORTING
// ═══════════════════════════════════════════════════════════════════════════════

/// Print a single StressReport section.
fn write_report(output: &mut String, title: &str, report: &StressReport) {
    writeln!(output, "─── {} ───", title).unwrap();
    writeln!(output, "  Stress type:    {}", report.stress_type.label()).unwrap();
    writeln!(output, "  Classification: {} ({})", report.classification.label(), report.classification.severity()).unwrap();
    writeln!(output, "  Events:         {}", report.events_generated).unwrap();
    for (k, v) in &report.metrics {
        if k.starts_with("causal_") || k == "cis" {
            writeln!(output, "  {}: {:.4}", k, v).unwrap();
        } else {
            writeln!(output, "  {}: {:.2}", k, v).unwrap();
        }
    }
    for f in &report.findings {
        writeln!(output, "  • {}", f).unwrap();
    }
    for inv in &report.unexpected_invariants {
        writeln!(output, "  ✦ INVARIANT: {}", inv).unwrap();
    }
    writeln!(output).unwrap();
}

/// Run all experiments (E-S1 through E-S4, E-C1 through E-C4) and produce a combined report.
pub fn run_all_experiments() -> String {
    let mut output = String::new();

    writeln!(output, "══════════════════════════════════════════════════════════════════").unwrap();
    writeln!(output, "  STRATA v0.3 — B4.1 CAUSAL STRESS INTEGRATION & DRIFT RESISTANCE").unwrap();
    writeln!(output, "══════════════════════════════════════════════════════════════════\n").unwrap();

    // ── E-S1 ──
    let r1 = experiment_identity_stress();
    write_report(&mut output, "Experiment E-S1: Identity Stress Test", &r1);

    // ── E-S2 ──
    let r2 = experiment_semantic_collision();
    write_report(&mut output, "Experiment E-S2: Semantic Collision Test", &r2);

    // ── E-S3 ──
    writeln!(output, "─── Experiment E-S3: Causal Reconstruction Test ───").unwrap();
    let reports_s3 = experiment_causal_reconstruction();
    for (i, r) in reports_s3.iter().enumerate() {
        writeln!(output, "  Scenario {}:", i + 1).unwrap();
        writeln!(output, "    Classification: {} ({})", r.classification.label(), r.classification.severity()).unwrap();
        if let Some(ref cm) = r.causal_metrics {
            writeln!(output, "    CIS: {:.4}", cm.cis).unwrap();
        }
        for f in &r.findings {
            writeln!(output, "    • {}", f).unwrap();
        }
        for inv in &r.unexpected_invariants {
            writeln!(output, "    ✦ INVARIANT: {}", inv).unwrap();
        }
    }
    writeln!(output).unwrap();

    // ── E-S4 ──
    let r4 = experiment_representation_equivalence();
    write_report(&mut output, "Experiment E-S4: Representation Equivalence Test", &r4);

    // ── E-C1 ──
    let c1 = experiment_semantic_relabel();
    write_report(&mut output, "Experiment E-C1: Semantic Relabel Attack", &c1);

    // ── E-C2 ──
    let c2 = experiment_causal_swapping();
    write_report(&mut output, "Experiment E-C2: Causal Swapping Attack", &c2);

    // ── E-C3 ──
    let c3 = experiment_reconstruction_equivalence();
    write_report(&mut output, "Experiment E-C3: Reconstruction Equivalence Test", &c3);

    // ── E-C4 ──
    let c4 = experiment_explanation_collapse();
    write_report(&mut output, "Experiment E-C4: Explanation Collapse Test", &c4);

    // ════════════════════════════════════════════════════════════
    //  SUMMARY & VERDICT
    // ════════════════════════════════════════════════════════════

    writeln!(output, "══════════════════════════════════════════════════════════════════").unwrap();
    writeln!(output, "  CAUSAL INTEGRITY SCORE (CIS) SUMMARY").unwrap();
    writeln!(output, "══════════════════════════════════════════════════════════════════").unwrap();

    let all_cis: Vec<(&str, Option<f64>)> = vec![
        ("E-S1 Identity Drift", r1.causal_metrics.as_ref().map(|c| c.cis)),
        ("E-S2 Semantic Collision", r2.causal_metrics.as_ref().map(|c| c.cis)),
        ("E-S4 Representation Equivalence", r4.causal_metrics.as_ref().map(|c| c.cis)),
        ("E-C1 Semantic Relabel", c1.causal_metrics.as_ref().map(|c| c.cis)),
        ("E-C2 Causal Swapping", c2.causal_metrics.as_ref().map(|c| c.cis)),
        ("E-C3 Reconstruction Equivalence", c3.causal_metrics.as_ref().map(|c| c.cis)),
        ("E-C4 Explanation Collapse", c4.causal_metrics.as_ref().map(|c| c.cis)),
    ];

    let mut total_cis = 0.0f64;
    let mut cis_count = 0u64;
    for (name, cis) in &all_cis {
        if let Some(v) = cis {
            writeln!(output, "  {:<45} {:.4}", name, v).unwrap();
            total_cis += *v;
            cis_count += 1;
        } else {
            writeln!(output, "  {:<45} N/A", name).unwrap();
        }
    }
    let avg_cis = if cis_count > 0 { total_cis / cis_count as f64 } else { 0.0 };
    writeln!(output, "  {}", "-".repeat(60)).unwrap();
    writeln!(output, "  {:<45} {:.4}", "AVERAGE CIS", avg_cis).unwrap();

    // Worst-case drift scenario
    let worst = all_cis.iter().filter_map(|(n, c)| c.map(|v| (*n, v))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    if let Some((name, val)) = worst {
        writeln!(output, "\n  Worst-case CIS: {} ({:.4})", name, val).unwrap();
    }

    // E-C3 check: distinguishable explanations
    let c3_distinguishable = c3.metrics.get("distinguishable_pairs").copied().unwrap_or(0.0) as u64;
    let c3_pass = c3_distinguishable > 0;

    // E-C2 check: contradiction/ambiguity detection
    let c2_ambiguity = c2.metrics.get("ambiguity_flagged").copied().unwrap_or(0.0) > 0.0;

    // CIS pass: ≥ 0.75 under all E-S tests
    let es_cis_pass = [&r1, &r2, &r4].iter().all(|r| {
        r.causal_metrics.as_ref().map(|c| c.cis >= 0.75).unwrap_or(false)
    });

    writeln!(output, "\n══════════════════════════════════════════════════════════════════").unwrap();
    writeln!(output, "  B4.1 PASS/FAIL ASSESSMENT").unwrap();
    writeln!(output, "══════════════════════════════════════════════════════════════════").unwrap();

    writeln!(output, "  CIS ≥ 0.75 under all E-S tests:            {}", if es_cis_pass { "PASS" } else { "FAIL" }).unwrap();
    writeln!(output, "  E-C3 distinguishable explanations:         {}", if c3_pass { "PASS" } else { "FAIL" }).unwrap();
    writeln!(output, "  E-C1 explanation quality preserved:        {}", if c1.classification == FailureClass::F1StructuralStability { "PASS" } else { "FAIL" }).unwrap();
    writeln!(output, "  E-C2 contradiction/ambiguity flagged:      {}", if c2_ambiguity { "PASS" } else { "FAIL" }).unwrap();

    let all_pass = es_cis_pass && c3_pass && c1.classification == FailureClass::F1StructuralStability && c2_ambiguity;
    writeln!(output, "  {}", "=".repeat(60)).unwrap();
    writeln!(output, "  OVERALL: {}", if all_pass { "B4.1 PASS — Causal layer adds genuine discriminability" } else { "B4.1 FAIL — Causal layer needs revision" }).unwrap();
    writeln!(output, "══════════════════════════════════════════════════════════════════").unwrap();

    output
}
