use serde_json::json;

use strata::api::command::{diff, Command, CommandClass, CommandExecutor, CommandResult};
use strata::api::Engine;
use strata::test_utils::{test_bootstrap, test_engine};
use strata::{Event, EventType, KernelError};

/// Helper to create a clean engine with some ingested events.
fn setup_engine() -> impl Engine {
    let mut engine = test_engine(vec![]);
    let evt1 = Event::new("e1".into(), 1, EventType::CreateNode, json!({"id": "A"}));
    let evt2 = Event::new("e2".into(), 2, EventType::SetProperty, json!({"target_id": "A", "key": "color", "value": "red"}));
    let evt3 = Event::new("e3".into(), 3, EventType::CreateNode, json!({"id": "B"}));
    let evt4 = Event::new("e4".into(), 4, EventType::CreateEdge, json!({"id": "e1", "from": "A", "to": "B", "type": "knows"}));
    engine.ingest_event(evt1).unwrap();
    engine.ingest_event(evt2).unwrap();
    engine.ingest_event(evt3).unwrap();
    engine.ingest_event(evt4).unwrap();
    engine
}

// ── A. Command Equivalence ───────────────────────────────────────────────────
//
// For every command, compare Engine-direct call vs Command execution.

#[test]
fn equivalence_ingest() {
    let event = Event::new("x".into(), 0, EventType::CreateNode, json!({"id": "X"}));

    let mut eng = test_engine(vec![]);
    let direct = eng.ingest_event(event.clone());

    let mut eng2 = test_engine(vec![]);
    let mut exec = CommandExecutor::new(&mut eng2);
    let cmd_result = exec.execute(Command::Ingest(event));

    match cmd_result {
        CommandResult::Ingested => {
            assert!(direct.is_ok(), "Ingest equivalence failed: direct={:?}", direct);
        }
        CommandResult::Error(e) => {
            assert!(direct.is_err(), "Ingest equivalence failed: got error={:?}", e);
        }
        _ => panic!("Expected Ingest result, got {:?}", cmd_result),
    }
}

#[test]
fn equivalence_validate() {
    let event = Event::new("x".into(), 0, EventType::CreateNode, json!({"id": "X"}));
    let bad_event = Event::new("bad".into(), 0, EventType::CreateNode, json!({"id": ""}));

    let engine = test_engine(vec![]);
    let direct_ok = engine.validate(&event);
    let direct_bad = engine.validate(&bad_event);

    let mut exec_engine = test_engine(vec![]);
    let mut exec = CommandExecutor::new(&mut exec_engine);

    let result_ok = exec.execute(Command::Validate(event));
    let result_bad = exec.execute(Command::Validate(bad_event));

    match result_ok {
        CommandResult::Valid => {
            assert!(direct_ok.is_ok(), "Validate OK equivalence failed");
        }
        CommandResult::Error(_e) => {
            assert!(direct_ok.is_err(), "Validate OK equivalence failed");
        }
        _ => panic!("Expected Valid result, got {:?}", result_ok),
    }

    match result_bad {
        CommandResult::Valid => {
            assert!(direct_bad.is_ok(), "Validate Bad equivalence failed");
        }
        CommandResult::Error(_) => {
            assert!(direct_bad.is_err(), "Validate bad equivalence failed");
        }
        _ => panic!("Expected Valid result, got {:?}", result_bad),
    }
}

#[test]
fn equivalence_query_state() {
    let engine = setup_engine();
    let direct_nodes = engine.list_nodes();
    let direct_edges = engine.list_edges();

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::QueryState);

    match result {
        CommandResult::QueryState(view) => {
            assert_eq!(view.nodes.len(), direct_nodes.len());
            assert_eq!(view.edges.len(), direct_edges.len());
            for nv in &view.nodes {
                let direct_nv = engine.get_node(&nv.id).expect("node should exist");
                assert_eq!(*nv, direct_nv, "Node mismatch: {}", nv.id);
            }
        }
        _ => panic!("Expected QueryState result, got {:?}", result),
    }
}

#[test]
fn equivalence_get_node() {
    let engine = setup_engine();
    let direct = engine.get_node("A");

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::GetNode("A".into()));

    match result {
        CommandResult::GetNode(n) => {
            assert_eq!(direct, n, "GetNode equivalence failed");
        }
        _ => panic!("Expected GetNode result"),
    }
}

#[test]
fn equivalence_get_edge() {
    let engine = setup_engine();
    let direct = engine.get_edge("e1");

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::GetEdge("e1".into()));

    match result {
        CommandResult::GetEdge(e) => {
            assert_eq!(direct, e, "GetEdge equivalence failed");
        }
        _ => panic!("Expected GetEdge result"),
    }
}

#[test]
fn equivalence_list_nodes() {
    let engine = setup_engine();
    let direct = engine.list_nodes();

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::ListNodes);

    match result {
        CommandResult::ListNodes(nodes) => {
            assert_eq!(direct, nodes, "ListNodes equivalence failed");
        }
        _ => panic!("Expected ListNodes result"),
    }
}

#[test]
fn equivalence_list_edges() {
    let engine = setup_engine();
    let direct = engine.list_edges();

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::ListEdges);

    match result {
        CommandResult::ListEdges(edges) => {
            assert_eq!(direct, edges, "ListEdges equivalence failed");
        }
        _ => panic!("Expected ListEdges result"),
    }
}

#[test]
fn equivalence_event_by_id() {
    let engine = setup_engine();
    let direct = engine.event_by_id("e2");

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::EventById("e2".into()));

    match result {
        CommandResult::EventById(ev) => {
            assert_eq!(direct, ev, "EventById equivalence failed");
        }
        _ => panic!("Expected EventById result"),
    }
}

#[test]
fn equivalence_events_for_node() {
    let engine = setup_engine();
    let direct = engine.events_for_node("A");

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::EventsForNode("A".into()));

    match result {
        CommandResult::EventsForNode(evts) => {
            assert_eq!(direct.len(), evts.len(), "EventsForNode length mismatch");
        }
        _ => panic!("Expected EventsForNode result"),
    }
}

#[test]
fn equivalence_events_between() {
    let engine = setup_engine();
    let direct = engine.events_between(2, 3);

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::EventsBetween { start: 2, end: 3 });

    match result {
        CommandResult::EventsBetween(evts) => {
            assert_eq!(direct.len(), evts.len(), "EventsBetween length mismatch");
        }
        _ => panic!("Expected EventsBetween result"),
    }
}

#[test]
fn equivalence_latest_events() {
    let engine = setup_engine();
    let direct = engine.latest_events(2);

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::LatestEvents(2));

    match result {
        CommandResult::LatestEvents(evts) => {
            assert_eq!(direct.len(), evts.len(), "LatestEvents length mismatch");
            if !direct.is_empty() && !evts.is_empty() {
                assert_eq!(direct[0].id, evts[0].id, "LatestEvents order mismatch");
            }
        }
        _ => panic!("Expected LatestEvents result"),
    }
}

#[test]
fn equivalence_explain() {
    let engine = setup_engine();
    let direct = engine.get_explanation("A", Some("color"));

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::Explain {
        node_id: "A".into(),
        property_key: Some("color".into()),
    });

    match result {
        CommandResult::Explain(ex) => {
            assert_eq!(direct.target_node_id, ex.target_node_id);
            assert_eq!(direct.property_key, ex.property_key);
            assert_eq!(direct.current_value, ex.current_value);
            assert_eq!(direct.hops, ex.hops);
        }
        _ => panic!("Expected Explain result"),
    }
}

#[test]
fn equivalence_causal_chain() {
    let engine = setup_engine();
    let direct = engine.causal_chain("e2");

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::CausalChain("e2".into()));

    match result {
        CommandResult::CausalChain(chain) => {
            assert_eq!(direct.len(), chain.len(), "CausalChain length mismatch");
        }
        _ => panic!("Expected CausalChain result"),
    }
}

#[test]
fn equivalence_snapshot_metadata() {
    let engine = setup_engine();
    let direct = engine.get_snapshot_metadata();

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::SnapshotMetadata);

    match result {
        CommandResult::SnapshotMetadata(meta) => {
            assert_eq!(direct.node_count, meta.node_count);
            assert_eq!(direct.edge_count, meta.edge_count);
            assert_eq!(direct.last_event_timestamp, meta.last_event_timestamp);
        }
        _ => panic!("Expected SnapshotMetadata result"),
    }
}

#[test]
fn equivalence_replay() {
    let engine = setup_engine();
    let events = vec![
        Event::new("r1".into(), 1, EventType::CreateNode, json!({"id": "X"})),
        Event::new("r2".into(), 2, EventType::SetProperty, json!({"target_id": "X", "key": "color", "value": "blue"})),
    ];
    let direct = engine.replay(&events);

    let mut eng = setup_engine();
    let mut exec = CommandExecutor::new(&mut eng);
    let result = exec.execute(Command::Replay(events));

    match result {
        CommandResult::Replay(state) => {
            assert_eq!(direct.node_count(), state.node_count(), "Replay node count mismatch");
            assert_eq!(direct.edge_count(), state.edge_count(), "Replay edge count mismatch");
        }
        _ => panic!("Expected Replay result"),
    }
}

// ── B. Determinism ───────────────────────────────────────────────────────────

#[test]
fn determinism_query_state() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);

    let r1 = exec.execute(Command::QueryState);
    let r2 = exec.execute(Command::QueryState);

    assert_eq!(r1, r2, "QueryState determinism violated");
}

#[test]
fn determinism_list_nodes() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);

    let r1 = exec.execute(Command::ListNodes);
    let r2 = exec.execute(Command::ListNodes);

    assert_eq!(r1, r2, "ListNodes determinism violated");
}

#[test]
fn determinism_explain() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);
    let cmd = Command::Explain {
        node_id: "A".into(),
        property_key: Some("color".into()),
    };

    let r1 = exec.execute(cmd.clone());
    let r2 = exec.execute(cmd.clone());

    assert_eq!(r1, r2, "Explain determinism violated");
}

#[test]
fn determinism_causal_chain() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);
    let cmd = Command::CausalChain("e2".into());

    let r1 = exec.execute(cmd.clone());
    let r2 = exec.execute(cmd.clone());

    assert_eq!(r1, r2, "CausalChain determinism violated");
}

#[test]
fn determinism_replay() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);
    let events = vec![
        Event::new("r1".into(), 1, EventType::CreateNode, json!({"id": "X"})),
    ];
    let cmd = Command::Replay(events);

    let r1 = exec.execute(cmd.clone());
    let r2 = exec.execute(cmd.clone());

    assert_eq!(r1, r2, "Replay determinism violated");
}

#[test]
fn determinism_diff() {
    let mut engine = setup_engine();
    let events_a = vec![
        Event::new("a1".into(), 1, EventType::CreateNode, json!({"id": "X"})),
    ];
    let events_b = vec![
        Event::new("b1".into(), 1, EventType::CreateNode, json!({"id": "Y"})),
    ];

    let r1 = diff(&mut engine, "A", events_a.clone(), "B", events_b.clone());
    let r2 = diff(&mut engine, "A", events_a, "B", events_b);

    assert_eq!(r1, r2, "Diff determinism violated");
}

// ── C. Boundary Enforcement ──────────────────────────────────────────────────

/// A minimal Engine implementation for testing executor independence.
struct MockEngine {
    nodes: Vec<String>,
}

impl MockEngine {
    fn new() -> Self {
        MockEngine { nodes: vec!["mock-A".into()] }
    }
}

impl Engine for MockEngine {
    fn validate(&self, _event: &Event) -> Result<(), KernelError> {
        Ok(())
    }

    fn ingest_event(&mut self, event: Event) -> Result<(), KernelError> {
        self.nodes.push(event.id.clone());
        Ok(())
    }

    fn replay(&self, _events: &[Event]) -> strata::GraphState {
        strata::GraphState::empty()
    }

    fn query_state(&self) -> &strata::GraphState {
        Box::leak(Box::new(strata::GraphState::empty()))
    }

    fn export_snapshot(&self) -> Result<String, KernelError> {
        Ok("{}".into())
    }

    fn get_node(&self, id: &str) -> Option<strata::NodeView> {
        if id == "mock-A" {
            Some(strata::NodeView {
                id: "mock-A".into(),
                properties: std::collections::BTreeMap::new(),
            })
        } else {
            None
        }
    }

    fn get_edge(&self, _id: &str) -> Option<strata::EdgeView> {
        None
    }

    fn list_nodes(&self) -> Vec<strata::NodeView> {
        vec![strata::NodeView {
            id: "mock-A".into(),
            properties: std::collections::BTreeMap::new(),
        }]
    }

    fn list_edges(&self) -> Vec<strata::EdgeView> {
        vec![]
    }

    fn event_by_id(&self, _id: &str) -> Option<strata::EventView> {
        None
    }

    fn events_for_node(&self, _node_id: &str) -> Vec<strata::EventView> {
        vec![]
    }

    fn events_between(&self, _start: u64, _end: u64) -> Vec<strata::EventView> {
        vec![]
    }

    fn latest_events(&self, _n: usize) -> Vec<strata::EventView> {
        vec![]
    }

    fn get_explanation(&self, node_id: &str, _property_key: Option<&str>) -> strata::ExplanationView {
        strata::ExplanationView {
            target_node_id: node_id.into(),
            property_key: None,
            current_value: None,
            chain: vec![],
            hops: 0,
        }
    }

    fn causal_chain(&self, _event_id: &str) -> Vec<strata::CausalChainLink> {
        vec![]
    }

    fn get_snapshot_metadata(&self) -> strata::SnapshotView {
        strata::SnapshotView {
            kernel_version: "1.0".into(),
            schema_version: "1.0".into(),
            last_event_timestamp: 0,
            node_count: 1,
            edge_count: 0,
        }
    }
}

#[test]
fn executor_works_with_any_engine() {
    let mut mock = MockEngine::new();
    let mut exec = CommandExecutor::new(&mut mock);

    let result = exec.execute(Command::GetNode("mock-A".into()));
    match result {
        CommandResult::GetNode(Some(node)) => {
            assert_eq!(node.id, "mock-A");
        }
        other => panic!("Expected GetNode(Some), got {:?}", other),
    }

    let event = Event::new("t".into(), 0, EventType::CreateNode, json!({"id": "T"}));
    let result = exec.execute(Command::Validate(event));
    match result {
        CommandResult::Valid => {}
        other => panic!("Expected Valid, got {:?}", other),
    }

    let result = exec.execute(Command::Replay(vec![]));
    match result {
        CommandResult::Replay(state) => {
            assert_eq!(state.node_count(), 0);
        }
        other => panic!("Expected Replay result, got {:?}", other),
    }
}

// ── D. DTO Purity ────────────────────────────────────────────────────────────
//
// CommandResult variants contain only public DTO types. No internal kernel types
// (GraphState, Node, Edge, EventEnvelope) may appear.

#[test]
fn dto_purity_state_result() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);
    let result = exec.execute(Command::QueryState);

    match result {
        CommandResult::QueryState(ref view) => {
            let debug_repr = format!("{:?}", view);
            assert!(!debug_repr.contains("GraphState"), "State result leaks GraphState");
            assert!(!debug_repr.contains("Node {"), "State result leaks internal Node");
        }
        _ => panic!("Expected QueryState result"),
    }
}

#[test]
fn dto_purity_replay_result() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);
    let events = vec![Event::new("x".into(), 0, EventType::CreateNode, json!({"id": "X"}))];
    let result = exec.execute(Command::Replay(events));

    match result {
        CommandResult::Replay(ref view) => {
            let debug_repr = format!("{:?}", view);
            assert!(!debug_repr.contains("GraphState"), "Replay result leaks GraphState");
        }
        _ => panic!("Expected Replay result"),
    }
}

#[test]
fn dto_purity_event_result() {
    let mut engine = setup_engine();
    let mut exec = CommandExecutor::new(&mut engine);
    let result = exec.execute(Command::EventById("e1".into()));

    match result {
        CommandResult::EventById(Some(ref ev)) => {
            let debug_repr = format!("{:?}", ev);
            assert!(!debug_repr.contains("EventEnvelope"), "Event result leaks EventEnvelope");
        }
        _ => panic!("Expected EventById(Some) result"),
    }
}

// ── Diff Tests ───────────────────────────────────────────────────────────────

#[test]
fn diff_identical_lists() {
    let mut engine = setup_engine();
    let events = vec![
        Event::new("d1".into(), 1, EventType::CreateNode, json!({"id": "D"})),
    ];
    let diff_result = diff(&mut engine, "same", events.clone(), "same", events);

    assert!(diff_result.states_equal, "Identical event lists should produce equal states");
    assert!(diff_result.nodes_only_in_a.is_empty());
    assert!(diff_result.nodes_only_in_b.is_empty());
}

#[test]
fn diff_different_lists() {
    let mut engine = setup_engine();
    let events_a = vec![
        Event::new("a1".into(), 1, EventType::CreateNode, json!({"id": "X"})),
    ];
    let events_b = vec![
        Event::new("b1".into(), 1, EventType::CreateNode, json!({"id": "Y"})),
    ];
    let diff_result = diff(&mut engine, "A", events_a, "B", events_b);

    assert!(!diff_result.states_equal, "Different event lists should produce different states");
    assert_eq!(diff_result.nodes_only_in_a, vec!["X"]);
    assert_eq!(diff_result.nodes_only_in_b, vec!["Y"]);
}

// ── E. Command Classification Integrity ───────────────────────────────────────
//
// Verifies I1–I4 from the Command Classification Integrity Layer:
//
//   I1 — Total Coverage: every Command variant returns a CommandClass
//   I2 — Exclusivity:    each variant belongs to exactly one class
//   I3 — Determinism:    class() is referentially transparent
//   I4 — Enforceability: exhaustive match in class() provides compile-time
//         coverage; this test suite provides runtime drift detection.
//
// If a new Command variant is added, the match in Command::class() will
// fail to compile until classified.  This test suite then double-checks
// the classification independently.

/// Every Command variant maps to exactly one expected CommandClass.
///
/// This test serves as a canonical registry of all command classifications.
/// Adding a new Command variant without updating this test is a detectable
/// gap (the variant will be missing from the assertions).
#[test]
fn classification_every_variant_maps_to_expected_class() {
    use strata::api::command::Command;

    // ── Execution ────────────────────────────────────────────────────────
    assert_eq!(
        Command::Ingest(
            Event::new("x".into(), 0, EventType::CreateNode, json!({"id": "X"}))
        ).class(),
        CommandClass::Execution,
        "Ingest must be Execution",
    );

    // ── Query ────────────────────────────────────────────────────────────
    assert_eq!(
        Command::Validate(
            Event::new("x".into(), 0, EventType::CreateNode, json!({"id": "X"}))
        ).class(),
        CommandClass::Query,
        "Validate must be Query",
    );
    assert_eq!(Command::Replay(vec![]).class(), CommandClass::Query, "Replay must be Query");
    assert_eq!(Command::QueryState.class(), CommandClass::Query, "QueryState must be Query");
    assert_eq!(
        Command::Explain { node_id: "x".into(), property_key: None }.class(),
        CommandClass::Query,
        "Explain must be Query",
    );
    assert_eq!(
        Command::CausalChain("x".into()).class(),
        CommandClass::Query,
        "CausalChain must be Query",
    );
    assert_eq!(Command::ExportSnapshot.class(), CommandClass::Query, "ExportSnapshot must be Query");
    assert_eq!(Command::GetNode("x".into()).class(), CommandClass::Query, "GetNode must be Query");
    assert_eq!(Command::GetEdge("x".into()).class(), CommandClass::Query, "GetEdge must be Query");
    assert_eq!(Command::ListNodes.class(), CommandClass::Query, "ListNodes must be Query");
    assert_eq!(Command::ListEdges.class(), CommandClass::Query, "ListEdges must be Query");
    assert_eq!(Command::EventById("x".into()).class(), CommandClass::Query, "EventById must be Query");
    assert_eq!(
        Command::EventsForNode("x".into()).class(),
        CommandClass::Query,
        "EventsForNode must be Query",
    );
    assert_eq!(
        Command::EventsBetween { start: 0, end: 1 }.class(),
        CommandClass::Query,
        "EventsBetween must be Query",
    );
    assert_eq!(Command::LatestEvents(5).class(), CommandClass::Query, "LatestEvents must be Query");
    assert_eq!(Command::SnapshotMetadata.class(), CommandClass::Query, "SnapshotMetadata must be Query");

    // ── System ───────────────────────────────────────────────────────────
    assert_eq!(Command::GetVersion.class(), CommandClass::System, "GetVersion must be System");
    assert_eq!(
        Command::GetSchemaVersion.class(),
        CommandClass::System,
        "GetSchemaVersion must be System",
    );
    assert_eq!(Command::ValidateLog.class(), CommandClass::System, "ValidateLog must be System");
    assert_eq!(Command::ReplayCheck.class(), CommandClass::System, "ReplayCheck must be System");
    assert_eq!(Command::WorkflowList.class(), CommandClass::System, "WorkflowList must be System");
    assert_eq!(
        Command::WorkflowRun("test".into()).class(),
        CommandClass::System,
        "WorkflowRun must be System",
    );
    assert_eq!(
        Command::WorkflowValidate.class(),
        CommandClass::System,
        "WorkflowValidate must be System",
    );
    assert_eq!(Command::ListCommands.class(), CommandClass::System, "ListCommands must be System");
    assert_eq!(
        Command::Describe("test".into()).class(),
        CommandClass::System,
        "Describe must be System",
    );
}

/// class() is deterministic: the same Command value always returns the
/// same CommandClass (I3).  Verified at runtime by calling twice.
#[test]
fn classification_deterministic() {
    let exe_cmd = Command::Ingest(
        Event::new("x".into(), 0, EventType::CreateNode, json!({"id": "X"})),
    );
    let qry_cmd = Command::QueryState;
    let sys_cmd = Command::GetVersion;

    // Call twice — must be identical
    assert_eq!(exe_cmd.class(), exe_cmd.class(), "Execution class must be deterministic");
    assert_eq!(qry_cmd.class(), qry_cmd.class(), "Query class must be deterministic");
    assert_eq!(sys_cmd.class(), sys_cmd.class(), "System class must be deterministic");
}

/// The Bootstrap dispatcher routes by class FIRST, then by variant.
/// This test verifies that system commands correctly bypass the engine
/// executor and that the executor catches any system command that slips
/// through (returning an error rather than executing).
#[test]
fn dispatch_routes_by_class() {
    use strata::api::result::ResultPayload;
    use strata::cli::CliCommand;


    let mut bs = test_bootstrap(vec![]);

    // System commands must NOT reach the executor.
    // Bootstrap::run() wraps in an envelope and calls execute(),
    // which dispatches by class → System arm → no engine call.
    let r1 = bs.run(CliCommand::Version);
    assert!(matches!(r1.result, ResultPayload::Version(_)), "Version must produce Version");

    let r2 = bs.run(CliCommand::SchemaVersion);
    assert!(matches!(r2.result, ResultPayload::SchemaVersion(_)), "SchemaVersion must produce SchemaVersion");

    let r3 = bs.run(CliCommand::WorkflowList);
    assert!(matches!(r3.result, ResultPayload::WorkflowList(_)), "WorkflowList must produce WorkflowList");
}

/// CommandExecutor catches system commands with an error (wildcard arm).
/// This test documents and verifies that behaviour.
#[test]
fn executor_rejects_system_commands() {
    let mut engine = test_engine(vec![]);
    let mut exec = CommandExecutor::new(&mut engine);

    let result = exec.execute(Command::GetVersion);
    match result {
        CommandResult::Error(_) => {} // Expected — system cmd not handled by executor
        other => panic!("Expected Error for system command in executor, got {:?}", other),
    }

    let result = exec.execute(Command::GetSchemaVersion);
    match result {
        CommandResult::Error(_) => {}
        other => panic!("Expected Error for system command in executor, got {:?}", other),
    }

    let result = exec.execute(Command::ListCommands);
    match result {
        CommandResult::Error(_) => {}
        other => panic!("Expected Error for system command in executor, got {:?}", other),
    }
}

/// No command belongs to more than one class (I2).
/// This is structurally guaranteed by the enum match in class().
#[test]
fn classification_exclusivity() {
    let exec_cmd = Command::Ingest(
        Event::new("x".into(), 0, EventType::CreateNode, json!({"id": "X"})),
    );
    let query_cmd = Command::QueryState;
    let sys_cmd = Command::GetVersion;

    assert_eq!(exec_cmd.class(), CommandClass::Execution);
    assert_ne!(exec_cmd.class(), CommandClass::Query);
    assert_ne!(exec_cmd.class(), CommandClass::System);

    assert_eq!(query_cmd.class(), CommandClass::Query);
    assert_ne!(query_cmd.class(), CommandClass::Execution);
    assert_ne!(query_cmd.class(), CommandClass::System);

    assert_eq!(sys_cmd.class(), CommandClass::System);
    assert_ne!(sys_cmd.class(), CommandClass::Execution);
    assert_ne!(sys_cmd.class(), CommandClass::Query);
}

#[test]
fn ingest_invalid_event_returns_error() {
    let mut engine = test_engine(vec![]);
    let mut exec = CommandExecutor::new(&mut engine);

    let evt1 = Event::new("e1".into(), 0, EventType::CreateNode, json!({"id": "dup"}));
    let ok = exec.execute(Command::Ingest(evt1));
    assert!(matches!(ok, CommandResult::Ingested));

    let evt2 = Event::new("e2".into(), 0, EventType::CreateNode, json!({"id": "dup"}));
    let err = exec.execute(Command::Ingest(evt2));
    match err {
        CommandResult::Error(KernelError::ValidationError(_)) => {}
        other => panic!("Expected Error(ValidationError), got {:?}", other),
    }
}

#[test]
fn validate_invalid_event() {
    let mut engine = test_engine(vec![]);
    let mut exec = CommandExecutor::new(&mut engine);

    let bad = Event::new("bad".into(), 0, EventType::CreateNode, json!({"id": ""}));
    let result = exec.execute(Command::Validate(bad));
    match result {
        CommandResult::Error(KernelError::ValidationError(_)) => {}
        other => panic!("Expected Error(ValidationError), got {:?}", other),
    }
}
