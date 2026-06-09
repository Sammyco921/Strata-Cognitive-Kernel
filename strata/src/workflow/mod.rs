use crate::api::{Engine, StrataEngine};
use crate::kernel::event::{Event, EventType};
use crate::persistence::ProductionPersister;
use crate::test_utils::test_engine;

pub fn list() -> Vec<&'static str> {
    vec!["event-lifecycle", "investigation", "debugging", "snapshot-recovery", "audit"]
}

pub fn run(name: &str) -> bool {
    match name {
        "event-lifecycle" => workflow_event_lifecycle(),
        "investigation" => workflow_investigation(),
        "debugging" => workflow_debugging(),
        "snapshot-recovery" => workflow_snapshot_recovery(),
        "audit" => workflow_audit(),
        _ => {
            eprintln!("  unknown workflow: {}", name);
            false
        }
    }
}

fn make_event(id: &str, ts: u64, event_type: EventType, payload: serde_json::Value) -> Event {
    Event::new(id.to_string(), ts, event_type, payload)
}

// ── Workflow A: Event Lifecycle ──────────────────────────────────────────────

fn workflow_event_lifecycle() -> bool {
    let mut engine = test_engine(vec![]);

    let evt = make_event("evt-A1", 0, EventType::CreateNode, serde_json::json!({"id": "A"}));
    if engine.validate(&evt).is_err() {
        eprintln!("  FAIL: validation of valid event failed");
        return false;
    }
    if engine.ingest_event(evt).is_err() {
        eprintln!("  FAIL: ingest of valid event failed");
        return false;
    }
    if engine.query_state().node_count() != 1 {
        eprintln!("  FAIL: expected 1 node after create, got {}", engine.query_state().node_count());
        return false;
    }

    let bad = make_event("evt-A2", 0, EventType::CreateNode, serde_json::json!({"id": ""}));
    if engine.validate(&bad).is_ok() {
        eprintln!("  FAIL: validation of bad event should have failed");
        return false;
    }

    true
}

// ── Workflow B: Investigation ────────────────────────────────────────────────

fn workflow_investigation() -> bool {
    let events = vec![
        make_event("evt-0", 0, EventType::CreateNode, serde_json::json!({"id": "X"})),
        make_event("evt-1", 0, EventType::CreateNode, serde_json::json!({"id": "Y"})),
        make_event("evt-2", 0, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "red"})),
        make_event("evt-3", 0, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "size", "value": "large"})),
        make_event("evt-4", 0, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "X", "to": "Y", "type": "connects"})),
    ];

    let mut engine = test_engine(vec![]);
    for e in events {
        if engine.ingest_event(e).is_err() {
            eprintln!("  FAIL: ingest failed during investigation");
            return false;
        }
    }

    // Query state
    if engine.query_state().node_count() != 2 {
        eprintln!("  FAIL: expected 2 nodes, got {}", engine.query_state().node_count());
        return false;
    }

    // Explain node X's color
    let ex = engine.get_explanation("X", Some("color"));
    if ex.hops == 0 || ex.chain.is_empty() {
        eprintln!("  FAIL: explanation for X:color should have causal chain");
        return false;
    }

    // Causal chain
    let chain = engine.causal_chain("evt-3");
    if chain.is_empty() {
        eprintln!("  FAIL: set-property event should have causal chain");
        return false;
    }

    true
}

// ── Workflow C: Debugging ────────────────────────────────────────────────────

fn workflow_debugging() -> bool {
    let raw = vec![
        make_event("evt-0", 0, EventType::CreateNode, serde_json::json!({"id": "X"})),
        make_event("evt-1", 0, EventType::CreateNode, serde_json::json!({"id": "Y"})),
        make_event("evt-2", 0, EventType::SetProperty, serde_json::json!({"target_id": "X", "key": "color", "value": "blue"})),
        make_event("evt-3", 0, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "X", "to": "Y", "type": "knows"})),
    ];

    // Ingest into engine
    let mut engine = test_engine(vec![]);
    for e in &raw {
        if engine.ingest_event(e.clone()).is_err() {
            eprintln!("  FAIL: ingest failed during debugging");
            return false;
        }
    }

    // Replay from same events and compare
    let replayed = engine.replay(&raw);
    if replayed != *engine.query_state() {
        eprintln!("  FAIL: replayed state does not match engine state");
        return false;
    }

    // Reverse order should produce different state
    let mut rev = raw.clone();
    rev.reverse();
    let replayed_rev = engine.replay(&rev);
    if replayed_rev == replayed {
        eprintln!("  FAIL: reversed replay should differ");
        return false;
    }

    true
}

// ── Workflow D: Snapshot Recovery ────────────────────────────────────────────

fn workflow_snapshot_recovery() -> bool {
    // Isolate from any previous persistence state
    let _ = std::fs::remove_file("events.jsonl");
    let _ = std::fs::remove_file("snapshot.json");

    let raw = vec![
        make_event("evt-0", 0, EventType::CreateNode, serde_json::json!({"id": "snap-A"})),
        make_event("evt-1", 0, EventType::SetProperty, serde_json::json!({"target_id": "snap-A", "key": "status", "value": "active"})),
    ];

    let mut engine = test_engine(vec![]);
    for e in raw {
        if engine.ingest_event(e).is_err() {
            eprintln!("  FAIL: ingest failed during snapshot recovery");
            return false;
        }
    }

    // Grab baseline state
    let baseline_node_count = engine.query_state().node_count();
    let baseline_edge_count = engine.query_state().edge_count();

    // Export snapshot (saves to disk)
    if engine.export_snapshot().is_err() {
        eprintln!("  FAIL: export snapshot failed");
        return false;
    }

    // Create a new engine — it loads from the snapshot on disk
    let engine2 = StrataEngine::new(ProductionPersister);
    if engine2.query_state().node_count() != baseline_node_count {
        eprintln!("  FAIL: restored node count {} != expected {}", engine2.query_state().node_count(), baseline_node_count);
        return false;
    }
    if engine2.query_state().edge_count() != baseline_edge_count {
        eprintln!("  FAIL: restored edge count mismatch");
        return false;
    }

    // Clean up snapshot file
    let _ = std::fs::remove_file("snapshot.json");

    true
}

// ── Workflow E: Audit ────────────────────────────────────────────────────────

fn workflow_audit() -> bool {
    let raw = vec![
        make_event("evt-0", 0, EventType::CreateNode, serde_json::json!({"id": "AuditNode"})),
        make_event("evt-1", 0, EventType::SetProperty, serde_json::json!({"target_id": "AuditNode", "key": "version", "value": "1"})),
        make_event("evt-2", 0, EventType::SetProperty, serde_json::json!({"target_id": "AuditNode", "key": "version", "value": "2"})),
        make_event("evt-3", 0, EventType::SetProperty, serde_json::json!({"target_id": "AuditNode", "key": "status", "value": "complete"})),
        make_event("evt-4", 0, EventType::CreateNode, serde_json::json!({"id": "Other"})),
    ];

    let mut engine = test_engine(vec![]);
    for e in &raw {
        if engine.ingest_event(e.clone()).is_err() {
            eprintln!("  FAIL: ingest failed during audit");
            return false;
        }
    }

    // List events for the audited entity
    let audit_events = engine.events_for_node("AuditNode");
    if audit_events.len() != 4 {
        eprintln!("  FAIL: expected 4 events for AuditNode, got {}", audit_events.len());
        return false;
    }

    // Verify timestamps are distinct and ascending
    let timestamps: Vec<u64> = audit_events.iter().map(|e| e.timestamp).collect();
    for i in 1..timestamps.len() {
        if timestamps[i] <= timestamps[i - 1] {
            eprintln!("  FAIL: timestamps not monotonically increasing in audit trail");
            return false;
        }
    }

    // Generate explanation for final state
    let ex = engine.get_explanation("AuditNode", Some("status"));
    if ex.chain.is_empty() {
        eprintln!("  FAIL: explanation for AuditNode:status should have chain");
        return false;
    }

    // Determinism: replay should produce identical state
    let state_a = engine.replay(&raw);
    let state_b = engine.replay(&raw);
    if state_a != state_b {
        eprintln!("  FAIL: audit replay determinism broken");
        return false;
    }

    true
}
