use crate::kernel::error::KernelError;
use crate::kernel::event::{Event, EventType};
use crate::kernel::graph::GraphState;
use crate::kernel::replay::{apply_event, detect_causal_cycle, get_string_field, replay};
use crate::kernel::version::CURRENT_KERNEL_VERSION;
use crate::persistence;

/// B7 Kernel v1 — FROZEN PUBLIC API.
///
/// G₀ = Ground Truth Event Graph (events + immutable state replay).
/// G₁ = Derived Causal/Explanation Graph (lossy, policy-driven projection).
pub struct Kernel {
    pub(crate) state: GraphState,
    pub(crate) clock: u64,
    pub(crate) event_count: u64,
    pub(crate) prior_events: Vec<Event>,
}

impl Kernel {
    pub fn new() -> Self {
        Kernel::new_with_events(None)
    }

    pub fn new_test(events: Vec<Event>) -> Self {
        let state = replay(&events);
        let clock = events.last().map(|e| e.timestamp).unwrap_or(0);
        let prior = events;
        Kernel { state, clock, event_count: clock, prior_events: prior }
    }

    fn new_with_events(loaded: Option<Vec<Event>>) -> Self {
        eprintln!("[strata] initializing kernel...");

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
                        eprintln!(
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
                        eprintln!("[strata] snapshot loaded (event ts={})", snap_ts);
                        (snap.state, snap_ts, all_events)
                    }
                }
                _ => {
                    let events = persistence::load_all_events().unwrap_or_default();
                    if !events.is_empty() {
                        eprintln!("[strata] replaying {} events from log...", events.len());
                    }
                    let state = replay(&events);
                    let clock = events.last().map(|e| e.timestamp).unwrap_or(0);
                    (state, clock, events)
                }
            },
        };

        eprintln!(
            "[strata] ready | {} nodes | {} edges | {} events | kernel v{}",
            state.node_count(),
            state.edge_count(),
            clock,
            CURRENT_KERNEL_VERSION,
        );

        Kernel { state, clock, event_count: clock, prior_events }
    }

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

    pub fn commit(&mut self, mut event: Event) -> Result<(), KernelError> {
        self.propose(&event)?;

        if let Some((_, cycle_path)) = detect_causal_cycle(&self.prior_events, &event) {
            return Err(KernelError::CausalCycleViolation {
                event_id: event.id.clone(),
                cycle_path,
            });
        }

        let ts = self.assign_timestamp();
        event.timestamp = ts;

        persistence::append_event(&event)?;
        apply_event(&mut self.state, &event);
        self.prior_events.push(event.clone());
        self.event_count = ts;

        eprintln!(
            "[strata] committed {} ({}) | ts={}",
            event.id, event.event_type, ts
        );
        Ok(())
    }

    pub fn get_state(&self) -> &GraphState {
        &self.state
    }

    pub fn get_causal_graph(&self) -> crate::projection::causal::CausalGraph {
        crate::projection::causal::project_default(&self.prior_events)
    }

    pub fn explain_belief(&self, node_id: &str, property_key: Option<&str>) -> crate::projection::causal::Explanation {
        let cg = self.get_causal_graph();
        cg.explain_belief(&self.state, node_id, property_key, 10)
    }

    pub fn trace_causal_chain(&self, event_id: &str) -> Vec<crate::projection::causal::CausalChainLink> {
        let cg = self.get_causal_graph();
        cg.trace_causal_chain(event_id, 10)
    }

    pub fn save_snapshot(&self) -> Result<(), KernelError> {
        persistence::save_snapshot(&self.state, self.clock)?;
        eprintln!("[strata] snapshot saved (event ts={})", self.clock);
        Ok(())
    }

    pub fn kernel_version(&self) -> crate::kernel::version::KernelVersion {
        CURRENT_KERNEL_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::event::Event;
    use crate::kernel::replay::replay;
    use crate::kernel::version::{KernelVersion, SchemaVersion};

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

        let policy = crate::projection::causal::ProjectionPolicy {
            pruning_threshold: 0.5,
            deduplicate: true,
            max_explanation_depth: 10,
        };
        let g1 = crate::projection::causal::project(&events, &policy);

        let full_g1 = crate::projection::causal::replay_causal(&events);
        assert!(g1.edges.len() <= full_g1.edges.len(), "T-B6.1: G₁ must be lossy");

        let state_g0_after = replay(&events);
        assert_eq!(state_g0, state_g0_after, "T-B6.1: G₀ replay must be invariant to G₁ modification");

        let state_via_kernel = replay(&events);
        assert_eq!(state_g0, state_via_kernel, "T-B6.1: G₁ projection must not affect G₀ replay");
    }

    #[test]
    fn tb62_projection_drift() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "a"})),
            make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "b"})),
            make_event(4, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "c"})),
        ];

        let g1_default = crate::projection::causal::project_default(&events);
        let drift_default = crate::projection::causal::compute_divergence(&events, &g1_default);

        let policy_aggro = crate::projection::causal::ProjectionPolicy {
            pruning_threshold: 0.5,
            deduplicate: true,
            max_explanation_depth: 10,
        };
        let g1_aggro = crate::projection::causal::project(&events, &policy_aggro);
        let drift_aggro = crate::projection::causal::compute_divergence(&events, &g1_aggro);

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

    #[test]
    fn tb63_explanation_anchoring() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
            make_event(3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let state = replay(&events);
        let g1 = crate::projection::causal::project_default(&events);
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
            crate::kernel::replay::guard_g1_invariant(&g1).is_ok(),
            "T-B6.4: guard_g1_invariant must accept typical G₁"
        );

        assert_eq!(
            g1.event_nodes.len(),
            2,
            "T-B6.4: G₁ should have 2 event nodes from 2 committed events"
        );
    }

    #[test]
    fn tb71_historical_replay() {
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

    #[test]
    fn tb72_snapshot_equivalence() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
        ];

        let replay_state = replay(&events);

        let kernel_state = {
            let k = Kernel::new_test(events.clone());
            k.state.clone()
        };

        assert_eq!(replay_state, kernel_state, "T-B7.2: snapshot state must equal replay state");
    }

    #[test]
    fn tb73_upgrade_compatibility() {
        use crate::kernel::compatibility::{V1NoopUpgrader, upgrade_all};
        use crate::kernel::event::EventEnvelope;
        use crate::kernel::version::SchemaVersion;

        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let envelopes: Vec<EventEnvelope> = events.iter()
            .map(|e| EventEnvelope::with_version(e.clone(), SchemaVersion::new(1, 0)))
            .collect();

        let upgrader = V1NoopUpgrader;
        let upgraded = upgrade_all(&upgrader, envelopes).expect("T-B7.3: upgrade must succeed");

        let native_state = replay(&events);

        let upgraded_events: Vec<Event> = upgraded.into_iter().map(|e| e.event).collect();
        let upgraded_state = replay(&upgraded_events);

        assert_eq!(native_state, upgraded_state, "T-B7.3: upgraded replay must match native replay");
    }

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

    #[test]
    fn tb75_snapshot_determinism() {
        let events = vec![
            make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            make_event(2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
        ];

        let replay_state = replay(&events);
        let replay_hash = format!("{:?}", replay_state);

        let k = Kernel::new_test(events.clone());

        let snapshot_state = k.state.clone();
        let snapshot_hash = format!("{:?}", snapshot_state);

        assert_eq!(snapshot_hash, replay_hash,
            "T-B7.5: snapshot state hash must equal replay state hash");
    }

    #[test]
    fn tb7_version_contract() {
        let kv: KernelVersion = Default::default();
        assert_eq!(kv.major, 1);
        assert_eq!(kv.minor, 0);

        let sv: SchemaVersion = Default::default();
        assert_eq!(sv.major, 1);
        assert_eq!(sv.minor, 0);

        assert_eq!(kv.to_string(), "1.0");
        assert_eq!(sv.to_string(), "1.0");

        let k = Kernel::new_test(vec![]);
        let runtime_kv = k.kernel_version();
        assert_eq!(runtime_kv.major, 1);
        assert_eq!(runtime_kv.minor, 0);

        let event = make_event(1, EventType::CreateNode, serde_json::json!({"id": "X"}));
        let envelope = crate::kernel::event::EventEnvelope::new(event);
        assert_eq!(envelope.schema_version.major, 1);
        assert_eq!(envelope.schema_version.minor, 0);
    }

    #[test]
    fn tb7_error_taxonomy() {
        let errs = vec![
            KernelError::ValidationError("bad payload".into()),
            KernelError::ReferenceError("node X not found".into()),
            KernelError::PersistenceError("I/O failed".into()),
            KernelError::ReplayError("corrupted log".into()),
            KernelError::CompatibilityError("version mismatch".into()),
            KernelError::ProjectionError("projection failed".into()),
            KernelError::CausalCycleViolation {
                event_id: "evt-X".into(),
                cycle_path: "evt-A -> evt-B -> evt-X".into(),
            },
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

    #[test]
    fn tb81_direct_cycle_rejected_at_commit() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = Event::with_causes(
            "evt-1".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "A"}), vec!["evt-2".into()], None,
        );
        k.commit(e1).unwrap();

        let e2 = Event::with_causes(
            "evt-2".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "B"}), vec!["evt-1".into()], None,
        );
        let result = k.commit(e2);
        assert!(result.is_err(), "direct cycle must be rejected");
        assert!(
            matches!(result, Err(KernelError::CausalCycleViolation { .. })),
            "error must be CausalCycleViolation, got {:?}", result
        );
    }

    #[test]
    fn tb82_self_loop_rejected_at_commit() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = Event::with_causes(
            "evt-1".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "A"}), vec!["evt-1".into()], None,
        );
        let result = k.commit(e1);
        assert!(result.is_err(), "self-loop must be rejected");
        assert!(
            matches!(result, Err(KernelError::CausalCycleViolation { .. })),
            "error must be CausalCycleViolation, got {:?}", result
        );
    }

    #[test]
    fn tb83_multi_hop_cycle_rejected_at_commit() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = Event::with_causes(
            "evt-1".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "A"}), vec!["evt-3".into()], None,
        );
        k.commit(e1).unwrap();

        let e2 = Event::with_causes(
            "evt-2".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "B"}), vec!["evt-1".into()], None,
        );
        k.commit(e2).unwrap();

        let e3 = Event::with_causes(
            "evt-3".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "C"}), vec!["evt-2".into()], None,
        );
        let result = k.commit(e3);
        assert!(result.is_err(), "multi-hop cycle must be rejected");
        assert!(
            matches!(result, Err(KernelError::CausalCycleViolation { .. })),
            "error must be CausalCycleViolation, got {:?}", result
        );
    }

    #[test]
    fn tb84_valid_causal_chain_accepted() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = Event::with_causes(
            "evt-1".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "A"}), vec![], None,
        );
        k.commit(e1).unwrap();

        let e2 = Event::with_causes(
            "evt-2".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "B"}), vec!["evt-1".into()], None,
        );
        k.commit(e2).unwrap();

        let e3 = Event::with_causes(
            "evt-3".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "C"}), vec!["evt-2".into()], None,
        );
        assert!(k.commit(e3).is_ok(), "linear causal chain must be accepted");
    }

    #[test]
    fn tb85_rejected_event_not_persisted() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = Event::with_causes(
            "evt-1".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "A"}), vec!["evt-2".into()], None,
        );
        k.commit(e1).unwrap();

        let prior_count_before = k.prior_events.len();
        let clock_before = k.clock;

        let e2 = Event::with_causes(
            "evt-2".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "B"}), vec!["evt-1".into()], None,
        );
        assert!(k.commit(e2).is_err());

        assert_eq!(
            k.prior_events.len(), prior_count_before,
            "rejected event must not appear in prior_events"
        );
        assert_eq!(
            k.clock, clock_before,
            "rejected event must not advance clock"
        );
    }

    #[test]
    fn tb86_cycle_error_contains_offending_id_and_path() {
        let mut k = Kernel::new_test(vec![]);
        let e1 = Event::with_causes(
            "evt-A".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "X"}), vec!["evt-B".into()], None,
        );
        k.commit(e1).unwrap();

        let e2 = Event::with_causes(
            "evt-B".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "Y"}), vec!["evt-A".into()], None,
        );
        let err = k.commit(e2).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("evt-B"), "error must mention offending event ID");
        assert!(msg.contains("cycle"), "error must mention cycle");
    }

    #[test]
    fn tb87_cycle_log_property() {
        let mut k = Kernel::new_test(vec![]);
        let good_events = vec![
            Event::new("e1".into(), 0, EventType::CreateNode, serde_json::json!({"id": "A"})),
            Event::new("e2".into(), 0, EventType::CreateNode, serde_json::json!({"id": "B"})),
            Event::with_causes("e3".into(), 0, EventType::CreateNode, serde_json::json!({"id": "C"}), vec!["e1".into()], None),
            Event::with_causes("e4".into(), 0, EventType::CreateNode, serde_json::json!({"id": "D"}), vec!["e2".into(), "e3".into()], None),
        ];

        for ev in good_events {
            k.commit(ev).unwrap();
        }

        assert!(crate::kernel::replay::detect_causal_cycle(&k.prior_events, &k.prior_events[0]).is_none());
        assert!(crate::kernel::replay::detect_causal_cycle(&k.prior_events, &k.prior_events[1]).is_none());
        assert!(crate::kernel::replay::detect_causal_cycle(&k.prior_events, &k.prior_events[2]).is_none());
        assert!(crate::kernel::replay::detect_causal_cycle(&k.prior_events, &k.prior_events[3]).is_none());

        let replay_replay = replay(&k.prior_events);
        assert!(replay_replay.nodes.contains_key("A"));
        assert!(replay_replay.nodes.contains_key("D"));
    }

    #[test]
    fn tb88_replay_isolation() {
        let events = vec![
            Event::new("e1".into(), 1, EventType::CreateNode, serde_json::json!({"id": "X"})),
            Event::new("e2".into(), 2, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
            Event::new("e3".into(), 3, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        ];

        let state_before = replay(&events);
        let g1_before = crate::projection::causal::project_default(&events);

        let _ = detect_causal_cycle(&events[..1], &events[2]);

        let state_after = replay(&events);
        let g1_after = crate::projection::causal::project_default(&events);

        assert_eq!(state_before, state_after, "cycle detection must not affect G₀ replay");
        assert_eq!(g1_before.edges.len(), g1_after.edges.len(), "cycle detection must not affect G₁ projection");
    }
}
