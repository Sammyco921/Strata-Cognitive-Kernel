use serde_json::json;

use strata::api::{Engine, StrataEngine};
use strata::{Event, EventType, KernelError};

/// Clean up persistence files left by snapshot export so tests are idempotent.
fn cleanup() {
    let _ = std::fs::remove_file("events.jsonl");
    let _ = std::fs::remove_file("causal.jsonl");
    let _ = std::fs::remove_file("snapshot.json");
}

/// Simulate a third-party developer consuming StrataEngine.
///
/// Rules enforced by import list above:
/// - NO direct kernel access
/// - NO direct projection access
/// - NO direct persistence access
/// - NO internal module imports
/// - Only public API surface (strata::api::* + crate root re-exports)
#[test]
fn full_lifecycle_through_strata_engine() {
    // Isolate from any left-over persistence from previous runs.
    cleanup();

    // ── 1. Create engine ──
    // Use from_events with empty list to avoid loading stale disk state.
    let mut engine = StrataEngine::from_events(vec![]);
    assert_eq!(engine.query_state().node_count(), 0);
    assert_eq!(engine.query_state().edge_count(), 0);

    // ── 2. Validate a valid event ──
    let evt = Event::new(
        "evt-1".into(),
        1,
        EventType::CreateNode,
        json!({"id": "A"}),
    );
    assert!(engine.validate(&evt).is_ok());

    // ── 3. Validate an invalid event (empty id) ──
    let bad_evt = Event::new(
        "evt-bad".into(),
        0,
        EventType::CreateNode,
        json!({"id": ""}),
    );
    assert!(engine.validate(&bad_evt).is_err());

    // ── 4. Ingest the valid event ──
    engine.ingest_event(evt).unwrap();

    // ── 5. Ingest a duplicate node → should fail ──
    let dup_evt = Event::new(
        "evt-dup".into(),
        2,
        EventType::CreateNode,
        json!({"id": "A"}),
    );
    let err = engine.ingest_event(dup_evt).unwrap_err();
    assert!(matches!(err, KernelError::ValidationError(_)));

    // ── 6. Ingest more events ──
    let evt2 = Event::new(
        "evt-2".into(),
        2,
        EventType::SetProperty,
        json!({"target_id": "A", "key": "color", "value": "blue"}),
    );
    engine.ingest_event(evt2).unwrap();

    let evt3 = Event::new(
        "evt-3".into(),
        3,
        EventType::CreateEdge,
        json!({"id": "e1", "from": "A", "to": "B", "type": "knows"}),
    );
    // B doesn't exist yet → ReferenceError
    let err = engine.ingest_event(evt3).unwrap_err();
    assert!(matches!(err, KernelError::ReferenceError(_)));

    let evt3b = Event::new(
        "evt-3b".into(),
        3,
        EventType::CreateNode,
        json!({"id": "B"}),
    );
    engine.ingest_event(evt3b).unwrap();

    let evt4 = Event::new(
        "evt-4".into(),
        4,
        EventType::CreateEdge,
        json!({"id": "e1", "from": "A", "to": "B", "type": "knows"}),
    );
    engine.ingest_event(evt4).unwrap();

    // ── 7. Query state ──
    let state = engine.query_state();
    assert_eq!(state.node_count(), 2);
    assert_eq!(state.edge_count(), 1);

    // ── 8. Generate explanation ──
    let explanation = engine.get_explanation("A", Some("color"));
    assert_eq!(explanation.target_node_id, "A");
    assert_eq!(explanation.property_key.as_deref(), Some("color"));
    assert!(!explanation.chain.is_empty());

    let a_node = state.nodes.get("A").expect("node A should exist");
    assert_eq!(
        a_node.properties.get("color"),
        Some(&serde_json::Value::String("blue".into()))
    );

    // ── 9. Export snapshot ──
    let snapshot_json = engine.export_snapshot().expect("snapshot should export");
    assert!(snapshot_json.contains("kernel_version"));
    assert!(snapshot_json.contains("schema_version"));
    assert!(snapshot_json.contains("A"));
    assert!(snapshot_json.contains("blue"));

    // ── 10. Replay state from events ──
    let replay_events = vec![
        Event::new("r1".into(), 1, EventType::CreateNode, json!({"id": "A"})),
        Event::new(
            "r2".into(),
            2,
            EventType::SetProperty,
            json!({"target_id": "A", "key": "color", "value": "blue"}),
        ),
        Event::new("r3".into(), 3, EventType::CreateNode, json!({"id": "B"})),
        Event::new(
            "r4".into(),
            4,
            EventType::CreateEdge,
            json!({"id": "e1", "from": "A", "to": "B", "type": "knows"}),
        ),
    ];
    let replayed = engine.replay(&replay_events);
    assert_eq!(replayed, *engine.query_state());

    // ── 11. `from_events` constructor ──
    let engine2 = StrataEngine::from_events(replay_events);
    assert_eq!(engine2.query_state().node_count(), 2);
    assert_eq!(engine2.query_state().edge_count(), 1);

    cleanup();
}

/// Exercise all query methods through the public Engine trait.
#[test]
fn query_methods_return_dtos() {
    cleanup();

    let mut engine = StrataEngine::from_events(vec![]);

    // Ingest events to create non-trivial state
    let evts = vec![
        Event::new("e1".into(), 1, EventType::CreateNode, json!({"id": "X"})),
        Event::new("e2".into(), 2, EventType::CreateNode, json!({"id": "Y"})),
        Event::new(
            "e3".into(),
            3,
            EventType::SetProperty,
            json!({"target_id": "X", "key": "color", "value": "red"}),
        ),
        Event::new(
            "e4".into(),
            4,
            EventType::CreateEdge,
            json!({"id": "e1", "from": "X", "to": "Y", "type": "knows"}),
        ),
    ];
    for e in evts {
        engine.ingest_event(e).unwrap();
    }

    // ── A. State Queries ──

    // get_node: existing
    let node = engine.get_node("X");
    assert!(node.is_some(), "get_node should find existing node");
    let node = node.unwrap();
    assert_eq!(node.id, "X");
    assert_eq!(
        node.properties.get("color"),
        Some(&json!("red"))
    );

    // get_node: missing
    assert!(engine.get_node("Z").is_none());

    // get_edge: existing
    let edge = engine.get_edge("e1");
    assert!(edge.is_some(), "get_edge should find existing edge");
    let edge = edge.unwrap();
    assert_eq!(edge.from, "X");
    assert_eq!(edge.to, "Y");
    assert_eq!(edge.edge_type, "knows");

    // get_edge: missing
    assert!(engine.get_edge("nonexistent").is_none());

    // list_nodes
    let nodes = engine.list_nodes();
    assert_eq!(nodes.len(), 2);
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"X"));
    assert!(ids.contains(&"Y"));

    // list_edges
    let edges = engine.list_edges();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].id, "e1");

    // ── B. History Queries ──

    // event_by_id
    let ev = engine.event_by_id("e1");
    assert!(ev.is_some());
    assert_eq!(ev.unwrap().event_type, EventType::CreateNode);

    assert!(engine.event_by_id("nonexistent").is_none());

    // events_for_node
    let x_events = engine.events_for_node("X");
    assert_eq!(x_events.len(), 3); // CreateNode, SetProperty, CreateEdge (from=X)

    let y_events = engine.events_for_node("Y");
    assert_eq!(y_events.len(), 2); // CreateNode, CreateEdge (to=Y)

    // events_between
    let range = engine.events_between(2, 3);
    assert_eq!(range.len(), 2); // ts=2: CreateNode(Y), ts=3: SetProperty

    // latest_events
    let latest = engine.latest_events(2);
    assert_eq!(latest.len(), 2);
    assert_eq!(latest[0].id, "e4"); // most recent first

    // ── C. Explanation Queries ──

    // get_explanation
    let ex = engine.get_explanation("X", Some("color"));
    assert_eq!(ex.target_node_id, "X");
    assert!(ex.hops > 0);
    assert!(!ex.chain.is_empty());
    assert_eq!(ex.current_value, Some(json!("red")));

    // causal_chain
    let chain = engine.causal_chain("e3");
    assert!(!chain.is_empty(), "e3 (SetProperty) should have a causal chain");

    // ── D. Snapshot Queries ──

    // get_snapshot_metadata
    let meta = engine.get_snapshot_metadata();
    assert_eq!(meta.node_count, 2);
    assert_eq!(meta.edge_count, 1);
    assert_eq!(meta.last_event_timestamp, 4);
    assert!(!meta.kernel_version.is_empty());
    assert!(!meta.schema_version.is_empty());

    cleanup();
}

/// Demonstrate the StrataEngine::new() constructor works with a clean
/// persistence directory (no pre-existing files).
#[test]
fn fresh_engine_starts_empty() {
    cleanup();
    let mut engine = StrataEngine::new();
    assert_eq!(engine.query_state().node_count(), 0);
    assert_eq!(engine.query_state().edge_count(), 0);

    let evt = Event::new("e1".into(), 1, EventType::CreateNode, json!({"id": "X"}));
    engine.ingest_event(evt).unwrap();
    assert_eq!(engine.query_state().node_count(), 1);

    cleanup();
}
