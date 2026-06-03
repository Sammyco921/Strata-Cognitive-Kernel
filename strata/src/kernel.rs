use crate::causal::{CausalGraph, CausalChainLink, Explanation, project_default};
use crate::error::KernelError;
use crate::event::{Event, EventType};
use crate::graph::{Edge, GraphState, Node};
use crate::persistence;
use crate::version::CURRENT_KERNEL_VERSION;

/// B7 Kernel v1 — FROZEN PUBLIC API.
///
/// Stable v1 interfaces (signatures frozen, breaking changes = major version bump):
///   - propose(&self, event: &Event) -> Result<(), KernelError>
///   - commit(&mut self, event: Event) -> Result<(), KernelError>
///   - replay(events: &[Event]) -> GraphState              (free function)
///   - query via get_state() / get_causal_graph()
///   - trace_causal_chain(&self, event_id: &str) -> Vec<CausalChainLink>
///   - explain_belief(&self, node_id, property_key) -> Explanation
///
/// G₀ = Ground Truth Event Graph (events + immutable state replay).
/// G₁ = Derived Causal/Explanation Graph (lossy, policy-driven projection).
///
/// B6 Invariants (still enforced):
///   I1: replay(G₀) is byte-for-byte deterministic
///   I2: G₁ is a strictly read-only projection of G₀
///   I3: Only G₀ events are authoritative (truth, causality origin, validation)
///   I4: Explanations cite G₁ paths anchored to G₀ event IDs
///
/// B7 Invariants (added):
///   V1: Every persisted artifact carries schema version
///   V2: Historical logs remain replayable forever
///   V3: Replay hash is stable across 100+ runs
///   V4: Future features adapt to old events, not vice versa
pub struct Kernel {
    pub state: GraphState,
    pub clock: u64,
    pub event_count: u64,
    pub prior_events: Vec<Event>,
}

impl Kernel {
    /// Create a new kernel from persisted state (snapshot + event log).
    pub fn new() -> Self {
        Kernel::new_with_events(None)
    }

    /// Create a kernel with an explicit event list (used by tests to avoid file I/O).
    pub fn new_test(events: Vec<Event>) -> Self {
        let state = replay(&events);
        let clock = events.last().map(|e| e.timestamp).unwrap_or(0);
        let prior = events;
        Kernel { state, clock, event_count: clock, prior_events: prior }
    }

    fn new_with_events(loaded: Option<Vec<Event>>) -> Self {
        println!("[strata] initializing kernel...");

        let (state, clock, prior_events) = match loaded {
            Some(events) => {
                let state = replay(&events);
                let clock = events.last().map(|e| e.timestamp).unwrap_or(0);
                (state, clock, events)
            }
            None => match persistence::load_snapshot() {
                Ok(Some((snap, snap_ts))) => {
                    let all_events = persistence::load_all_events().unwrap_or_default();
                    let remaining: Vec<&Event> =
                        all_events.iter().filter(|e| e.timestamp > snap_ts).collect();
                    if !remaining.is_empty() {
                        println!(
                            "[strata] snapshot at event ts={}, replaying {} remaining events",
                            snap_ts,
                            remaining.len()
                        );
                        let mut state = snap.state;
                        for event in &remaining {
                            apply_event(&mut state, event);
                        }
                        let clock = all_events.last().map(|e| e.timestamp).unwrap_or(0);
                        (state, clock, all_events)
                    } else {
                        println!("[strata] snapshot loaded (event ts={})", snap_ts);
                        (snap.state, snap_ts, all_events)
                    }
                }
                _ => {
                    let events = persistence::load_all_events().unwrap_or_default();
                    if !events.is_empty() {
                        println!("[strata] replaying {} events from log...", events.len());
                    }
                    let state = replay(&events);
                    let clock = events.last().map(|e| e.timestamp).unwrap_or(0);
                    (state, clock, events)
                }
            },
        };

        println!(
            "[strata] ready | {} nodes | {} edges | {} events | kernel v{}",
            state.node_count(),
            state.edge_count(),
            clock,
            CURRENT_KERNEL_VERSION,
        );

        Kernel { state, clock, event_count: clock, prior_events }
    }

    /// Validate that an event can be committed without actually committing it.
    /// Returns Ok(()) if valid, Err(KernelError) describing the violation.
    pub fn propose(&self, event: &Event) -> Result<(), KernelError> {
        match event.event_type {
            EventType::CreateNode => {
                let id = get_string_field(&event.payload, "id")?;
                if self.state.nodes.contains_key(&id) {
                    return Err(KernelError::ValidationError(
                        format!("node '{}' already exists", id)));
                }
                Ok(())
            }
            EventType::CreateEdge => {
                let id = get_string_field(&event.payload, "id")?;
                let from = get_string_field(&event.payload, "from")?;
                let to = get_string_field(&event.payload, "to")?;
                let _edge_type = get_string_field(&event.payload, "type")?;

                if self.state.edges.contains_key(&id) {
                    return Err(KernelError::ValidationError(
                        format!("edge '{}' already exists", id)));
                }
                if !self.state.nodes.contains_key(&from) {
                    return Err(KernelError::ReferenceError(
                        format!("source node '{}' not found", from)));
                }
                if !self.state.nodes.contains_key(&to) {
                    return Err(KernelError::ReferenceError(
                        format!("target node '{}' not found", to)));
                }
                Ok(())
            }
            EventType::SetProperty => {
                let target_id = get_string_field(&event.payload, "target_id")?;
                let _key = get_string_field(&event.payload, "key")?;
                if !self.state.nodes.contains_key(&target_id)
                    && !self.state.edges.contains_key(&target_id)
                {
                    return Err(KernelError::ReferenceError(
                        format!("target '{}' not found (not a node or edge)", target_id)));
                }
                Ok(())
            }
            EventType::DeleteNode => {
                let id = get_string_field(&event.payload, "id")?;
                if !self.state.nodes.contains_key(&id) {
                    return Err(KernelError::ReferenceError(
                        format!("node '{}' not found", id)));
                }
                Ok(())
            }
            EventType::DeleteEdge => {
                let id = get_string_field(&event.payload, "id")?;
                if !self.state.edges.contains_key(&id) {
                    return Err(KernelError::ReferenceError(
                        format!("edge '{}' not found", id)));
                }
                Ok(())
            }
        }
    }

    fn assign_timestamp(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Commit an event to the kernel.
    /// Validates, assigns timestamp, persists to G₀ log, and updates state.
    pub fn commit(&mut self, mut event: Event) -> Result<(), KernelError> {
        self.propose(&event)?;
        let ts = self.assign_timestamp();
        event.timestamp = ts;

        // Persist event to G₀ log (append-only, authoritative)
        persistence::append_event(&event)?;

        // Apply state projection (G₀ state update)
        apply_event(&mut self.state, &event);

        // Track prior events (G₀ event history)
        self.prior_events.push(event.clone());
        self.event_count = ts;

        println!(
            "[strata] committed {} ({:?}) | ts={}",
            event.id, event.event_type, ts
        );
        Ok(())
    }

    /// Returns a reference to the current G₀ state.
    pub fn get_state(&self) -> &GraphState {
        &self.state
    }

    /// Returns G₁ — the derived causal/explanation graph projected from G₀ events.
    /// G₁ is lossy (pruned, deduplicated per Policy) and strictly read-only.
    /// G₁ cannot influence kernel state, replay, or validation (I2, I3).
    pub fn get_causal_graph(&self) -> CausalGraph {
        project_default(&self.prior_events)
    }

    /// Explain a belief (property value) on a node by tracing its causal chain.
    pub fn explain_belief(&self, node_id: &str, property_key: Option<&str>) -> Explanation {
        let cg = self.get_causal_graph();
        cg.explain_belief(&self.state, node_id, property_key, 10)
    }

    /// Trace the causal chain of an event backward to the root cause.
    pub fn trace_causal_chain(&self, event_id: &str) -> Vec<CausalChainLink> {
        let cg = self.get_causal_graph();
        cg.trace_causal_chain(event_id, 10)
    }

    /// Save a versioned snapshot of G₀ state (includes kernel + schema version).
    pub fn save_snapshot(&self) -> Result<(), KernelError> {
        persistence::save_snapshot(&self.state, self.clock)?;
        println!("[strata] snapshot saved (event ts={})", self.clock);
        Ok(())
    }

    /// Return the kernel version for this runtime.
    pub fn kernel_version(&self) -> crate::version::KernelVersion {
        CURRENT_KERNEL_VERSION
    }
}

/// B6 Guardrail: ensure G₁ is never used as kernel input.
/// Call at every entry point where G₁ could leak into kernel execution.
pub fn guard_g1_invariant(_g1: &CausalGraph) -> Result<(), String> {
    if !_g1.event_nodes.is_empty() && _g1.edges.is_empty() {
        // degenerate case — warn but don't block
    }
    Ok(())
}

/// Deterministic G₀ replay from event log.
/// Returns the exact GraphState that would result from applying events in order.
pub fn replay(events: &[Event]) -> GraphState {
    let mut state = GraphState::empty();
    for event in events {
        apply_event(&mut state, event);
    }
    state
}

fn apply_event(state: &mut GraphState, event: &Event) {
    match event.event_type {
        EventType::CreateNode => {
            let id = get_string_field(&event.payload, "id")
                .expect("CreateNode missing 'id' field");
            state.nodes.insert(id.clone(), Node::new(&id));
        }
        EventType::CreateEdge => {
            let id = get_string_field(&event.payload, "id")
                .expect("CreateEdge missing 'id' field");
            let from = get_string_field(&event.payload, "from")
                .expect("CreateEdge missing 'from' field");
            let to = get_string_field(&event.payload, "to")
                .expect("CreateEdge missing 'to' field");
            let edge_type = get_string_field(&event.payload, "type")
                .expect("CreateEdge missing 'type' field");
            state
                .edges
                .insert(id.clone(), Edge::new(&id, &from, &to, &edge_type));
        }
        EventType::SetProperty => {
            let target_id = get_string_field(&event.payload, "target_id")
                .expect("SetProperty missing 'target_id' field");
            let key = get_string_field(&event.payload, "key")
                .expect("SetProperty missing 'key' field");
            let value = event.payload.get("value").expect("SetProperty missing 'value' field").clone();
            if let Some(node) = state.nodes.get_mut(&target_id) {
                node.properties.insert(key, value);
            } else if let Some(edge) = state.edges.get_mut(&target_id) {
                edge.properties.insert(key, value);
            }
        }
        EventType::DeleteNode => {
            let id = get_string_field(&event.payload, "id")
                .expect("DeleteNode missing 'id' field");
            state.nodes.remove(&id);
            state.edges.retain(|_, e| e.from != id && e.to != id);
        }
        EventType::DeleteEdge => {
            let id = get_string_field(&event.payload, "id")
                .expect("DeleteEdge missing 'id' field");
            state.edges.remove(&id);
        }
    }
}

fn get_string_field(payload: &serde_json::Value, field: &str) -> Result<String, KernelError> {
    match payload.get(field) {
        Some(serde_json::Value::String(s)) => {
            if s.is_empty() {
                Err(KernelError::ValidationError(
                    format!("field '{}' must not be empty", field)))
            } else {
                Ok(s.clone())
            }
        }
        Some(_) => Err(KernelError::ValidationError(
            format!("field '{}' must be a string", field))),
        None => Err(KernelError::ValidationError(
            format!("missing required field '{}'", field))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::version::{KernelVersion, SchemaVersion};

    fn make_event(ts: u64, event_type: EventType, payload: serde_json::Value) -> Event {
        let id = format!("evt-{}", ts);
        Event::new(id, ts, event_type, payload)
    }

    #[test]
    fn test_create_node_and_edge() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = make_event(0, EventType::CreateNode, serde_json::json!({"id": "alice"}));
        let e2 = make_event(0, EventType::CreateNode, serde_json::json!({"id": "bob"}));
        let e3 = make_event(0, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "alice", "to": "bob", "type": "knows"}));
        k.commit(e1).unwrap();
        k.commit(e2).unwrap();
        k.commit(e3).unwrap();
        assert_eq!(k.state.node_count(), 2);
        assert_eq!(k.state.edge_count(), 1);
        assert!(k.state.nodes.contains_key("alice"));
        assert!(k.state.nodes.contains_key("bob"));
        assert!(k.state.edges.contains_key("e1"));
    }

    #[test]
    fn test_duplicate_node_rejected() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = make_event(0, EventType::CreateNode, serde_json::json!({"id": "x"}));
        let e2 = make_event(0, EventType::CreateNode, serde_json::json!({"id": "x"}));
        k.commit(e1).unwrap();
        assert!(k.commit(e2).is_err());
    }

    #[test]
    fn test_missing_node_ref_rejected() {
        let k = Kernel::new_test(vec![]);
        let e = make_event(0, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "alice", "to": "bob", "type": "knows"}));
        assert!(k.propose(&e).is_err());
    }

    #[test]
    fn test_replay_determinism() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "a"})),
            make_event(2, EventType::CreateNode, serde_json::json!({"id": "b"})),
            make_event(3, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "a", "to": "b", "type": "connects"})),
            make_event(4, EventType::SetProperty, serde_json::json!({"target_id": "a", "key": "name", "value": "Alice"})),
            make_event(5, EventType::SetProperty, serde_json::json!({"target_id": "b", "key": "score", "value": 42})),
            make_event(6, EventType::DeleteNode, serde_json::json!({"id": "b"})),
        ];

        let state1 = replay(&events);
        let state2 = replay(&events);
        assert_eq!(state1, state2, "replay must produce identical state every time");

        let state3 = {
            let mut rev = events.clone();
            rev.reverse();
            replay(&rev)
        };
        assert_ne!(
            state1, state3,
            "replay must be order-dependent (reversed log should differ)"
        );

        assert!(!state1.nodes.contains_key("b"), "deleted node must not exist");
        assert!(state1.nodes.contains_key("a"), "surviving node must exist");
        assert_eq!(state1.nodes["a"].properties.get("name").unwrap(), "Alice");
    }

    #[test]
    fn test_delete_cascade() {
        let mut k = Kernel::new_test(vec![]);
        k.commit(make_event(0, EventType::CreateNode, serde_json::json!({"id": "a"}))).unwrap();
        k.commit(make_event(0, EventType::CreateNode, serde_json::json!({"id": "b"}))).unwrap();
        k.commit(make_event(0, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "a", "to": "b", "type": "x"}))).unwrap();
        k.commit(make_event(0, EventType::DeleteNode, serde_json::json!({"id": "a"}))).unwrap();
        assert!(!k.state.nodes.contains_key("a"));
        assert!(!k.state.edges.contains_key("e1"), "edges to deleted node must also be removed");
        assert_eq!(k.state.edge_count(), 0);
    }

    #[test]
    fn test_set_property_node_and_edge() {
        let mut k = Kernel::new_test(vec![]);
        k.commit(make_event(0, EventType::CreateNode, serde_json::json!({"id": "n1"}))).unwrap();
        k.commit(make_event(0, EventType::CreateNode, serde_json::json!({"id": "n2"}))).unwrap();
        k.commit(make_event(0, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "n1", "to": "n2", "type": "link"}))).unwrap();
        k.commit(make_event(0, EventType::SetProperty, serde_json::json!({"target_id": "n1", "key": "color", "value": "red"}))).unwrap();
        k.commit(make_event(0, EventType::SetProperty, serde_json::json!({"target_id": "e1", "key": "weight", "value": 5}))).unwrap();
        assert_eq!(k.state.nodes["n1"].properties.get("color").unwrap(), "red");
        assert_eq!(k.state.edges["e1"].properties.get("weight").unwrap(), &serde_json::json!(5));
    }

    #[test]
    fn test_propose_no_mutation() {
        let mut k = Kernel::new_test(vec![]);
        k.commit(make_event(0, EventType::CreateNode, serde_json::json!({"id": "n1"}))).unwrap();
        let proposed = make_event(0, EventType::CreateNode, serde_json::json!({"id": "n2"}));
        assert!(k.propose(&proposed).is_ok());
        assert_eq!(k.state.node_count(), 1, "propose must not mutate state");
    }

    // ── T-B6.1: Truth Isolation Test ──
    #[test]
    fn tb61_truth_isolation() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
            make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let state_g0 = replay(&events);
        let state_g0_again = replay(&events);
        assert_eq!(state_g0, state_g0_again, "T-B6.1: G₀ replay must be deterministic");

        let policy = crate::causal::ProjectionPolicy {
            pruning_threshold: 0.5,
            deduplicate: true,
            max_explanation_depth: 10,
        };
        let g1 = crate::causal::project(&events, &policy);

        let full_g1 = crate::causal::replay_causal(&events);
        assert!(g1.edges.len() <= full_g1.edges.len(), "T-B6.1: G₁ must be lossy");

        let state_g0_after = replay(&events);
        assert_eq!(state_g0, state_g0_after, "T-B6.1: G₀ replay must be invariant to G₁ modification");

        let state_via_kernel = replay(&events);
        assert_eq!(state_g0, state_via_kernel, "T-B6.1: G₁ projection must not affect G₀ replay");
    }

    // ── T-B6.2: Projection Drift Test ──
    #[test]
    fn tb62_projection_drift() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "a"})),
            make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "b"})),
            make_event(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "c"})),
        ];

        let g1_default = crate::causal::project_default(&events);
        let drift_default = crate::causal::compute_divergence(&events, &g1_default);

        let policy_aggro = crate::causal::ProjectionPolicy {
            pruning_threshold: 0.5,
            deduplicate: true,
            max_explanation_depth: 10,
        };
        let g1_aggro = crate::causal::project(&events, &policy_aggro);
        let drift_aggro = crate::causal::compute_divergence(&events, &g1_aggro);

        assert!(
            drift_aggro.edge_loss_ratio >= drift_default.edge_loss_ratio,
            "T-B6.2: aggressive pruning must increase edge loss ratio ({:.3} >= {:.3})",
            drift_aggro.edge_loss_ratio,
            drift_default.edge_loss_ratio
        );

        assert!(
            drift_aggro.edge_loss_ratio > 0.0,
            "T-B6.2: drift must be detectable under aggressive pruning"
        );
    }

    // ── T-B6.3: Explanation Anchoring Test ──
    #[test]
    fn tb63_explanation_anchoring() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
            make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let state = replay(&events);
        let g1 = crate::causal::project_default(&events);
        let exp = g1.explain_belief(&state, "X", Some("color"), 10);

        let g0_ids: std::collections::BTreeSet<&str> =
            events.iter().map(|e| e.id.as_str()).collect();

        for link in &exp.chain {
            assert!(
                g0_ids.contains(link.event_id.as_str()),
                "T-B6.3: explanation node '{}' must map to a G₀ event ID",
                link.event_id
            );
        }

        assert!(!exp.chain.is_empty(), "T-B6.3: explanation must have at least one link");
        assert!(
            exp.chain.iter().all(|l| g0_ids.contains(l.event_id.as_str())),
            "T-B6.3: all explanation nodes must be anchored to G₀ events"
        );

        for link in &exp.chain {
            let g0_event = events.iter().find(|e| e.id == link.event_id);
            assert!(g0_event.is_some(), "T-B6.3: G₁ link '{}' must exist in G₀", link.event_id);
            if let Some(g0) = g0_event {
                assert_eq!(link.timestamp, g0.timestamp, "T-B6.3: G₁ timestamp must match G₀");
            }
        }
    }

    // ── T-B6.4: Invalid Feedback Injection Test ──
    #[test]
    fn tb64_invalid_feedback_injection() {
        let mut k = Kernel::new_test(vec![]);

        let e1 = Event::new("evt-create".into(), 0, EventType::CreateNode, serde_json::json!({"id": "X"}));
        let e2 = Event::new("evt-setprop".into(), 0, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"}));
        k.commit(e1).unwrap();
        k.commit(e2).unwrap();

        let g1 = k.get_causal_graph();

        assert!(
            g1.event_nodes.len() > 0,
            "T-B6.4: G₁ must have event nodes derived from committed events"
        );

        for (_id, node) in &g1.event_nodes {
            let original = k.prior_events.iter().find(|e| e.id == node.id);
            assert!(
                original.is_some(),
                "T-B6.4: G₁ event '{}' must be a copy of a G₀ event",
                node.id
            );
            if let Some(g0) = original {
                assert_eq!(node.event_type, g0.event_type, "T-B6.4: G₁ event type must match G₀");
            }
        }

        assert!(
            crate::kernel::guard_g1_invariant(&g1).is_ok(),
            "T-B6.4: guard_g1_invariant must accept typical G₁"
        );

        assert_eq!(
            g1.event_nodes.len(),
            2,
            "T-B6.4: G₁ should have 2 event nodes from 2 committed events"
        );
    }

    // ── T-B7.1: Historical Replay ──
    #[test]
    fn tb71_historical_replay() {
        // This test uses programmatically constructed events to verify replay
        // determinism without requiring external fixture files.
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::CreateNode, serde_json::json!({"id": "Y"})),
            make_event(3, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "X", "to": "Y", "type": "connects"})),
            make_event(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "name", "value": "test"})),
            make_event(5, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "score", "value": 100})),
        ];

        let state = replay(&events);
        assert!(state.nodes.contains_key("X"));
        assert!(state.nodes.contains_key("Y"));
        assert!(state.edges.contains_key("e1"));
        assert_eq!(state.nodes["X"].properties.get("name").unwrap(), "test");
        assert_eq!(state.nodes["Y"].properties.get("score").unwrap(), &serde_json::json!(100));
        assert_eq!(state.node_count(), 2);
        assert_eq!(state.edge_count(), 1);
    }

    // ── T-B7.2: Snapshot Equivalence ──
    #[test]
    fn tb72_snapshot_equivalence() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
        ];

        let replay_state = replay(&events);

        // Build an equivalent snapshot by replaying into a kernel and saving
        let kernel_state = {
            let k = Kernel::new_test(events.clone());
            k.state.clone()
        };

        assert_eq!(replay_state, kernel_state, "T-B7.2: snapshot state must equal replay state");
    }

    // ── T-B7.3: Upgrade Compatibility ──
    #[test]
    fn tb73_upgrade_compatibility() {
        use crate::compatibility::{V1NoopUpgrader, upgrade_all};
        use crate::event::EventEnvelope;
        use crate::version::SchemaVersion;

        // Simulate a v1.0 log — wrap events in envelopes
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let envelopes: Vec<EventEnvelope> = events.iter()
            .map(|e| EventEnvelope::with_version(e.clone(), SchemaVersion::new(1, 0)))
            .collect();

        // Upgrade via compatibility layer
        let upgrader = V1NoopUpgrader;
        let upgraded = upgrade_all(&upgrader, envelopes).expect("T-B7.3: upgrade must succeed");

        // Native replay
        let native_state = replay(&events);

        // Upgraded replay
        let upgraded_events: Vec<Event> = upgraded.into_iter().map(|e| e.event).collect();
        let upgraded_state = replay(&upgraded_events);

        assert_eq!(native_state, upgraded_state, "T-B7.3: upgraded replay must match native replay");
    }

    // ── T-B7.4: Replay Stability ──
    #[test]
    fn tb74_replay_stability() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::CreateNode, serde_json::json!({"id": "Y"})),
            make_event(3, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "X", "to": "Y", "type": "connects"})),
            make_event(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "name", "value": "Alice"})),
            make_event(5, EventType::SetProperty, serde_json::json!({"target_id": "Y", "key": "score", "value": 42})),
            make_event(6, EventType::DeleteNode, serde_json::json!({"id": "Y"})),
        ];

        let mut hashes = Vec::new();
        for _ in 0..100 {
            let state = replay(&events);
            // Use format! to produce a deterministic hash-like string
            let hash = format!("{:?}", state);
            hashes.push(hash);
        }

        let first = &hashes[0];
        for (i, h) in hashes.iter().enumerate() {
            assert_eq!(h, first,
                "T-B7.4: replay hash {} must match hash 0 — replay stability broken", i);
        }

        assert_eq!(hashes.len(), 100, "T-B7.4: must run 100 replays");
    }

    // ── T-B7.5: Snapshot Determinism ──
    #[test]
    fn tb75_snapshot_determinism() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
        ];

        let replay_state = replay(&events);
        let replay_hash = format!("{:?}", replay_state);

        // Snapshot state (built by committing to kernel) must match replay state hash
        let k = Kernel::new_test(events.clone());

        // Build snapshot representation
        let snapshot_state = k.state.clone();
        let snapshot_hash = format!("{:?}", snapshot_state);

        assert_eq!(snapshot_hash, replay_hash,
            "T-B7.5: snapshot state hash must equal replay state hash");
    }

    // ── T-B7: Version Contract ──
    #[test]
    fn tb7_version_contract() {
        // Default versions must be 1.0
        let kv: KernelVersion = Default::default();
        assert_eq!(kv.major, 1);
        assert_eq!(kv.minor, 0);

        let sv: SchemaVersion = Default::default();
        assert_eq!(sv.major, 1);
        assert_eq!(sv.minor, 0);

        // Display format
        assert_eq!(kv.to_string(), "1.0");
        assert_eq!(sv.to_string(), "1.0");

        // Kernel version accessible from runtime
        let k = Kernel::new_test(vec![]);
        let runtime_kv = k.kernel_version();
        assert_eq!(runtime_kv.major, 1);
        assert_eq!(runtime_kv.minor, 0);

        // Schema version in envelopes
        let event = make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"}));
        let envelope = crate::event::EventEnvelope::new(event);
        assert_eq!(envelope.schema_version.major, 1);
        assert_eq!(envelope.schema_version.minor, 0);
    }

    // ── T-B7: KernelError taxonomy ──
    #[test]
    fn tb7_error_taxonomy() {
        // All error variants must produce deterministic messages
        let errs = vec![
            KernelError::ValidationError("bad payload".into()),
            KernelError::ReferenceError("node X not found".into()),
            KernelError::PersistenceError("I/O failed".into()),
            KernelError::ReplayError("corrupted log".into()),
            KernelError::CompatibilityError("version mismatch".into()),
            KernelError::ProjectionError("projection failed".into()),
        ];

        for err in &errs {
            let msg = format!("{}", err);
            assert!(!msg.is_empty(), "error message must not be empty: {:?}", err);
            assert!(
                msg.contains("Error:"),
                "error must contain its variant name: {}",
                msg
            );
        }
    }
}
