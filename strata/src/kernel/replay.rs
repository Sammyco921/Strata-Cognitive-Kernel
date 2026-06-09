use crate::kernel::error::KernelError;
use crate::kernel::event::{Event, EventType};
use crate::kernel::graph::{Edge, GraphState, Node};

/// Deterministic G₀ replay from event log.
/// Returns the exact GraphState that would result from applying events in order.
pub fn replay(events: &[Event]) -> GraphState {
    let mut state = GraphState::empty();
    for event in events {
        apply_event(&mut state, event);
    }
    state
}

pub fn apply_event(state: &mut GraphState, event: &Event) {
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

pub fn get_string_field(payload: &serde_json::Value, field: &str) -> Result<String, KernelError> {
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

/// Extract G₀-authoritative causal edges from an event.
///
/// Returns `Vec<(cause_event_id, dependent_event_id)>` tuples derived
/// exclusively from the event's declared `causes` field.
///
/// This is the only valid source of causal edges for cycle detection:
/// - MUST only read `event.causes` (or equivalent canonical declaration field)
/// - MUST NOT include projected, inferred, or normalization-derived edges
/// - MUST be deterministic (sorted output)
pub fn extract_g0_causal_edges(event: &Event) -> Vec<(String, String)> {
    let mut edges: Vec<(String, String)> = event
        .causes
        .iter()
        .map(|cause| (cause.clone(), event.id.clone()))
        .collect();
    edges.sort();
    edges
}

/// Pure, deterministic cycle detection for causal edges.
///
/// Builds the adjacency graph exclusively via [`extract_g0_causal_edges`],
/// ensuring only G₀ event-declared causal relations participate in validation.
/// Projected, inferred, or normalization-derived edges are never included.
///
/// Pipeline:
///   events → extract_g0_causal_edges → adjacency graph → DFS cycle detection
///
/// Returns `None` if no cycle would be introduced.
/// Returns `Some((candidate_event_id, cycle_path_string))` if a cycle is detected.
pub fn detect_causal_cycle<'a>(
    existing_events: impl IntoIterator<Item = &'a Event>,
    candidate: &Event,
) -> Option<(String, String)> {
    use std::collections::{BTreeMap, BTreeSet};

    let all_events: Vec<&Event> = existing_events.into_iter().collect();

    let edges: Vec<(String, String)> = all_events
        .iter()
        .flat_map(|ev| extract_g0_causal_edges(ev))
        .chain(extract_g0_causal_edges(candidate))
        .collect();

    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut all_nodes: BTreeSet<&str> = BTreeSet::new();

    for (cause, event_id) in &edges {
        adjacency.entry(cause.as_str()).or_default().push(event_id.as_str());
        all_nodes.insert(cause.as_str());
        all_nodes.insert(event_id.as_str());
    }

    // 0 = unvisited, 1 = in current DFS stack, 2 = fully visited
    let mut state: BTreeMap<&str, u8> = BTreeMap::new();
    let mut path_stack: Vec<&str> = Vec::new();
    let mut found_stack: Vec<&str> = Vec::new();

    fn dfs<'b>(
        node: &'b str,
        adj: &BTreeMap<&'b str, Vec<&'b str>>,
        state: &mut BTreeMap<&'b str, u8>,
        path: &mut Vec<&'b str>,
        found: &mut Vec<&'b str>,
    ) -> bool {
        match state.get(node).copied().unwrap_or(0) {
            2 => return false,
            1 => {
                found.clear();
                if let Some(pos) = path.iter().position(|n| *n == node) {
                    found.extend_from_slice(&path[pos..]);
                }
                found.push(node);
                return true;
            }
            _ => {}
        }

        state.insert(node, 1);
        path.push(node);

        if let Some(neighbors) = adj.get(node) {
            for n in neighbors {
                if dfs(n, adj, state, path, found) {
                    return true;
                }
            }
        }

        state.insert(node, 2);
        path.pop();

        false
    }

    for node in all_nodes.iter() {
        if state.get(node).copied().unwrap_or(0) == 0 {
            if dfs(node, &adjacency, &mut state, &mut path_stack, &mut found_stack) {
                let path_str = found_stack.join(" -> ");
                return Some((candidate.id.clone(), path_str));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::event::EventType;
    use crate::kernel::event::Event;

    fn make_event(id: &str, ts: u64, event_type: EventType, payload: serde_json::Value, causes: Vec<String>) -> Event {
        let mut e = Event::new(id.to_string(), ts, event_type, payload);
        e.causes = causes;
        e
    }

    #[test]
    fn test_no_cycle_without_causes() {
        let existing = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "A"}), vec![]),
            make_event("evt-2", 2, EventType::CreateNode, serde_json::json!({"id": "B"}), vec!["evt-1".into()]),
        ];
        let candidate = make_event("evt-3", 0, EventType::CreateNode, serde_json::json!({"id": "C"}), vec!["evt-2".into()]);
        assert!(detect_causal_cycle(&existing, &candidate).is_none());
    }

    #[test]
    fn test_direct_cycle_detected() {
        let existing = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "A"}), vec!["evt-2".into()]),
        ];
        let candidate = make_event("evt-2", 0, EventType::CreateNode, serde_json::json!({"id": "B"}), vec!["evt-1".into()]);
        let result = detect_causal_cycle(&existing, &candidate);
        assert!(result.is_some(), "direct cycle must be detected");
        let (id, path) = result.unwrap();
        assert_eq!(id, "evt-2");
        assert!(path.contains("evt-1"), "path must contain evt-1");
        assert!(path.contains("evt-2"), "path must contain evt-2");
    }

    #[test]
    fn test_self_loop_detected() {
        let existing: Vec<Event> = vec![];
        let candidate = make_event("evt-1", 0, EventType::CreateNode, serde_json::json!({"id": "A"}), vec!["evt-1".into()]);
        let result = detect_causal_cycle(&existing, &candidate);
        assert!(result.is_some(), "self-loop must be detected");
    }

    #[test]
    fn test_multi_hop_cycle_detected() {
        let existing = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "A"}), vec!["evt-3".into()]),
            make_event("evt-2", 2, EventType::CreateNode, serde_json::json!({"id": "B"}), vec!["evt-1".into()]),
        ];
        let candidate = make_event("evt-3", 0, EventType::CreateNode, serde_json::json!({"id": "C"}), vec!["evt-2".into()]);
        let result = detect_causal_cycle(&existing, &candidate);
        assert!(result.is_some(), "multi-hop cycle must be detected");
    }

    #[test]
    fn test_cycle_detection_deterministic() {
        let existing = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "A"}), vec!["evt-2".into()]),
        ];
        let candidate = make_event("evt-2", 0, EventType::CreateNode, serde_json::json!({"id": "B"}), vec!["evt-1".into()]);

        let results: Vec<_> = (0..100).map(|_| {
            detect_causal_cycle(&existing, &candidate).map(|(id, path)| (id, path))
        }).collect();

        let first = &results[0];
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r, first, "cycle detection must be deterministic (run {})", i);
        }
    }

    #[test]
    fn test_candidate_not_in_existing_is_fine() {
        let existing = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "A"}), vec![]),
            make_event("evt-2", 2, EventType::CreateNode, serde_json::json!({"id": "B"}), vec!["evt-1".into()]),
        ];
        let candidate = make_event("evt-3", 0, EventType::CreateNode, serde_json::json!({"id": "C"}), vec![]);
        assert!(detect_causal_cycle(&existing, &candidate).is_none());
    }

    #[test]
    fn test_candidate_with_ref_to_nonexistent_is_fine() {
        let existing = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "A"}), vec![]),
        ];
        let candidate = make_event("evt-2", 0, EventType::CreateNode, serde_json::json!({"id": "B"}), vec!["evt-nonexistent".into()]);
        assert!(detect_causal_cycle(&existing, &candidate).is_none(), "reference to nonexistent event is not a cycle");
    }

    #[test]
    fn test_diamond_dependency_no_cycle() {
        let existing = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "root"}), vec![]),
            make_event("evt-2", 2, EventType::CreateNode, serde_json::json!({"id": "A"}), vec!["evt-1".into()]),
            make_event("evt-3", 3, EventType::CreateNode, serde_json::json!({"id": "B"}), vec!["evt-1".into()]),
        ];
        let candidate = make_event("evt-4", 0, EventType::CreateNode, serde_json::json!({"id": "C"}), vec!["evt-2".into(), "evt-3".into()]);
        assert!(detect_causal_cycle(&existing, &candidate).is_none(), "diamond dependency is not a cycle");
    }

    #[test]
    fn test_replay_determinism_unchanged() {
        let events = vec![
            make_event("evt-1", 1, EventType::CreateNode, serde_json::json!({"id": "X"}), vec![]),
            make_event("evt-2", 2, EventType::CreateNode, serde_json::json!({"id": "Y"}), vec!["evt-1".into()]),
        ];
        let state1 = replay(&events);
        let state2 = replay(&events);
        assert_eq!(state1, state2, "replay must still be deterministic");
    }

    // ── extract_g0_causal_edges ─────────────────────────────────────────

    #[test]
    fn test_extract_g0_causal_edges_empty() {
        let e = Event::new("e1".into(), 0, EventType::CreateNode, serde_json::json!({"id": "A"}));
        let edges = extract_g0_causal_edges(&e);
        assert!(edges.is_empty(), "event with no causes must produce no edges");
    }

    #[test]
    fn test_extract_g0_causal_edges_single() {
        let mut e = Event::new("e1".into(), 0, EventType::CreateNode, serde_json::json!({"id": "A"}));
        e.causes = vec!["evt-root".into()];
        let edges = extract_g0_causal_edges(&e);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0], ("evt-root".to_string(), "e1".to_string()));
    }

    #[test]
    fn test_extract_g0_causal_edges_multiple_sorted() {
        let mut e = Event::new("evt-child".into(), 0, EventType::CreateNode, serde_json::json!({"id": "A"}));
        e.causes = vec!["evt-Z".into(), "evt-A".into(), "evt-M".into()];
        let edges = extract_g0_causal_edges(&e);
        assert_eq!(edges.len(), 3);
        // Must be sorted for determinism
        for pair in edges.windows(2) {
            assert!(pair[0] <= pair[1], "edges must be in sorted order");
        }
    }

    #[test]
    fn test_extract_g0_causal_edges_ignores_meta_reason() {
        let e = Event::with_causes(
            "evt-2".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "B"}),
            vec!["evt-1".into()],
            Some("simulated projection edge".into()),
        );
        let edges = extract_g0_causal_edges(&e);
        assert_eq!(edges.len(), 1, "meta_reason must not affect edge extraction");
        assert_eq!(edges[0], ("evt-1".to_string(), "evt-2".to_string()));
    }

    // ── Test A: Projection Ignorance ────────────────────────────────────

    #[test]
    fn test_projection_ignorance() {
        // G₁ would infer Contradicts relationships between SetProperty events
        // that overwrite the same key, creating a cycle in projection space.
        // But detect_causal_cycle strictly reads only Event.causes — no cycle.
        let a = make_event("evt-create", 1, EventType::CreateNode,
            serde_json::json!({"id": "X"}), vec![]);
        let b = make_event("evt-set-1", 2, EventType::SetProperty,
            serde_json::json!({"target_id": "X", "key": "color", "value": "red"}), vec![]);
        let c = make_event("evt-set-2", 3, EventType::SetProperty,
            serde_json::json!({"target_id": "X", "key": "color", "value": "blue"}), vec![]);

        // No events declare explicit causes → no edges → no cycle
        assert!(
            detect_causal_cycle(&[a, b], &c).is_none(),
            "projection-only edges must not affect cycle detection"
        );
    }

    #[test]
    fn test_projection_ignorance_with_derivesfrom_mask() {
        // Even when meta_reason suggests a causal relationship, the
        // explicit 'causes' field is the sole source of truth.
        let existing = vec![
            Event::with_causes("evt-1".into(), 1, EventType::CreateNode,
                serde_json::json!({"id": "X"}),
                vec![],
                Some("G₁ infers: evt-2 causes evt-1".into())),
        ];
        let candidate = Event::with_causes("evt-2".into(), 0, EventType::CreateNode,
            serde_json::json!({"id": "Y"}),
            vec![],
            Some("G₁ infers: evt-1 causes evt-2".into()));

        // meta_reason alone must not create edges in cycle detection
        assert!(
            detect_causal_cycle(&existing, &candidate).is_none(),
            "meta_reason must not be treated as a causal edge"
        );
    }

    // ── Test B: G₀ Exclusivity ──────────────────────────────────────────

    #[test]
    fn test_g0_exclusivity_no_cycle_when_g1_would_cycle() {
        // G₁ would form a cycle:
        //   evt-set-2 Contradicts evt-set-1
        //   evt-set-1 Contradicts evt-set-2  (if replayed symmetrically)
        //
        // G₀ causes: evt-set-1 ← evt-create, evt-set-2 ← evt-set-1  (acyclic chain)
        let create = make_event("evt-create", 1, EventType::CreateNode,
            serde_json::json!({"id": "X"}), vec![]);
        let set1 = make_event("evt-set-1", 2, EventType::SetProperty,
            serde_json::json!({"target_id": "X", "key": "color", "value": "red"}),
            vec!["evt-create".into()]);
        let set2 = make_event("evt-set-2", 3, EventType::SetProperty,
            serde_json::json!({"target_id": "X", "key": "color", "value": "blue"}),
            vec!["evt-set-1".into()]);

        // G₀ edge: evt-create → evt-set-1 → evt-set-2  (acyclic)
        assert!(
            detect_causal_cycle(&[create, set1], &set2).is_none(),
            "G₀-acyclic event set must not produce cycle violation even if G₁ would cycle"
        );
    }

    #[test]
    fn test_g0_exclusivity_cycle_only_in_g0() {
        // Only G₀ causes can produce a cycle violation. If the causes field
        // is acyclic, no violation occurs regardless of G₁ structure.
        let a = make_event("evt-A", 1, EventType::CreateNode,
            serde_json::json!({"id": "X"}), vec!["evt-B".into()]);
        let b = make_event("evt-B", 2, EventType::CreateNode,
            serde_json::json!({"id": "Y"}), vec!["evt-A".into()]);

        // G₀ cycle: evt-A → evt-B → evt-A (via explicit causes)
        let result = detect_causal_cycle(&[a], &b);
        assert!(result.is_some(), "G₀ cycle must be detected");
    }

    // ── Test C: Causal Minimality ───────────────────────────────────────

    #[test]
    fn test_causal_minimality_only_causes_contribute() {
        // Payload fields like "from", "to", "target_id" must NOT be treated
        // as causal edges. Only the explicit 'causes' field contributes.
        let existing = vec![
            make_event("evt-node", 1, EventType::CreateNode,
                serde_json::json!({"id": "A"}), vec![]),
        ];
        let candidate = Event::with_causes("evt-edge".into(), 0, EventType::CreateEdge,
            serde_json::json!({"id": "e1", "from": "A", "to": "B", "type": "knows"}),
            vec![],
            None,
        );

        // "from": "A" in payload must not create an implicit causal edge
        assert!(
            detect_causal_cycle(&existing, &candidate).is_none(),
            "payload fields must not create implicit causal edges"
        );
    }

    #[test]
    fn test_causal_minimality_payload_relations_not_edges() {
        // SetProperty targeting a node must not create an implicit edge
        // from that node's CreateNode event.
        let existing = vec![
            make_event("evt-create", 1, EventType::CreateNode,
                serde_json::json!({"id": "X"}), vec![]),
        ];
        let candidate = Event::with_causes("evt-set".into(), 0, EventType::SetProperty,
            serde_json::json!({"target_id": "X", "key": "color", "value": "red"}),
            vec![],
            None,
        );

        assert!(
            detect_causal_cycle(&existing, &candidate).is_none(),
            "target_id in payload must not create implicit causal edge"
        );
    }

    #[test]
    fn test_causal_minimality_delete_does_not_infer_edge() {
        // DeleteNode must not create an implicit edge to prior events
        // that targeted the deleted node.
        let existing = vec![
            make_event("evt-create", 1, EventType::CreateNode,
                serde_json::json!({"id": "X"}), vec![]),
            make_event("evt-set", 2, EventType::SetProperty,
                serde_json::json!({"target_id": "X", "key": "color", "value": "red"}), vec![]),
        ];
        let candidate = make_event("evt-delete", 0, EventType::DeleteNode,
            serde_json::json!({"id": "X"}), vec![]);

        assert!(
            detect_causal_cycle(&existing, &candidate).is_none(),
            "DeleteNode payload must not create implicit edges"
        );
    }
}
