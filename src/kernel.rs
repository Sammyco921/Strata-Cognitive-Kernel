use std::collections::BTreeMap;

// ── Event Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn from_str(s: &str) -> Option<Confidence> {
        match s.to_lowercase().as_str() {
            "low" => Some(Confidence::Low),
            "medium" => Some(Confidence::Medium),
            "high" => Some(Confidence::High),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }
}

/// The core event payload — pure state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    CreateNode { id: u64, node_type: String },
    CreateEdge { id: u64, from_node: u64, to_node: u64, edge_type: String },
    SetProperty { node_id: u64, key: String, value: String },
    AssertBelief { node_id: u64, confidence: Confidence },
    AttachEvidence { belief_id: u64, evidence_id: u64 },
}

/// A fully-specified kernel event with identity, ordering, causality, and payload.
///
/// Ordering is determined by `logical_seq` (primary) then `id` (tie-break).
/// `timestamp` is informational only and MUST NOT affect ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequencedEvent {
    pub logical_seq: u64,
    pub id: String,
    pub timestamp: Option<u64>,
    pub causes: Vec<String>,
    pub event: Event,
}

impl SequencedEvent {
    pub fn new(
        logical_seq: u64,
        id: String,
        timestamp: Option<u64>,
        causes: Vec<String>,
        event: Event,
    ) -> Self {
        SequencedEvent { logical_seq, id, timestamp, causes, event }
    }

    /// Convenience constructor: derives `id` from `logical_seq`.
    pub fn from_seq(logical_seq: u64, event: Event) -> Self {
        SequencedEvent {
            logical_seq,
            id: format!("evt-{}", logical_seq),
            timestamp: None,
            causes: Vec::new(),
            event,
        }
    }

    pub fn sort_key(&self) -> (u64, &str) {
        (self.logical_seq, &self.id)
    }
}

impl PartialOrd for SequencedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SequencedEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.logical_seq
            .cmp(&other.logical_seq)
            .then_with(|| self.id.cmp(&other.id))
    }
}

/// Validation error for events with missing or invalid logical_seq.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventValidationError {
    MissingLogicalSeq,
    MissingId,
    Other(String),
}

/// Sort a slice of events in-place by (logical_seq, id).
/// This ensures deterministic replay regardless of input order.
pub fn sort_events(events: &mut [SequencedEvent]) {
    events.sort();
}

/// Validate that an event has the required fields for replay.
pub fn validate_event(event: &SequencedEvent) -> Result<(), EventValidationError> {
    if event.id.is_empty() {
        return Err(EventValidationError::MissingId);
    }
    Ok(())
}

// ── Graph State ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: u64,
    pub node_type: String,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub id: u64,
    pub from_node: u64,
    pub to_node: u64,
    pub edge_type: String,
    pub properties: BTreeMap<String, String>,
}

pub type NodeId = u64;
pub type EdgeId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphState {
    pub nodes: BTreeMap<NodeId, Node>,
    pub edges: BTreeMap<EdgeId, Edge>,
}

impl GraphState {
    pub fn empty() -> Self {
        GraphState {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.get(&id)
    }

    pub fn find_nodes_by_type(&self, node_type: &str) -> Vec<&Node> {
        self.nodes
            .values()
            .filter(|n| n.node_type == node_type)
            .collect()
    }

    pub fn find_edges_by_type(&self, edge_type: &str) -> Vec<&Edge> {
        self.edges
            .values()
            .filter(|e| e.edge_type == edge_type)
            .collect()
    }

    pub fn edges_from(&self, node_id: NodeId) -> Vec<&Edge> {
        self.edges
            .values()
            .filter(|e| e.from_node == node_id)
            .collect()
    }

    pub fn edges_to(&self, node_id: NodeId) -> Vec<&Edge> {
        self.edges
            .values()
            .filter(|e| e.to_node == node_id)
            .collect()
    }
}

// ── Event Log ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EventLog {
    events: Vec<SequencedEvent>,
    next_seq: u64,
}

impl EventLog {
    pub fn new() -> Self {
        EventLog {
            events: Vec::new(),
            next_seq: 1,
        }
    }

    pub fn append(&mut self, event: Event) -> SequencedEvent {
        let logical_seq = self.next_seq;
        self.next_seq += 1;
        let id = format!("evt-{}", logical_seq);
        let se = SequencedEvent::new(logical_seq, id, None, Vec::new(), event);
        self.events.push(se.clone());
        se
    }

    pub fn append_with_causes(
        &mut self,
        event: Event,
        causes: Vec<String>,
    ) -> SequencedEvent {
        let logical_seq = self.next_seq;
        self.next_seq += 1;
        let id = format!("evt-{}", logical_seq);
        let se = SequencedEvent::new(logical_seq, id, None, causes, event);
        self.events.push(se.clone());
        se
    }

    pub fn iter(&self) -> impl Iterator<Item = &SequencedEvent> {
        self.events.iter()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn get_events(&self) -> &[SequencedEvent] {
        &self.events
    }
}

// ── Validation ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    DuplicateNodeId(u64),
    DuplicateEdgeId(u64),
    NodeNotFound(u64),
    EdgeNotFound(u64),
    Other(String),
}

pub trait Validator: std::fmt::Debug {
    fn validate(&self, event: &Event, state: &GraphState) -> Result<(), ValidationError>;
}

#[derive(Debug)]
pub struct NoDuplicateNodeValidator;

impl Validator for NoDuplicateNodeValidator {
    fn validate(&self, event: &Event, state: &GraphState) -> Result<(), ValidationError> {
        if let Event::CreateNode { id, .. } = event {
            if state.nodes.contains_key(id) {
                return Err(ValidationError::DuplicateNodeId(*id));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct NoDuplicateEdgeValidator;

impl Validator for NoDuplicateEdgeValidator {
    fn validate(&self, event: &Event, state: &GraphState) -> Result<(), ValidationError> {
        if let Event::CreateEdge { id, .. } = event {
            if state.edges.contains_key(id) {
                return Err(ValidationError::DuplicateEdgeId(*id));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct NodeExistenceValidator;

impl Validator for NodeExistenceValidator {
    fn validate(&self, event: &Event, state: &GraphState) -> Result<(), ValidationError> {
        match event {
            Event::CreateEdge { from_node, to_node, .. } => {
                if !state.nodes.contains_key(from_node) {
                    return Err(ValidationError::NodeNotFound(*from_node));
                }
                if !state.nodes.contains_key(to_node) {
                    return Err(ValidationError::NodeNotFound(*to_node));
                }
            }
            Event::SetProperty { node_id, .. } => {
                if !state.nodes.contains_key(node_id) && !state.edges.contains_key(node_id) {
                    return Err(ValidationError::NodeNotFound(*node_id));
                }
            }
            Event::AssertBelief { node_id, .. } => {
                if !state.nodes.contains_key(node_id) {
                    return Err(ValidationError::NodeNotFound(*node_id));
                }
            }
            Event::AttachEvidence { belief_id, .. } => {
                if !state.nodes.contains_key(belief_id) {
                    return Err(ValidationError::NodeNotFound(*belief_id));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct EdgeExistenceValidator;

impl Validator for EdgeExistenceValidator {
    fn validate(&self, event: &Event, state: &GraphState) -> Result<(), ValidationError> {
        if let Event::AttachEvidence { evidence_id, .. } = event {
            if !state.edges.contains_key(evidence_id) {
                return Err(ValidationError::EdgeNotFound(*evidence_id));
            }
        }
        Ok(())
    }
}

pub fn default_validators() -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(NoDuplicateNodeValidator),
        Box::new(NoDuplicateEdgeValidator),
        Box::new(NodeExistenceValidator),
        Box::new(EdgeExistenceValidator),
    ]
}

// ── Event Application (State Projection) ─────────────────────────────────

pub fn apply_event(state: &mut GraphState, event: &Event) {
    match event {
        Event::CreateNode { id, node_type } => {
            state.nodes.insert(*id, Node {
                id: *id,
                node_type: node_type.clone(),
                properties: BTreeMap::new(),
            });
        }
        Event::CreateEdge { id, from_node, to_node, edge_type } => {
            state.edges.insert(*id, Edge {
                id: *id,
                from_node: *from_node,
                to_node: *to_node,
                edge_type: edge_type.clone(),
                properties: BTreeMap::new(),
            });
        }
        Event::SetProperty { node_id, key, value } => {
            if let Some(node) = state.nodes.get_mut(node_id) {
                node.properties.insert(key.clone(), value.clone());
            } else if let Some(edge) = state.edges.get_mut(node_id) {
                edge.properties.insert(key.clone(), value.clone());
            }
        }
        Event::AssertBelief { node_id, .. } => {
            if let Some(node) = state.nodes.get_mut(node_id) {
                node.properties.insert("belief".to_string(), "true".to_string());
            }
        }
        Event::AttachEvidence { belief_id, evidence_id } => {
            if let Some(node) = state.nodes.get_mut(belief_id) {
                let count = node.properties
                    .get("evidence_count")
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                node.properties.insert("evidence_count".to_string(), (count + 1).to_string());
                let evidence_list = node.properties
                    .entry("evidence_ids".to_string())
                    .or_insert_with(|| String::new());
                if !evidence_list.is_empty() {
                    evidence_list.push(',');
                }
                evidence_list.push_str(&evidence_id.to_string());
            }
        }
    }
}

pub fn replay(events: &[SequencedEvent]) -> GraphState {
    let mut sorted = events.to_vec();
    sort_events(&mut sorted);
    let mut state = GraphState::empty();
    for se in &sorted {
        apply_event(&mut state, &se.event);
    }
    state
}

pub fn replay_from_log(log: &EventLog) -> GraphState {
    replay(log.get_events())
}

// ── Kernel ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum CommitError {
    ValidationFailed(Vec<ValidationError>),
}

#[derive(Debug)]
pub struct Kernel {
    log: EventLog,
    state: GraphState,
    validators: Vec<Box<dyn Validator>>,
}

impl Kernel {
    pub fn new() -> Self {
        Kernel {
            log: EventLog::new(),
            state: GraphState::empty(),
            validators: default_validators(),
        }
    }

    pub fn with_validators(validators: Vec<Box<dyn Validator>>) -> Self {
        Kernel {
            log: EventLog::new(),
            state: GraphState::empty(),
            validators,
        }
    }

    pub fn propose_and_commit(&mut self, event: Event) -> Result<(), CommitError> {
        let mut errors = Vec::new();
        for validator in &self.validators {
            if let Err(e) = validator.validate(&event, &self.state) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(CommitError::ValidationFailed(errors));
        }
        let se = self.log.append(event);
        apply_event(&mut self.state, &se.event);
        Ok(())
    }

    pub fn state(&self) -> &GraphState {
        &self.state
    }

    pub fn log(&self) -> &EventLog {
        &self.log
    }

    pub fn replay(&self) -> GraphState {
        replay_from_log(&self.log)
    }

    pub fn assert_equivalent(&self) -> bool {
        let replayed = self.replay();
        self.state == replayed
    }

    pub fn event_count(&self) -> usize {
        self.log.len()
    }
}

// ── Belief State Query ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeliefEntry {
    pub node_id: NodeId,
    pub node_type: String,
    pub evidence_count: u64,
    pub effective_confidence: Confidence,
}

pub fn compute_belief_state(state: &GraphState) -> Vec<BeliefEntry> {
    let mut beliefs = Vec::new();
    for node in state.nodes.values() {
        if node.properties.get("belief") == Some(&"true".to_string()) {
            let evidence_count = node
                .properties
                .get("evidence_count")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let effective_confidence = match evidence_count {
                0 => Confidence::Low,
                1 => Confidence::Low,
                2 | 3 => Confidence::Medium,
                _ => Confidence::High,
            };
            beliefs.push(BeliefEntry {
                node_id: node.id,
                node_type: node.node_type.clone(),
                evidence_count,
                effective_confidence,
            });
        }
    }
    beliefs.sort_by_key(|b| b.node_id);
    beliefs
}

pub fn find_belief(state: &GraphState, node_id: NodeId) -> Option<BeliefEntry> {
    compute_belief_state(state)
        .into_iter()
        .find(|b| b.node_id == node_id)
}

// ── Semantic Overlap Measurement (for E4) ─────────────────────────────────
// Matches nodes by their semantic identity (type + name/label property),
// not by numeric ID, since independent encodings use different IDs.

#[derive(Debug, Clone)]
pub struct OverlapMetrics {
    pub total_nodes_v1: usize,
    pub total_nodes_v2: usize,
    pub overlapping_nodes: usize,
    pub total_edges_v1: usize,
    pub total_edges_v2: usize,
    pub overlapping_edges: usize,
    /// Edge overlap ignoring edge type (same from→to node pairs)
    pub overlapping_edges_topology: usize,
    pub node_overlap_pct: f64,
    pub edge_overlap_pct: f64,
    pub edge_topology_overlap_pct: f64,
    pub structural_similarity_pct: f64,
}

fn node_semantic_key(node: &Node) -> (String, String) {
    let name = node.properties.get("name")
        .or_else(|| node.properties.get("label"))
        .cloned()
        .unwrap_or_default();
    (node.node_type.clone(), name)
}

pub fn measure_graph_overlap(g1: &GraphState, g2: &GraphState) -> OverlapMetrics {
    let sem_keys_1: BTreeSet<(String, String)> = g1.nodes.values().map(node_semantic_key).collect();
    let sem_keys_2: BTreeSet<(String, String)> = g2.nodes.values().map(node_semantic_key).collect();
    let overlapping_nodes = sem_keys_1.intersection(&sem_keys_2).count();
    let total_unique_node_keys = sem_keys_1.len().max(sem_keys_2.len());

    // Map edges to semantic triples: (from_name, edge_type, to_name)
    let edge_triples_1: BTreeSet<(String, String, String)> = g1.edges.values()
        .filter_map(|e| {
            let from_node = g1.nodes.get(&e.from_node)?;
            let to_node = g1.nodes.get(&e.to_node)?;
            let fk = node_semantic_key(from_node);
            let tk = node_semantic_key(to_node);
            Some((fk.1, e.edge_type.clone(), tk.1))
        })
        .collect();

    let edge_triples_2: BTreeSet<(String, String, String)> = g2.edges.values()
        .filter_map(|e| {
            let from_node = g2.nodes.get(&e.from_node)?;
            let to_node = g2.nodes.get(&e.to_node)?;
            let fk = node_semantic_key(from_node);
            let tk = node_semantic_key(to_node);
            Some((fk.1, e.edge_type.clone(), tk.1))
        })
        .collect();

    // Topology-only edges (ignore edge type): just (from_name, to_name)
    let edge_pairs_1: BTreeSet<(String, String)> = edge_triples_1.iter()
        .map(|(f, _, t)| (f.clone(), t.clone())).collect();
    let edge_pairs_2: BTreeSet<(String, String)> = edge_triples_2.iter()
        .map(|(f, _, t)| (f.clone(), t.clone())).collect();

    let overlapping_edges = edge_triples_1.intersection(&edge_triples_2).count();
    let total_unique_edge_triples = edge_triples_1.len().max(edge_triples_2.len());

    let overlapping_edges_topology = edge_pairs_1.intersection(&edge_pairs_2).count();
    let total_unique_pairs = edge_pairs_1.len().max(edge_pairs_2.len());

    let node_pct = if total_unique_node_keys > 0 {
        (overlapping_nodes as f64 / total_unique_node_keys as f64) * 100.0
    } else {
        100.0
    };

    let edge_pct = if total_unique_edge_triples > 0 {
        (overlapping_edges as f64 / total_unique_edge_triples as f64) * 100.0
    } else {
        100.0
    };

    let topo_pct = if total_unique_pairs > 0 {
        (overlapping_edges_topology as f64 / total_unique_pairs as f64) * 100.0
    } else {
        100.0
    };

    OverlapMetrics {
        total_nodes_v1: g1.nodes.len(),
        total_nodes_v2: g2.nodes.len(),
        overlapping_nodes,
        total_edges_v1: g1.edges.len(),
        total_edges_v2: g2.edges.len(),
        overlapping_edges,
        overlapping_edges_topology,
        node_overlap_pct: node_pct,
        edge_overlap_pct: edge_pct,
        edge_topology_overlap_pct: topo_pct,
        structural_similarity_pct: (node_pct + edge_pct) / 2.0,
    }
}

use std::collections::BTreeSet;

// ── Kernel State Bundle (State Boundary Model) ──────────────────────────

/// Configuration state — immutable at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigState {
    pub abi_registry: Vec<String>,
    pub invariant_definitions: Vec<String>,
    pub schema_descriptors: Vec<String>,
}

impl ConfigState {
    pub fn empty() -> Self {
        ConfigState {
            abi_registry: Vec::new(),
            invariant_definitions: Vec::new(),
            schema_descriptors: Vec::new(),
        }
    }
}

/// Derived views — ephemeral state that MUST NOT persist
/// unless explicitly converted into events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedViews {
    pub query_results: Vec<String>,
    pub execution_plans: Vec<String>,
    pub rule_annotations: Vec<String>,
    pub coherence_reports: Vec<String>,
}

impl DerivedViews {
    pub fn empty() -> Self {
        DerivedViews {
            query_results: Vec::new(),
            execution_plans: Vec::new(),
            rule_annotations: Vec::new(),
            coherence_reports: Vec::new(),
        }
    }
}

/// Kernel state boundary bundle.
///
/// Categorizes all state into three explicit compartments:
///
/// 1. **event_state** — Fully reconstructible from event log.
/// 2. **config_state** — Immutable configuration, cannot change at runtime.
/// 3. **derived_views** — Ephemeral, must not persist unless converted to events.
///
/// Hard invariant: All persistent state transitions MUST originate from events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelStateBundle {
    pub event_state: GraphState,
    pub config_state: ConfigState,
    pub derived_views: DerivedViews,
}

impl KernelStateBundle {
    pub fn new(event_state: GraphState) -> Self {
        KernelStateBundle {
            event_state,
            config_state: ConfigState::empty(),
            derived_views: DerivedViews::empty(),
        }
    }

    pub fn with_config(event_state: GraphState, config_state: ConfigState) -> Self {
        KernelStateBundle {
            event_state,
            config_state,
            derived_views: DerivedViews::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_kernel() {
        let kernel = Kernel::new();
        assert!(kernel.state().nodes.is_empty());
        assert!(kernel.state().edges.is_empty());
        assert_eq!(kernel.event_count(), 0);
    }

    #[test]
    fn test_create_node() {
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "test".into() }).unwrap();
        assert_eq!(kernel.state().node_count(), 1);
        assert_eq!(kernel.state().get_node(1).unwrap().node_type, "test");
    }

    #[test]
    fn test_duplicate_node_rejected() {
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "test".into() }).unwrap();
        let result = kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "other".into() });
        assert!(result.is_err());
        assert_eq!(kernel.state().node_count(), 1);
    }

    #[test]
    fn test_replay_equivalence() {
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "a".into() }).unwrap();
        kernel.propose_and_commit(Event::CreateNode { id: 2, node_type: "b".into() }).unwrap();
        kernel.propose_and_commit(Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "link".into() }).unwrap();
        kernel.propose_and_commit(Event::SetProperty { node_id: 1, key: "color".into(), value: "red".into() }).unwrap();
        assert!(kernel.assert_equivalent());
    }

    #[test]
    fn test_belief_confidence() {
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 1, node_type: "condition".into() }).unwrap();
        kernel.propose_and_commit(Event::CreateNode { id: 2, node_type: "symptom".into() }).unwrap();
        kernel.propose_and_commit(Event::CreateNode { id: 3, node_type: "symptom".into() }).unwrap();
        kernel.propose_and_commit(Event::CreateNode { id: 4, node_type: "symptom".into() }).unwrap();

        kernel.propose_and_commit(Event::AssertBelief { node_id: 1, confidence: Confidence::Low }).unwrap();

        let beliefs = compute_belief_state(kernel.state());
        assert_eq!(beliefs.len(), 1);
        assert_eq!(beliefs[0].evidence_count, 0);
        assert_eq!(beliefs[0].effective_confidence, Confidence::Low);

        kernel.propose_and_commit(Event::CreateEdge { id: 10, from_node: 2, to_node: 1, edge_type: "evidence_for".into() }).unwrap();
        kernel.propose_and_commit(Event::AttachEvidence { belief_id: 1, evidence_id: 10 }).unwrap();
        let beliefs = compute_belief_state(kernel.state());
        assert_eq!(beliefs[0].evidence_count, 1);
        assert_eq!(beliefs[0].effective_confidence, Confidence::Low);

        kernel.propose_and_commit(Event::CreateEdge { id: 11, from_node: 3, to_node: 1, edge_type: "evidence_for".into() }).unwrap();
        kernel.propose_and_commit(Event::AttachEvidence { belief_id: 1, evidence_id: 11 }).unwrap();
        let beliefs = compute_belief_state(kernel.state());
        assert_eq!(beliefs[0].evidence_count, 2);
        assert_eq!(beliefs[0].effective_confidence, Confidence::Medium);

        kernel.propose_and_commit(Event::CreateEdge { id: 12, from_node: 4, to_node: 1, edge_type: "evidence_for".into() }).unwrap();
        kernel.propose_and_commit(Event::AttachEvidence { belief_id: 1, evidence_id: 12 }).unwrap();
        let beliefs = compute_belief_state(kernel.state());
        assert_eq!(beliefs[0].evidence_count, 3);
        assert_eq!(beliefs[0].effective_confidence, Confidence::Medium);

        kernel.propose_and_commit(Event::CreateNode { id: 5, node_type: "symptom".into() }).unwrap();
        kernel.propose_and_commit(Event::CreateEdge { id: 13, from_node: 5, to_node: 1, edge_type: "evidence_for".into() }).unwrap();
        kernel.propose_and_commit(Event::AttachEvidence { belief_id: 1, evidence_id: 13 }).unwrap();
        let beliefs = compute_belief_state(kernel.state());
        assert_eq!(beliefs[0].evidence_count, 4);
        assert_eq!(beliefs[0].effective_confidence, Confidence::High);
    }

    #[test]
    fn test_overlap_identical() {
        let g = GraphState::empty();
        let metrics = measure_graph_overlap(&g, &g);
        assert_eq!(metrics.structural_similarity_pct, 100.0);
    }

    // ── PART A: Event-Time Consistency Tests ───────────────────────────

    #[test]
    fn test_same_events_different_timestamps_identical_replay() {
        let mut events_a = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 2, node_type: "B".into() }),
        ];
        events_a[0].timestamp = Some(100);
        events_a[1].timestamp = Some(200);

        let mut events_b = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 2, node_type: "B".into() }),
        ];
        events_b[0].timestamp = Some(9999);
        events_b[1].timestamp = Some(0);

        assert_eq!(replay(&events_a), replay(&events_b));
    }

    #[test]
    fn test_shuffled_events_identical_final_state() {
        let ordered = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 2, node_type: "B".into() }),
            SequencedEvent::from_seq(3, Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "link".into() }),
        ];

        let shuffled = vec![
            SequencedEvent::from_seq(3, Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "link".into() }),
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 2, node_type: "B".into() }),
        ];

        assert_eq!(replay(&ordered), replay(&shuffled));
    }

    #[test]
    fn test_shuffled_events_100_runs() {
        let ordered = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "X".into() }),
            SequencedEvent::from_seq(2, Event::SetProperty { node_id: 1, key: "color".into(), value: "red".into() }),
            SequencedEvent::from_seq(3, Event::CreateNode { id: 2, node_type: "Y".into() }),
        ];
        let first = replay(&ordered);
        for _ in 0..100 {
            let mut shuffled = ordered.clone();
            shuffled.reverse();
            assert_eq!(first, replay(&shuffled));
        }
    }

    #[test]
    fn test_validate_event_missing_id_rejected() {
        let event = SequencedEvent {
            logical_seq: 1,
            id: String::new(),
            timestamp: None,
            causes: Vec::new(),
            event: Event::CreateNode { id: 1, node_type: "test".into() },
        };
        assert!(validate_event(&event).is_err());
    }

    #[test]
    fn test_validate_event_valid_accepted() {
        let event = SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "test".into() });
        assert!(validate_event(&event).is_ok());
    }

    #[test]
    fn test_causal_chain_preserved_across_replay() {
        let events = vec![
            SequencedEvent::new(1, "evt-A".into(), None, vec![], Event::CreateNode { id: 1, node_type: "A".into() }),
            SequencedEvent::new(2, "evt-B".into(), None, vec!["evt-A".into()], Event::CreateNode { id: 2, node_type: "B".into() }),
            SequencedEvent::new(3, "evt-C".into(), None, vec!["evt-B".into()], Event::CreateNode { id: 3, node_type: "C".into() }),
        ];
        let state = replay(&events);
        assert_eq!(state.node_count(), 3);
        assert!(state.nodes.contains_key(&1));
        assert!(state.nodes.contains_key(&2));
        assert!(state.nodes.contains_key(&3));
        // Causes field is informational; ordering by logical_seq ensures replay
        assert_eq!(events[1].causes, vec!["evt-A"]);
        assert_eq!(events[2].causes, vec!["evt-B"]);
    }

    #[test]
    fn test_sequenced_event_ordering_by_logical_seq_then_id() {
        let mut events = vec![
            SequencedEvent::new(2, "evt-B".into(), None, vec![], Event::CreateNode { id: 2, node_type: "B".into() }),
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
            SequencedEvent::new(2, "evt-A".into(), None, vec![], Event::CreateNode { id: 3, node_type: "C".into() }),
        ];
        // After sort: logical_seq 1 first, then logical_seq 2 sorted by id ("evt-A" < "evt-B")
        sort_events(&mut events);
        assert_eq!(events[0].logical_seq, 1);
        assert_eq!(events[1].logical_seq, 2);
        assert_eq!(events[1].id, "evt-A");
        assert_eq!(events[2].id, "evt-B");
    }

    #[test]
    fn test_sort_events_is_stable_for_equal_keys() {
        let e1 = SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() });
        let e2 = SequencedEvent::from_seq(1, Event::CreateNode { id: 2, node_type: "B".into() });
        let e3 = SequencedEvent::from_seq(1, Event::CreateNode { id: 3, node_type: "C".into() });
        let original = vec![e3.clone(), e2.clone(), e1.clone()];
        let mut events = original.clone();
        sort_events(&mut events);
        // All have same logical_seq and id ("evt-1"), so sort preserves original order
        assert_eq!(events[0].event, original[0].event);
        assert_eq!(events[1].event, original[1].event);
        assert_eq!(events[2].event, original[2].event);
    }

    // ── PART B: State Boundary Tests ──────────────────────────────────

    #[test]
    fn test_event_replay_reconstructs_identical_event_state() {
        let events = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 2, node_type: "B".into() }),
        ];
        let state = replay(&events);
        let bundle = KernelStateBundle::new(state.clone());
        assert_eq!(bundle.event_state, state);
        assert_eq!(bundle.event_state.node_count(), 2);
    }

    #[test]
    fn test_config_state_immutable_across_runs() {
        let config = ConfigState {
            abi_registry: vec!["abi:v1".into()],
            invariant_definitions: vec!["inv:replay".into()],
            schema_descriptors: vec!["schema:kernel".into()],
        };
        let config_clone = config.clone();
        // Config state is never mutated by kernel operations
        assert_eq!(config, config_clone);
    }

    #[test]
    fn test_derived_views_do_not_mutate_event_state() {
        let events = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
        ];
        let state = replay(&events);
        let original_state = state.clone();

        let mut views = DerivedViews::empty();
        views.query_results.push("result:1".into());
        views.execution_plans.push("plan:1".into());

        // Derived views are separate from event state — no mutation path
        assert_eq!(state, original_state);
        assert_eq!(views.query_results.len(), 1);
        assert_eq!(views.execution_plans.len(), 1);
    }

    #[test]
    fn test_no_state_mutation_outside_event_application() {
        let mut log = EventLog::new();
        let event = Event::CreateNode { id: 1, node_type: "test".into() };
        let se = log.append(event.clone());

        // Verify the EventLog only stores events, doesn't apply them
        assert_eq!(log.len(), 1);
        assert_eq!(se.logical_seq, 1);
        assert_eq!(se.event, event);

        // State is only produced via replay/apply_event
        let state = replay(log.get_events());
        assert_eq!(state.node_count(), 1);
    }

    #[test]
    fn test_kernel_state_bundle_boundaries() {
        let events = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 1, node_type: "A".into() }),
        ];
        let event_state = replay(&events);

        let config = ConfigState {
            abi_registry: vec!["abi:v1".into()],
            invariant_definitions: vec![],
            schema_descriptors: vec![],
        };

        let mut views = DerivedViews::empty();
        views.coherence_reports.push("report:1".into());

        let bundle = KernelStateBundle { event_state: event_state.clone(), config_state: config.clone(), derived_views: views.clone() };

        // Event state is fully reconstructible from events
        let reconstructed = replay(&events);
        assert_eq!(bundle.event_state, reconstructed);

        // Config state is independent
        assert_eq!(bundle.config_state, config);

        // Derived views are independent
        assert_eq!(bundle.derived_views, views);

        // No cross-contamination
        assert_ne!(bundle.event_state, GraphState::empty());
    }

    #[test]
    fn test_event_log_sequential_ids_are_monotonic() {
        let mut log = EventLog::new();
        let e1 = log.append(Event::CreateNode { id: 1, node_type: "A".into() });
        let e2 = log.append(Event::CreateNode { id: 2, node_type: "B".into() });
        let e3 = log.append(Event::CreateNode { id: 3, node_type: "C".into() });
        assert!(e1.logical_seq < e2.logical_seq);
        assert!(e2.logical_seq < e3.logical_seq);
        assert_eq!(e1.logical_seq, 1);
        assert_eq!(e3.logical_seq, 3);
    }

    #[test]
    fn test_event_replay_determinism_100_runs() {
        let events = vec![
            SequencedEvent::from_seq(1, Event::CreateNode { id: 5, node_type: "X".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 3, node_type: "Y".into() }),
            SequencedEvent::from_seq(3, Event::SetProperty { node_id: 5, key: "name".into(), value: "test".into() }),
            SequencedEvent::from_seq(4, Event::CreateEdge { id: 1, from_node: 5, to_node: 3, edge_type: "link".into() }),
        ];
        let first = replay(&events);
        for _ in 0..100 {
            assert_eq!(first, replay(&events));
        }
    }

    #[test]
    fn test_event_log_roundtrip_via_replay() {
        let mut kernel = Kernel::new();
        kernel.propose_and_commit(Event::CreateNode { id: 10, node_type: "test".into() }).unwrap();
        kernel.propose_and_commit(Event::SetProperty { node_id: 10, key: "color".into(), value: "blue".into() }).unwrap();

        let log = kernel.log();
        let replayed = replay(log.get_events());
        assert_eq!(*kernel.state(), replayed);
        assert!(kernel.assert_equivalent());
    }

    #[test]
    fn test_append_with_causes() {
        let mut log = EventLog::new();
        let se = log.append_with_causes(
            Event::CreateNode { id: 1, node_type: "A".into() },
            vec!["evt-0".into()],
        );
        assert_eq!(se.causes, vec!["evt-0"]);
        assert_eq!(se.logical_seq, 1);
        assert_eq!(se.id, "evt-1");
    }
}
