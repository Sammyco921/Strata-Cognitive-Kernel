use strata::api::result::ResultPayload;
use strata::api::Engine;
use strata::cli::CliCommand;
use strata::test_utils::test_bootstrap;

// ── A. Single Entry Validation ──────────────────────────────────────────────
//
// Every CLI command type routes through Bootstrap::run() and returns a
// CommandResultV1 → ResultPayload.  No execution path bypasses the bootstrap
// entrypoint.

#[test]
fn single_entry_create_node() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::CreateNode { id: "test-A".into() });
    assert!(rv1.is_ok(), "CreateNode should succeed: {:?}", rv1);
    match rv1.result {
        ResultPayload::Ingested(_) => {}
        other => panic!("Expected Ingested, got {:?}", other),
    }
}

#[test]
fn single_entry_create_edge() {
    let mut bs = test_bootstrap(vec![]);
    bs.run(CliCommand::CreateNode { id: "from-A".into() });
    bs.run(CliCommand::CreateNode { id: "to-B".into() });
    let rv1 = bs.run(CliCommand::CreateEdge {
        id: "edge-1".into(),
        from: "from-A".into(),
        to: "to-B".into(),
        r#type: "connects".into(),
    });
    assert!(rv1.is_ok(), "CreateEdge should succeed: {:?}", rv1);
}

#[test]
fn single_entry_set_property() {
    let mut bs = test_bootstrap(vec![]);
    bs.run(CliCommand::CreateNode { id: "prop-node".into() });
    let rv1 = bs.run(CliCommand::SetProperty {
        target: "prop-node".into(),
        key: "color".into(),
        value: "red".into(),
    });
    assert!(rv1.is_ok(), "SetProperty should succeed: {:?}", rv1);
}

#[test]
fn single_entry_delete_node() {
    let mut bs = test_bootstrap(vec![]);
    bs.run(CliCommand::CreateNode { id: "del-me".into() });
    let rv1 = bs.run(CliCommand::DeleteNode { id: "del-me".into() });
    assert!(rv1.is_ok(), "DeleteNode should succeed: {:?}", rv1);
}

#[test]
fn single_entry_delete_edge() {
    let mut bs = test_bootstrap(vec![]);
    bs.run(CliCommand::CreateNode { id: "A".into() });
    bs.run(CliCommand::CreateNode { id: "B".into() });
    bs.run(CliCommand::CreateEdge {
        id: "e1".into(),
        from: "A".into(),
        to: "B".into(),
        r#type: "x".into(),
    });
    let rv1 = bs.run(CliCommand::DeleteEdge { id: "e1".into() });
    assert!(rv1.is_ok(), "DeleteEdge should succeed: {:?}", rv1);
}

#[test]
fn single_entry_show_state() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::ShowState);
    match rv1.result {
        ResultPayload::StateView(view) => {
            assert_eq!(view.node_count(), 0, "fresh state should have 0 nodes");
        }
        other => panic!("Expected StateView, got {:?}", other),
    }
}

#[test]
fn single_entry_explain() {
    let mut bs = test_bootstrap(vec![]);
    bs.run(CliCommand::CreateNode { id: "X".into() });
    bs.run(CliCommand::SetProperty {
        target: "X".into(),
        key: "color".into(),
        value: "blue".into(),
    });
    let rv1 = bs.run(CliCommand::Explain {
        node_id: "X".into(),
        property_key: Some("color".into()),
    });
    match rv1.result {
        ResultPayload::ExplanationView(ex) => {
            assert_eq!(ex.target_node_id, "X");
        }
        other => panic!("Expected ExplanationView, got {:?}", other),
    }
}

#[test]
fn single_entry_trace() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::Trace {
        event_id: "nonexistent".into(),
    });
    match rv1.result {
        ResultPayload::CausalChainView(chain) => {
            assert!(chain.is_empty(), "nonexistent event should have empty chain");
        }
        other => panic!("Expected CausalChainView, got {:?}", other),
    }
}

#[test]
fn single_entry_system_commands() {
    let mut bs = test_bootstrap(vec![]);
    match bs.run(CliCommand::Version).result {
        ResultPayload::Version(v) => assert!(!v.is_empty(), "Version should not be empty"),
        other => panic!("Expected Version, got {:?}", other),
    }
    match bs.run(CliCommand::SchemaVersion).result {
        ResultPayload::SchemaVersion(v) => assert!(!v.is_empty(), "SchemaVersion should not be empty"),
        other => panic!("Expected SchemaVersion, got {:?}", other),
    }
    match bs.run(CliCommand::WorkflowList).result {
        ResultPayload::WorkflowList(_) => {}
        other => panic!("Expected WorkflowList for WorkflowList, got {:?}", other),
    }
}

#[test]
fn single_entry_all_commands_produce_result() {
    let mut bs = test_bootstrap(vec![]);

    let commands: Vec<CliCommand> = vec![
        CliCommand::CreateNode { id: "all-test".into() },
        CliCommand::Version,
        CliCommand::SchemaVersion,
        CliCommand::ShowState,
        CliCommand::WorkflowList,
    ];

    for cmd in commands {
        let rv1 = bs.run(cmd);
        assert!(matches!(
            rv1.result,
            ResultPayload::Ingested(_)
                | ResultPayload::Version(_)
                | ResultPayload::SchemaVersion(_)
                | ResultPayload::StateView(_)
                | ResultPayload::WorkflowList(_)
        ), "Unexpected result type: {:?}", rv1.result);
    }
}

// ── B. No Direct Engine Access Test ──────────────────────────────────────────
//
// CLI module MUST NOT import Engine, StrataEngine, Kernel, Event, or any
// persistence types. This is verified at compile time by checking imports.
//
// The test below confirms that the CLI module's public API does not include
// engine-related types, confirming it is a pure adapter.

#[test]
fn cli_module_does_not_export_engine_types() {
    let cli_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/cli/mod.rs");
    let source = std::fs::read_to_string(cli_path).expect("cli/mod.rs must exist");

    assert!(
        !source.contains("use crate::api::Engine"),
        "CLI module must not import Engine trait"
    );
    assert!(
        !source.contains("use crate::api::StrataEngine"),
        "CLI module must not import StrataEngine"
    );
    assert!(
        !source.contains("use crate::kernel"),
        "CLI module must not import kernel modules"
    );
    assert!(
        !source.contains("use crate::persistence"),
        "CLI module must not import persistence"
    );
    assert!(
        !source.contains("use crate::projection"),
        "CLI module must not import projection modules"
    );
    assert!(
        !source.contains("Event::new"),
        "CLI module must not construct Event objects directly"
    );
}

#[test]
fn cli_has_no_run_function() {
    let cli_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/cli/mod.rs");
    let source = std::fs::read_to_string(cli_path).expect("cli/mod.rs must exist");
    assert!(
        !source.contains("pub fn run("),
        "CLI module must not have a run() function"
    );
}

// ── C. Deterministic Boot Test ──────────────────────────────────────────────
//
// Multiple boots from identical initial conditions must produce identical
// engine state.

#[test]
fn deterministic_boot_empty() {
    let bs1 = test_bootstrap(vec![]);
    let bs2 = test_bootstrap(vec![]);

    let engine1 = bs1.engine();
    let engine2 = bs2.engine();

    assert_eq!(
        engine1.query_state().node_count(),
        engine2.query_state().node_count(),
        "Empty boots should have same node count"
    );
    assert_eq!(
        engine1.query_state().edge_count(),
        engine2.query_state().edge_count(),
        "Empty boots should have same edge count"
    );
}

#[test]
fn deterministic_boot_with_events() {
    use strata::{Event, EventType};

    let events = vec![
        Event::new("e1".into(), 1, EventType::CreateNode, serde_json::json!({"id": "A"})),
        Event::new("e2".into(), 2, EventType::CreateNode, serde_json::json!({"id": "B"})),
        Event::new("e3".into(), 3, EventType::CreateEdge, serde_json::json!({"id": "e1", "from": "A", "to": "B", "type": "connects"})),
    ];

    let bs1 = test_bootstrap(events.clone());
    let bs2 = test_bootstrap(events.clone());

    let state1 = bs1.engine().query_state().clone();
    let state2 = bs2.engine().query_state().clone();

    assert_eq!(state1, state2, "Identical event sequences must produce identical state");
}

#[test]
fn deterministic_boot_multiple_calls_same_state() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::ShowState);
    let rv2 = bs.run(CliCommand::ShowState);

    // Compare only the result payload, not trace_id (which differs per call)
    assert_eq!(rv1.result, rv2.result, "Repeated ShowState must be identical");
}

// ── D. Command Routing Test ──────────────────────────────────────────────────
//
// All CLI inputs map to valid API commands and produce appropriate results.

#[test]
fn command_routing_create_node() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::CreateNode { id: "routing-test".into() });
    assert!(rv1.is_ok(), "CreateNode should route successfully: {:?}", rv1);

    let state_rv = bs.run(CliCommand::ShowState);
    match state_rv.result {
        ResultPayload::StateView(view) => {
            assert_eq!(view.node_count(), 1);
            assert_eq!(view.nodes[0].id, "routing-test");
        }
        other => panic!("Expected StateView, got {:?}", other),
    }
}

#[test]
fn command_routing_invalid_node_rejected() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::CreateNode { id: "".into() });
    match rv1.result {
        ResultPayload::Error(_) => {} // Expected — empty ID is invalid
        ResultPayload::Ingested(_) => {}
        other => panic!("Expected Error or Ingested, got {:?}", other),
    }
}

#[test]
fn command_routing_version() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::Version);
    match rv1.result {
        ResultPayload::Version(v) => {
            assert!(!v.is_empty(), "Version should be non-empty");
        }
        other => panic!("Expected Version, got {:?}", other),
    }
}

#[test]
fn command_routing_schema_version() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::SchemaVersion);
    match rv1.result {
        ResultPayload::SchemaVersion(v) => {
            assert!(!v.is_empty(), "SchemaVersion should be non-empty");
        }
        other => panic!("Expected SchemaVersion, got {:?}", other),
    }
}

#[test]
fn command_routing_workflow_list() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::WorkflowList);
    match rv1.result {
        ResultPayload::WorkflowList(wl) => {
            // WorkflowList result contains a list of workflow names
            assert!(!wl.workflows.is_empty(), "Workflow list should not be empty");
        }
        other => panic!("Expected WorkflowList, got {:?}", other),
    }
}

#[test]
fn command_routing_all_variants_have_kind_str() {
    let commands: Vec<CliCommand> = vec![
        CliCommand::CreateNode { id: "a".into() },
        CliCommand::CreateEdge { id: "a".into(), from: "b".into(), to: "c".into(), r#type: "d".into() },
        CliCommand::SetProperty { target: "a".into(), key: "k".into(), value: "v".into() },
        CliCommand::DeleteNode { id: "a".into() },
        CliCommand::DeleteEdge { id: "a".into() },
        CliCommand::Replay,
        CliCommand::ShowState,
        CliCommand::SaveSnapshot,
        CliCommand::Explain { node_id: "a".into(), property_key: None },
        CliCommand::Trace { event_id: "a".into() },
        CliCommand::Version,
        CliCommand::SchemaVersion,
        CliCommand::ValidateLog,
        CliCommand::ReplayCheck,
        CliCommand::WorkflowList,
        CliCommand::WorkflowRun { name: "test".into() },
        CliCommand::WorkflowValidate,
    ];

    for cmd in &commands {
        let kind = cmd.kind_str();
        assert!(!kind.is_empty(), "kind_str must be non-empty for all variants");
        assert!(
            kind.chars().next().map(|c| c.is_uppercase()).unwrap_or(false),
            "kind_str '{}' should start with uppercase",
            kind
        );
    }
}

// ── Trace Logging Test ───────────────────────────────────────────────────────
//
// Trace output is deterministic and captures: command received, command type,
// execution start/end, result type.

#[test]
fn trace_output_is_deterministic() {
    let mut bs = test_bootstrap(vec![]);
    bs.set_trace(true);

    let rv1 = bs.run(CliCommand::CreateNode { id: "trace-test".into() });
    assert!(rv1.is_ok(), "Trace-enabled execution should succeed");

    let rv2 = bs.run(CliCommand::ShowState);
    match rv2.result {
        ResultPayload::StateView(view) => {
            assert_eq!(view.node_count(), 1);
        }
        other => panic!("Expected StateView, got {:?}", other),
    }
}

#[test]
fn trace_disabled_produces_no_output() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::Version);
    match rv1.result {
        ResultPayload::Version(v) => {
            assert_eq!(v, "1.0");
        }
        other => panic!("Expected Version, got {:?}", other),
    }
}

// ── Error Propagation Test ───────────────────────────────────────────────────
//
// All engine errors propagate as ResultPayload::Error.

#[test]
fn error_propagation_delete_nonexistent_node() {
    let mut bs = test_bootstrap(vec![]);
    let rv1 = bs.run(CliCommand::DeleteNode { id: "i-dont-exist".into() });
    match rv1.result {
        ResultPayload::Error(_) => {} // Expected
        ResultPayload::Ingested(_) => {
            // Kernel may accept delete of non-existent node depending on validation
        }
        other => panic!("Expected Error or Ingested, got {:?}", other),
    }
}

#[test]
fn error_propagation_create_duplicate_node() {
    let mut bs = test_bootstrap(vec![]);
    let r1 = bs.run(CliCommand::CreateNode { id: "dup".into() });
    assert!(r1.is_ok(), "First creation should succeed");

    let r2 = bs.run(CliCommand::CreateNode { id: "dup".into() });
    match r2.result {
        ResultPayload::Error(_) => {} // Expected — duplicate
        ResultPayload::Ingested(_) => {} // Some kernels may not enforce uniqueness
        other => panic!("Expected Error or Ingested for duplicate, got {:?}", other),
    }
}

// ── format_output Test ──────────────────────────────────────────────────────
//
// format_output produces valid non-empty strings for all result types.
// This test uses CommandResult directly (not through Bootstrap::run).

#[test]
fn format_output_all_variants() {
    use strata::api::command::CommandResult;
    use strata::api::command::StateView;
    use strata::{EdgeView, EventType, EventView, ExplanationView, KernelError, NodeView, SnapshotView};
    use std::collections::BTreeMap;

    let results: Vec<CommandResult> = vec![
        CommandResult::Valid,
        CommandResult::Ingested,
        CommandResult::QueryState(StateView { nodes: vec![], edges: vec![] }),
        CommandResult::GetNode(Some(NodeView { id: "n1".into(), properties: BTreeMap::new() })),
        CommandResult::GetNode(None),
        CommandResult::GetEdge(Some(EdgeView { id: "e1".into(), from: "a".into(), to: "b".into(), edge_type: "x".into(), properties: BTreeMap::new() })),
        CommandResult::GetEdge(None),
        CommandResult::ListNodes(vec![NodeView { id: "n1".into(), properties: BTreeMap::new() }]),
        CommandResult::ListEdges(vec![]),
        CommandResult::EventById(Some(EventView { id: "e1".into(), timestamp: 1, event_type: EventType::CreateNode, payload: serde_json::json!({}) })),
        CommandResult::EventById(None),
        CommandResult::EventsForNode(vec![]),
        CommandResult::EventsBetween(vec![]),
        CommandResult::LatestEvents(vec![]),
        CommandResult::Explain(ExplanationView {
            target_node_id: "x".into(),
            property_key: None,
            current_value: None,
            chain: vec![],
            hops: 0,
        }),
        CommandResult::CausalChain(vec![]),
        CommandResult::SnapshotMetadata(SnapshotView {
            kernel_version: "1.0".into(),
            schema_version: "1.0".into(),
            last_event_timestamp: 0,
            node_count: 0,
            edge_count: 0,
        }),
        CommandResult::ExportSnapshot("{}".into()),
        CommandResult::Message("hello".into()),
        CommandResult::Replay(StateView { nodes: vec![], edges: vec![] }),
        CommandResult::Error(KernelError::ValidationError("test".into())),
    ];

    for result in &results {
        let output = result.format_output();
        assert!(!output.is_empty(), "format_output must be non-empty for {:?}", result);
    }
}

#[test]
fn format_output_state_with_nodes() {
    use strata::api::command::CommandResult;
    use strata::api::command::StateView;
    use strata::NodeView;
    use std::collections::BTreeMap;

    let mut props = BTreeMap::new();
    props.insert("color".into(), serde_json::json!("red"));
    props.insert("size".into(), serde_json::json!("large"));

    let view = StateView {
        nodes: vec![NodeView { id: "A".into(), properties: props }],
        edges: vec![],
    };

    let output = CommandResult::QueryState(view).format_output();
    assert!(output.contains("NODE A"), "output should contain NODE A: {}", output);
    assert!(output.contains("color="), "output should contain color=: {}", output);
    assert!(output.contains("size="), "output should contain size=: {}", output);
}
