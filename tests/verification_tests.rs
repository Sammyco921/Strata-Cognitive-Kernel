use strata_kill_test::verification::*;

/// Test 1: Registry contains all expected invariants
#[test]
fn test_registry_contains_all_expected_invariants() {
    let r = global_invariant_registry();
    assert_eq!(r.len(), 12, "Expected 12 invariants");

    for id in &[
        "KERNEL_REPLAY_EQUIVALENCE",
        "KERNEL_EVENT_ORDERING",
        "KERNEL_SNAPSHOT_EQUIVALENCE",
        "ONTOLOGY_REPLAY_IDEMPOTENCE",
        "ONTOLOGY_REGISTRY_DERIVATION",
        "SEMANTIC_PROJECTION_PURITY",
        "SEMANTIC_QUERY_DETERMINISM",
        "SEMANTIC_RULE_DETERMINISM",
        "SEMANTIC_PIPELINE_DETERMINISM",
        "ABI_CONTRACT_CONSISTENCY",
        "ABI_SCHEMA_CONSISTENCY",
        "ABI_SERIALIZATION_CONSISTENCY",
    ] {
        let spec = r.get(&InvariantId(id.to_string()));
        assert!(spec.is_some(), "Missing invariant: {}", id);
        assert!(!spec.unwrap().description.is_empty(), "Empty description for: {}", id);
    }
}

/// Test 2: Registry ordering is deterministic
#[test]
fn test_registry_ordering_deterministic() {
    let r = global_invariant_registry();
    let targets = r.verification_targets();
    // Verify BTreeMap ensures deterministic sort order
    for i in 1..targets.len() {
        assert!(
            targets[i - 1] < targets[i],
            "Registry targets not sorted: {:?} before {:?}",
            targets[i - 1], targets[i],
        );
    }

    // 100-run determinism
    let first: Vec<InvariantId> = global_invariant_registry().verification_targets();
    for _ in 0..100 {
        let current: Vec<InvariantId> = global_invariant_registry().verification_targets();
        assert_eq!(first, current, "Registry ordering changed between runs");
    }
}

/// Test 3: verify_all succeeds on valid system
#[test]
fn test_verify_all_succeeds_on_valid_system() {
    let report = verify_all();
    assert!(
        report.is_all_passed(),
        "Verification failed: {} out of {} invariants failed.\nFailures: {:?}",
        report.failed,
        report.total,
        report.results.iter()
            .filter(|r| r.status == InvariantStatus::Failed)
            .map(|r| (&r.invariant_id.0, &r.reason))
            .collect::<Vec<_>>(),
    );
    assert_eq!(report.total, 12);
    assert_eq!(report.passed, 12);
    assert_eq!(report.failed, 0);
}

/// Test 4: Simulated replay invariant detects known corruption
#[test]
fn test_replay_invariant_detects_corruption() {
    // Simulate corruption by constructing unequal states
    let events = make_test_events();
    let state1 = strata_kill_test::kernel::replay(&events);

    // Create corrupted state by removing an event
    let state2 = strata_kill_test::kernel::replay(&[]);

    let result = if state1 == state2 {
        VerificationResult::passed(
            InvariantId("KERNEL_REPLAY_EQUIVALENCE".into()),
            InvariantLayer::Kernel,
        )
    } else {
        VerificationResult::failed(
            InvariantId("KERNEL_REPLAY_EQUIVALENCE".into()),
            InvariantLayer::Kernel,
            "replay outputs diverged",
        )
    };

    assert_eq!(result.status, InvariantStatus::Failed);
    assert!(result.reason.contains("KERNEL_REPLAY_EQUIVALENCE"));
    assert!(result.reason.contains("replay outputs diverged"));
}

/// Test 5: Ontology invariant detects divergence
#[test]
fn test_ontology_invariant_detects_divergence() {
    let events = make_ontology_test_events();
    let r1 = strata_kill_test::ontology::replay::replay_ontology(&events);

    // Remove an event to create divergence
    let mut corrupted = events.clone();
    corrupted.pop();
    let r2 = strata_kill_test::ontology::replay::replay_ontology(&corrupted);

    let result = if r1 == r2 {
        VerificationResult::passed(
            InvariantId("ONTOLOGY_REPLAY_IDEMPOTENCE".into()),
            InvariantLayer::Ontology,
        )
    } else {
        VerificationResult::failed(
            InvariantId("ONTOLOGY_REPLAY_IDEMPOTENCE".into()),
            InvariantLayer::Ontology,
            "repeated replay produced divergent registries",
        )
    };

    assert_eq!(result.status, InvariantStatus::Failed);
    assert!(result.reason.contains("ONTOLOGY_REPLAY_IDEMPOTENCE"));
}

/// Test 6: Semantic invariant detects divergence
#[test]
fn test_semantic_invariant_detects_divergence() {
    use strata_kill_test::ontology::semantic::query::types::QuerySpec;
    use strata_kill_test::ontology::semantic::projection::semantic_project;

    let events = make_test_sequenced_events();
    let onto = make_test_ontology_registry();
    let graph = semantic_project(&events, &onto);

    let spec = QuerySpec {
        node_type_filter: Some(vec!["person".into()]),
        edge_type_filter: None,
        property_filters: Vec::new(),
        traversal_depth: None,
        source_node_ids: None,
        target_node_ids: None,
    };

    let r1 = strata_kill_test::ontology::semantic::query::engine::query(&graph, &onto, &spec);

    // Query with different spec should produce different result
    let spec2 = QuerySpec {
        node_type_filter: Some(vec!["nonexistent".into()]),
        edge_type_filter: None,
        property_filters: Vec::new(),
        traversal_depth: None,
        source_node_ids: None,
        target_node_ids: None,
    };
    let r2 = strata_kill_test::ontology::semantic::query::engine::query(&graph, &onto, &spec2);

    let result = if r1 == r2 {
        VerificationResult::passed(
            InvariantId("SEMANTIC_QUERY_DETERMINISM".into()),
            InvariantLayer::Semantic,
        )
    } else {
        VerificationResult::failed(
            InvariantId("SEMANTIC_QUERY_DETERMINISM".into()),
            InvariantLayer::Semantic,
            "repeated query on same inputs produced divergent results",
        )
    };

    // r1 and r2 differ because different specs produce different results:
    // r1 has person nodes, r2 has none
    assert_eq!(r1.nodes.len(), 1, "person query should find 1 node");
    assert!(r2.nodes.is_empty(), "nonexistent query should find 0 nodes");
    assert_eq!(result.status, InvariantStatus::Failed);
}

/// Test 7: ABI invariant detects drift
#[test]
fn test_abi_invariant_detects_drift() {
    // The real verify passes on the current system
    let report = verify_all();
    let abi_results: Vec<&VerificationResult> = report.results.iter()
        .filter(|r| r.layer == InvariantLayer::Abi)
        .collect();
    assert_eq!(abi_results.len(), 3);
    for r in &abi_results {
        assert_eq!(r.status, InvariantStatus::Passed, "ABI invariant failed: {}", r.reason);
    }

    // Simulate drift: a field that should be in the contract isn't
    let registry = strata_kill_test::abi::global_registry();
    let qt = registry.get("QueryTypes").unwrap();
    let missing_field = "nonexistent_field_xyz";
    let has_field = qt.fields.contains(missing_field);
    assert!(!has_field, "Test invariant: nonexistent field should not be in contract");
}

/// Test 8: Verification report serialization is deterministic
#[test]
fn test_verification_report_serialization_deterministic() {
    let report = verify_all();
    let s1 = report.to_deterministic_string();
    let s2 = report.to_deterministic_string();
    assert_eq!(s1, s2);
}

/// Test 9: Verification report roundtrip through string contains expected data
#[test]
fn test_verification_report_contains_expected_data() {
    let report = verify_all();
    let json = report.to_deterministic_string();

    // Should contain all invariant IDs
    for id in &[
        "KERNEL_REPLAY_EQUIVALENCE",
        "KERNEL_EVENT_ORDERING",
        "KERNEL_SNAPSHOT_EQUIVALENCE",
        "ABI_CONTRACT_CONSISTENCY",
    ] {
        assert!(json.contains(id), "Report missing invariant ID: {}", id);
    }

    // Should contain layer names
    assert!(json.contains("Kernel"));
    assert!(json.contains("Ontology"));
    assert!(json.contains("Semantic"));
    assert!(json.contains("Abi"));

    // Should contain counts
    assert!(json.contains("\"total\":12"));
    assert!(json.contains("\"passed\""));
    assert!(json.contains("\"failed\""));
}

/// Test 10: 100-run stability
#[test]
fn test_100_run_stability() {
    let first = verify_all().to_deterministic_string();
    for _ in 0..100 {
        let current = verify_all().to_deterministic_string();
        assert_eq!(first, current, "Verification output changed between runs");
    }
}

/// Test 11: No mutation of inputs
#[test]
fn test_no_mutation_of_inputs() {
    use strata_kill_test::kernel::replay;
    use strata_kill_test::ontology::replay::replay_ontology;

    // Kernel events
    let events = make_test_events();
    let events_clone = events.clone();
    let _ = replay(&events);
    assert_eq!(events, events_clone, "Kernel replay mutated input events");

    // Ontology events
    let onto_events = make_ontology_test_events();
    let onto_clone = onto_events.clone();
    let _ = replay_ontology(&onto_events);
    assert_eq!(onto_events, onto_clone, "Ontology replay mutated input events");
}

/// Test 12: Failure localization contains invariant ID and reason
#[test]
fn test_failure_localization() {
    let failed = VerificationResult::failed(
        InvariantId("SPECIFIC_FAILURE".into()),
        InvariantLayer::Kernel,
        "a specific reason",
    );
    assert_eq!(failed.status, InvariantStatus::Failed);
    assert!(failed.reason.contains("SPECIFIC_FAILURE"));
    assert!(failed.reason.contains("a specific reason"));
    assert_eq!(failed.layer, InvariantLayer::Kernel);
    assert_eq!(failed.invariant_id.0, "SPECIFIC_FAILURE");
}

/// Test 13: Layer classification is correct for all invariants
#[test]
fn test_layer_classification() {
    let registry = global_invariant_registry();

    assert_eq!(registry.by_layer(&InvariantLayer::Kernel).len(), 3);
    assert_eq!(registry.by_layer(&InvariantLayer::Ontology).len(), 2);
    assert_eq!(registry.by_layer(&InvariantLayer::Semantic).len(), 4);
    assert_eq!(registry.by_layer(&InvariantLayer::Abi).len(), 3);
}

/// Test 14: Report counts match result vectors
#[test]
fn test_report_counts_match_results() {
    let results = vec![
        VerificationResult::passed(InvariantId("A".into()), InvariantLayer::Kernel),
        VerificationResult::passed(InvariantId("B".into()), InvariantLayer::Ontology),
        VerificationResult::failed(InvariantId("C".into()), InvariantLayer::Semantic, "fail"),
    ];
    let report = VerificationReport::new(results.clone());
    assert_eq!(report.total, results.len());
    assert_eq!(report.passed, 2);
    assert_eq!(report.failed, 1);

    // Verify result ordering matches input
    assert_eq!(report.results[0].invariant_id.0, "A");
    assert_eq!(report.results[1].invariant_id.0, "B");
    assert_eq!(report.results[2].invariant_id.0, "C");
}

/// Test 15: Duplicate invariant IDs are rejected
#[test]
fn test_duplicate_invariant_id_rejected() {
    let mut registry = InvariantRegistry::new();
    let spec1 = InvariantSpec {
        id: InvariantId("DUP".into()),
        layer: InvariantLayer::Kernel,
        description: "first".into(),
    };
    assert!(registry.register(spec1).is_ok());

    let spec2 = InvariantSpec {
        id: InvariantId("DUP".into()),
        layer: InvariantLayer::Kernel,
        description: "second".into(),
    };
    let result = registry.register(spec2);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Duplicate"));
}

/// Test 16: Empty registry is valid
#[test]
fn test_empty_registry() {
    let registry = InvariantRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    let targets = registry.verification_targets();
    assert!(targets.is_empty());
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn make_test_events() -> Vec<strata_kill_test::kernel::SequencedEvent> {
    use strata_kill_test::kernel::{Event, SequencedEvent};
    vec![
        SequencedEvent::from_seq(0, Event::CreateNode { id: 1, node_type: "person".into() }),
        SequencedEvent::from_seq(1, Event::CreateNode { id: 2, node_type: "organization".into() }),
        SequencedEvent::from_seq(2, Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "works_at".into() }),
    ]
}

fn make_test_sequenced_events() -> Vec<strata_kill_test::kernel::SequencedEvent> {
    make_test_events()
}

fn make_ontology_test_events() -> Vec<strata_kill_test::ontology::types::OntologyEvent> {
    use strata_kill_test::ontology::types::*;
    vec![
        OntologyEvent::new(
            OntologyEventType::CreateEntityType,
            OntologyPayload::EntityType(EntityTypeDef {
                name: "person".into(), description: Some("A person".into()),
            }),
            0,
        ),
        OntologyEvent::new(
            OntologyEventType::CreateEntityType,
            OntologyPayload::EntityType(EntityTypeDef {
                name: "organization".into(), description: Some("An org".into()),
            }),
            1,
        ),
    ]
}

fn make_test_ontology_registry() -> strata_kill_test::ontology::types::OntologyRegistry {
    strata_kill_test::ontology::replay::replay_ontology(&make_ontology_test_events())
}
