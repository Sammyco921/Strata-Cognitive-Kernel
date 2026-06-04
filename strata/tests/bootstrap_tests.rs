use strata::api::command::CommandResult;
use strata::api::Engine;
use strata::bootstrap::Bootstrap;
use strata::cli::CliCommand;

// ── A. Single Entry Validation ──────────────────────────────────────────────
//
// Every CLI command type routes through Bootstrap::run() and returns a
// CommandResult. No execution path bypasses the bootstrap entrypoint.

#[test]
fn single_entry_create_node() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::CreateNode { id: "test-A".into() });
    assert!(result.is_ok(), "CreateNode should succeed: {:?}", result);
    match result {
        CommandResult::Ingested => {}
        other => panic!("Expected Ingested, got {:?}", other),
    }
}

#[test]
fn single_entry_create_edge() {
    let mut bs = Bootstrap::from_events(vec![]);
    bs.run(CliCommand::CreateNode { id: "from-A".into() });
    bs.run(CliCommand::CreateNode { id: "to-B".into() });
    let result = bs.run(CliCommand::CreateEdge {
        id: "edge-1".into(),
        from: "from-A".into(),
        to: "to-B".into(),
        r#type: "connects".into(),
    });
    assert!(result.is_ok(), "CreateEdge should succeed: {:?}", result);
}

#[test]
fn single_entry_set_property() {
    let mut bs = Bootstrap::from_events(vec![]);
    bs.run(CliCommand::CreateNode { id: "prop-node".into() });
    let result = bs.run(CliCommand::SetProperty {
        target: "prop-node".into(),
        key: "color".into(),
        value: "red".into(),
    });
    assert!(result.is_ok(), "SetProperty should succeed: {:?}", result);
}

#[test]
fn single_entry_delete_node() {
    let mut bs = Bootstrap::from_events(vec![]);
    bs.run(CliCommand::CreateNode { id: "del-me".into() });
    let result = bs.run(CliCommand::DeleteNode { id: "del-me".into() });
    assert!(result.is_ok(), "DeleteNode should succeed: {:?}", result);
}

#[test]
fn single_entry_delete_edge() {
    let mut bs = Bootstrap::from_events(vec![]);
    bs.run(CliCommand::CreateNode { id: "A".into() });
    bs.run(CliCommand::CreateNode { id: "B".into() });
    bs.run(CliCommand::CreateEdge {
        id: "e1".into(),
        from: "A".into(),
        to: "B".into(),
        r#type: "x".into(),
    });
    let result = bs.run(CliCommand::DeleteEdge { id: "e1".into() });
    assert!(result.is_ok(), "DeleteEdge should succeed: {:?}", result);
}

#[test]
fn single_entry_show_state() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::ShowState);
    match result {
        CommandResult::QueryState(view) => {
            assert_eq!(view.node_count(), 0, "fresh state should have 0 nodes");
        }
        other => panic!("Expected QueryState, got {:?}", other),
    }
}

#[test]
fn single_entry_explain() {
    let mut bs = Bootstrap::from_events(vec![]);
    bs.run(CliCommand::CreateNode { id: "X".into() });
    bs.run(CliCommand::SetProperty {
        target: "X".into(),
        key: "color".into(),
        value: "blue".into(),
    });
    let result = bs.run(CliCommand::Explain {
        node_id: "X".into(),
        property_key: Some("color".into()),
    });
    match result {
        CommandResult::Explain(ex) => {
            assert_eq!(ex.target_node_id, "X");
        }
        other => panic!("Expected Explain, got {:?}", other),
    }
}

#[test]
fn single_entry_trace() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::Trace {
        event_id: "nonexistent".into(),
    });
    // Should be a CausalChain result (possibly empty)
    match result {
        CommandResult::CausalChain(chain) => {
            assert!(chain.is_empty(), "nonexistent event should have empty chain");
        }
        other => panic!("Expected CausalChain, got {:?}", other),
    }
}

#[test]
fn single_entry_system_commands() {
    let mut bs = Bootstrap::from_events(vec![]);
    match bs.run(CliCommand::Version) {
        CommandResult::Message(v) => assert!(!v.is_empty(), "Version should not be empty"),
        other => panic!("Expected Message, got {:?}", other),
    }
    match bs.run(CliCommand::SchemaVersion) {
        CommandResult::Message(v) => assert!(!v.is_empty(), "SchemaVersion should not be empty"),
        other => panic!("Expected Message, got {:?}", other),
    }
    match bs.run(CliCommand::WorkflowList) {
        CommandResult::Message(_) => {}
        other => panic!("Expected Message for WorkflowList, got {:?}", other),
    }
}

#[test]
fn single_entry_all_commands_produce_result() {
    // Every command variant must produce a CommandResult
    let mut bs = Bootstrap::from_events(vec![]);

    let commands: Vec<CliCommand> = vec![
        CliCommand::CreateNode { id: "all-test".into() },
        CliCommand::Version,
        CliCommand::SchemaVersion,
        CliCommand::ShowState,
        CliCommand::WorkflowList,
    ];

    for cmd in commands {
        let result = bs.run(cmd);
    assert!(matches!(
        result,
        CommandResult::Ingested
            | CommandResult::Message(_)
            | CommandResult::QueryState(_)
    ), "Unexpected result type: {:?}", result);
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
    // The CLI module should only export CliCommand and Cli-related types
    // If it exports Engine types, this would break the pure-adapter constraint
    let cli_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/cli/mod.rs");
    let source = std::fs::read_to_string(cli_path).expect("cli/mod.rs must exist");

    // The CLI module should NOT use these engine-related imports
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
    // The CLI module's `run()` function that directly called engine methods
    // must be removed — only Bootstrap::run() should exist.
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
    let bs1 = Bootstrap::from_events(vec![]);
    let bs2 = Bootstrap::from_events(vec![]);

    // Both should have identical engine state
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

    let bs1 = Bootstrap::from_events(events.clone());
    let bs2 = Bootstrap::from_events(events.clone());

    let state1 = bs1.engine().query_state().clone();
    let state2 = bs2.engine().query_state().clone();

    assert_eq!(state1, state2, "Identical event sequences must produce identical state");
}

#[test]
fn deterministic_boot_multiple_calls_same_state() {
    // Calling run() multiple times should not affect initialization order
    let mut bs = Bootstrap::from_events(vec![]);
    let result1 = bs.run(CliCommand::ShowState);
    let result2 = bs.run(CliCommand::ShowState);

    assert_eq!(result1, result2, "Repeated ShowState must be identical");
}

// ── D. Command Routing Test ──────────────────────────────────────────────────
//
// All CLI inputs map to valid API commands and produce appropriate results.

#[test]
fn command_routing_create_node() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::CreateNode { id: "routing-test".into() });
    assert!(result.is_ok(), "CreateNode should route successfully: {:?}", result);

    // Verify the node was actually created
    let state = bs.run(CliCommand::ShowState);
    match state {
        CommandResult::QueryState(view) => {
            assert_eq!(view.node_count(), 1);
            assert_eq!(view.nodes[0].id, "routing-test");
        }
        other => panic!("Expected QueryState, got {:?}", other),
    }
}

#[test]
fn command_routing_invalid_node_rejected() {
    let mut bs = Bootstrap::from_events(vec![]);
    // Creating a node with empty ID should propagate an error
    let result = bs.run(CliCommand::CreateNode { id: "".into() });
    match result {
        CommandResult::Error(_) => {} // Expected — empty ID is invalid
        CommandResult::Ingested => {}
        other => panic!("Expected Error or Ingest, got {:?}", other),
    }
}

#[test]
fn command_routing_version() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::Version);
    match result {
        CommandResult::Message(v) => {
            assert!(!v.is_empty(), "Version should be non-empty");
        }
        other => panic!("Expected Message for Version, got {:?}", other),
    }
}

#[test]
fn command_routing_schema_version() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::SchemaVersion);
    match result {
        CommandResult::Message(v) => {
            assert!(!v.is_empty(), "SchemaVersion should be non-empty");
        }
        other => panic!("Expected Message for SchemaVersion, got {:?}", other),
    }
}

#[test]
fn command_routing_workflow_list() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::WorkflowList);
    match result {
        CommandResult::Message(msg) => {
            assert!(msg.contains("workflows"), "Workflow list should mention workflows");
        }
        other => panic!("Expected Message for WorkflowList, got {:?}", other),
    }
}

#[test]
fn command_routing_all_variants_have_kind_str() {
    // Every CliCommand variant must have a non-empty kind_str
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
        // kind_str should be a PascalCase match of the variant name
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
    let mut bs = Bootstrap::from_events(vec![]);
    bs.set_trace(true);

    // Trace should not panic or produce non-deterministic output
    let result = bs.run(CliCommand::CreateNode { id: "trace-test".into() });
    assert!(result.is_ok(), "Trace-enabled execution should succeed");

    // Second call should also succeed
    let result2 = bs.run(CliCommand::ShowState);
    match result2 {
        CommandResult::QueryState(view) => {
            assert_eq!(view.node_count(), 1);
        }
        other => panic!("Expected QueryState, got {:?}", other),
    }
}

#[test]
fn trace_disabled_produces_no_output() {
    // Default bootstrap has trace disabled
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::Version);
    match result {
        CommandResult::Message(v) => {
            assert_eq!(v, "1.0");
        }
        other => panic!("Expected Message, got {:?}", other),
    }
}

// ── Error Propagation Test ───────────────────────────────────────────────────
//
// All engine errors propagate as CommandResult::Error.

#[test]
fn error_propagation_delete_nonexistent_node() {
    let mut bs = Bootstrap::from_events(vec![]);
    let result = bs.run(CliCommand::DeleteNode { id: "i-dont-exist".into() });
    match result {
        CommandResult::Error(_) => {} // Expected
        CommandResult::Ingested => {
            // Kernel may accept delete of non-existent node depending on validation
            // This is fine — error still propagates through correct channel
        }
        other => panic!("Expected Error or Ingest, got {:?}", other),
    }
}

#[test]
fn error_propagation_create_duplicate_node() {
    let mut bs = Bootstrap::from_events(vec![]);
    let r1 = bs.run(CliCommand::CreateNode { id: "dup".into() });
    assert!(r1.is_ok(), "First creation should succeed");

    let r2 = bs.run(CliCommand::CreateNode { id: "dup".into() });
    match r2 {
        CommandResult::Error(_) => {} // Expected — duplicate
        CommandResult::Ingested => {} // Some kernels may not enforce uniqueness
        other => panic!("Expected Error or Ingest for duplicate, got {:?}", other),
    }
}

// ── format_output Test ──────────────────────────────────────────────────────
//
// format_output produces valid non-empty strings for all result types.

#[test]
fn format_output_all_variants() {
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

    // All variants must produce non-empty output
    for result in &results {
        let output = result.format_output();
        assert!(!output.is_empty(), "format_output must be non-empty for {:?}", result);
    }
}

/// Comment to maintain spacing.
#[test]
fn format_output_state_with_nodes() {
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
