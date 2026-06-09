use crate::verification::types::*;
use crate::verification::registry::global_invariant_registry;
use std::collections::BTreeSet;

pub fn verify_all() -> VerificationReport {
    let registry = global_invariant_registry();
    let targets = registry.verification_targets();
    let mut results: Vec<VerificationResult> = Vec::new();

    for id in &targets {
        let spec = registry.get(id).unwrap();
        let result = match id.0.as_str() {
            "KERNEL_REPLAY_EQUIVALENCE" => verify_kernel_replay_equivalence(),
            "KERNEL_EVENT_ORDERING" => verify_kernel_event_ordering(),
            "KERNEL_SNAPSHOT_EQUIVALENCE" => verify_kernel_snapshot_equivalence(),
            "ONTOLOGY_REPLAY_IDEMPOTENCE" => verify_ontology_replay_idempotence(),
            "ONTOLOGY_REGISTRY_DERIVATION" => verify_ontology_registry_derivation(),
            "SEMANTIC_PROJECTION_PURITY" => verify_semantic_projection_purity(),
            "SEMANTIC_QUERY_DETERMINISM" => verify_semantic_query_determinism(),
            "SEMANTIC_RULE_DETERMINISM" => verify_semantic_rule_determinism(),
            "SEMANTIC_PIPELINE_DETERMINISM" => verify_semantic_pipeline_determinism(),
            "ABI_CONTRACT_CONSISTENCY" => verify_abi_contract_consistency(),
            "ABI_SCHEMA_CONSISTENCY" => verify_abi_schema_consistency(),
            "ABI_SERIALIZATION_CONSISTENCY" => verify_abi_serialization_consistency(),
            _ => VerificationResult::failed(
                id.clone(),
                spec.layer.clone(),
                &format!("Unknown invariant: {}", id.0),
            ),
        };
        results.push(result);
    }

    VerificationReport::new(results)
}

// ── Kernel invariants ────────────────────────────────────────────────────

fn verify_kernel_replay_equivalence() -> VerificationResult {
    let id = InvariantId("KERNEL_REPLAY_EQUIVALENCE".into());
    let layer = InvariantLayer::Kernel;

    let events = make_sequenced_events();
    let state1 = crate::kernel::replay(&events);
    let state2 = crate::kernel::replay(&events);

    if state1 == state2 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "replay outputs diverged")
    }
}

fn verify_kernel_event_ordering() -> VerificationResult {
    let id = InvariantId("KERNEL_EVENT_ORDERING".into());
    let layer = InvariantLayer::Kernel;

    let events = make_sequenced_events();
    let state = crate::kernel::replay(&events);

    // Verify events applied in order produce deterministic state
    // by replaying twice and checking
    let state2 = crate::kernel::replay(&events);
    if state == state2 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "ordered replay produced divergent states")
    }
}

fn verify_kernel_snapshot_equivalence() -> VerificationResult {
    let id = InvariantId("KERNEL_SNAPSHOT_EQUIVALENCE".into());
    let layer = InvariantLayer::Kernel;

    use crate::kernel::{Kernel, NoDuplicateNodeValidator, NoDuplicateEdgeValidator, NodeExistenceValidator};

    let mut kernel = Kernel::with_validators(vec![
        Box::new(NoDuplicateNodeValidator),
        Box::new(NoDuplicateEdgeValidator),
        Box::new(NodeExistenceValidator),
    ]);

    // Commit events one by one (this builds the direct state snapshot)
    for event in make_events() {
        let _ = kernel.propose_and_commit(event);
    }

    let direct_state = kernel.state().clone();
    let replayed_state = kernel.replay();

    if direct_state == replayed_state {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "snapshot state does not equal replay state")
    }
}

// ── Ontology invariants ──────────────────────────────────────────────────

fn verify_ontology_replay_idempotence() -> VerificationResult {
    let id = InvariantId("ONTOLOGY_REPLAY_IDEMPOTENCE".into());
    let layer = InvariantLayer::Ontology;

    let events = make_ontology_events();

    let r1 = crate::ontology::replay::replay_ontology(&events);
    let r2 = crate::ontology::replay::replay_ontology(&events);
    let r3 = crate::ontology::replay::replay_ontology(&events);

    if r1 == r2 && r2 == r3 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "repeated replay produced divergent registries")
    }
}

fn verify_ontology_registry_derivation() -> VerificationResult {
    let id = InvariantId("ONTOLOGY_REGISTRY_DERIVATION".into());
    let layer = InvariantLayer::Ontology;

    use crate::ontology::types::*;

    let events = make_ontology_events();

    // Via replay
    let from_replay = crate::ontology::replay::replay_ontology(&events);

    // Via iterative apply_event
    let mut from_apply = OntologyRegistry::empty();
    for event in &events {
        from_apply.apply_event(event);
    }

    if from_replay == from_apply {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "replay derivation differs from iterative apply_event")
    }
}

// ── Semantic invariants ──────────────────────────────────────────────────

fn verify_semantic_projection_purity() -> VerificationResult {
    let id = InvariantId("SEMANTIC_PROJECTION_PURITY".into());
    let layer = InvariantLayer::Semantic;

    let events = make_sequenced_events();
    let onto = make_ontology_registry();

    let g1 = crate::ontology::semantic::projection::semantic_project(&events, &onto);
    let g2 = crate::ontology::semantic::projection::semantic_project(&events, &onto);
    let g3 = crate::ontology::semantic::projection::semantic_project(&events, &onto);

    if g1 == g2 && g2 == g3 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "repeated projection produced divergent graphs")
    }
}

fn verify_semantic_query_determinism() -> VerificationResult {
    let id = InvariantId("SEMANTIC_QUERY_DETERMINISM".into());
    let layer = InvariantLayer::Semantic;

    use crate::ontology::semantic::query::types::QuerySpec;

    let events = make_sequenced_events();
    let onto = make_ontology_registry();
    let graph = crate::ontology::semantic::projection::semantic_project(&events, &onto);

    let spec = QuerySpec {
        node_type_filter: Some(vec!["person".into()]),
        edge_type_filter: None,
        property_filters: Vec::new(),
        traversal_depth: None,
        source_node_ids: None,
        target_node_ids: None,
    };

    let r1 = crate::ontology::semantic::query::engine::query(&graph, &onto, &spec);
    let r2 = crate::ontology::semantic::query::engine::query(&graph, &onto, &spec);

    if r1 == r2 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "repeated query on same inputs produced divergent results")
    }
}

fn verify_semantic_rule_determinism() -> VerificationResult {
    let id = InvariantId("SEMANTIC_RULE_DETERMINISM".into());
    let layer = InvariantLayer::Semantic;

    use crate::ontology::semantic::query::types::QuerySpec;
    use crate::ontology::semantic::rules::types::RuleSpec;

    let events = make_sequenced_events();
    let onto = make_ontology_registry();
    let graph = crate::ontology::semantic::projection::semantic_project(&events, &onto);

    let spec = QuerySpec {
        node_type_filter: None,
        edge_type_filter: None,
        property_filters: Vec::new(),
        traversal_depth: None,
        source_node_ids: None,
        target_node_ids: None,
    };
    let result = crate::ontology::semantic::query::engine::query(&graph, &onto, &spec);

    let mut rules = BTreeSet::new();
    rules.insert(RuleSpec {
        id: "r1".into(),
        tag: "tag1".into(),
        node_type_match: Some("person".into()),
        node_property_matches: Vec::new(),
        specific_node_id: None,
        edge_type_match: None,
        edge_property_matches: Vec::new(),
        specific_edge_id: None,
    });

    let a1 = crate::ontology::semantic::rules::engine::apply_rules(&result, &graph, &rules);
    let a2 = crate::ontology::semantic::rules::engine::apply_rules(&result, &graph, &rules);

    if a1 == a2 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "repeated rule application produced divergent annotations")
    }
}

fn verify_semantic_pipeline_determinism() -> VerificationResult {
    let id = InvariantId("SEMANTIC_PIPELINE_DETERMINISM".into());
    let layer = InvariantLayer::Semantic;

    use crate::ontology::semantic::composition::types::PipelineSpec;

    let events = make_sequenced_events();
    let onto = make_ontology_registry();
    let graph = crate::ontology::semantic::projection::semantic_project(&events, &onto);

    let pipeline = PipelineSpec { steps: Vec::new() };

    let p1 = crate::ontology::semantic::composition::engine::execute_pipeline(&graph, &onto, &pipeline);
    let p2 = crate::ontology::semantic::composition::engine::execute_pipeline(&graph, &onto, &pipeline);

    if p1 == p2 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "repeated pipeline execution produced divergent results")
    }
}

// ── ABI invariants ───────────────────────────────────────────────────────

fn verify_abi_contract_consistency() -> VerificationResult {
    let id = InvariantId("ABI_CONTRACT_CONSISTENCY".into());
    let layer = InvariantLayer::Abi;

    let report = crate::abi::validate_global_registry();
    if report.is_valid {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "ABI contract registry validation failed")
    }
}

fn verify_abi_schema_consistency() -> VerificationResult {
    let id = InvariantId("ABI_SCHEMA_CONSISTENCY".into());
    let layer = InvariantLayer::Abi;

    let registry = crate::abi::global_registry();

    // Check QueryTypes contract contains QuerySpec and ResultSet schema fields
    let qt = registry.get("QueryTypes").unwrap();
    let query_spec_fields = crate::ontology::abi::schema::QUERY_SPEC_SCHEMA.field_names;
    let result_set_fields = crate::ontology::abi::schema::RESULT_SET_SCHEMA.field_names;

    for f in query_spec_fields {
        if !qt.fields.contains(*f) {
            return VerificationResult::failed(
                id, layer,
                &format!("QueryTypes contract missing field '{}' from QUERY_SPEC_SCHEMA", f),
            );
        }
    }
    for f in result_set_fields {
        if !qt.fields.contains(*f) {
            return VerificationResult::failed(
                id, layer,
                &format!("QueryTypes contract missing field '{}' from RESULT_SET_SCHEMA", f),
            );
        }
    }

    // Check RuleTypes contract
    let rt = registry.get("RuleTypes").unwrap();
    let rule_spec_fields = crate::ontology::abi::schema::RULE_SPEC_SCHEMA.field_names;
    let annot_fields = crate::ontology::abi::schema::ANNOTATED_RESULT_SET_SCHEMA.field_names;
    for f in rule_spec_fields {
        if !rt.fields.contains(*f) {
            return VerificationResult::failed(
                id, layer,
                &format!("RuleTypes contract missing field '{}' from RULE_SPEC_SCHEMA", f),
            );
        }
    }
    for f in annot_fields {
        if !rt.fields.contains(*f) {
            return VerificationResult::failed(
                id, layer,
                &format!("RuleTypes contract missing field '{}' from ANNOTATED_RESULT_SET_SCHEMA", f),
            );
        }
    }

    // Check CompositionTypes contract
    let ct = registry.get("CompositionTypes").unwrap();
    let pipe_spec_fields = crate::ontology::abi::schema::PIPELINE_SPEC_SCHEMA.field_names;
    let pipe_result_fields = crate::ontology::abi::schema::PIPELINE_RESULT_SCHEMA.field_names;
    for f in pipe_spec_fields {
        if !ct.fields.contains(*f) {
            return VerificationResult::failed(
                id, layer,
                &format!("CompositionTypes contract missing field '{}' from PIPELINE_SPEC_SCHEMA", f),
            );
        }
    }
    for f in pipe_result_fields {
        if !ct.fields.contains(*f) {
            return VerificationResult::failed(
                id, layer,
                &format!("CompositionTypes contract missing field '{}' from PIPELINE_RESULT_SCHEMA", f),
            );
        }
    }

    VerificationResult::passed(id, layer)
}

fn verify_abi_serialization_consistency() -> VerificationResult {
    let id = InvariantId("ABI_SERIALIZATION_CONSISTENCY".into());
    let layer = InvariantLayer::Abi;

    use crate::ontology::semantic::query::types::QuerySpec;

    let spec = QuerySpec {
        node_type_filter: Some(vec!["person".into()]),
        edge_type_filter: None,
        property_filters: Vec::new(),
        traversal_depth: None,
        source_node_ids: None,
        target_node_ids: None,
    };

    let s1 = spec.to_deterministic_string();
    let s2 = spec.to_deterministic_string();
    let s3 = spec.to_deterministic_string();

    if s1 == s2 && s2 == s3 {
        VerificationResult::passed(id, layer)
    } else {
        VerificationResult::failed(id, layer, "deterministic serialization produced divergent output")
    }
}

// ── Test helpers ─────────────────────────────────────────────────────────

fn make_events() -> Vec<crate::kernel::Event> {
    use crate::kernel::Event;
    vec![
        Event::CreateNode { id: 1, node_type: "person".into() },
        Event::CreateNode { id: 2, node_type: "organization".into() },
        Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "works_at".into() },
        Event::SetProperty { node_id: 1, key: "name".into(), value: "Alice".into() },
    ]
}

fn make_sequenced_events() -> Vec<crate::kernel::SequencedEvent> {
    use crate::kernel::{Event, SequencedEvent};
    vec![
        SequencedEvent::from_seq(0, Event::CreateNode { id: 1, node_type: "person".into() }),
        SequencedEvent::from_seq(1, Event::CreateNode { id: 2, node_type: "organization".into() }),
        SequencedEvent::from_seq(2, Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "works_at".into() }),
        SequencedEvent::from_seq(3, Event::SetProperty { node_id: 1, key: "name".into(), value: "Alice".into() }),
    ]
}

fn make_ontology_events() -> Vec<crate::ontology::types::OntologyEvent> {
    use crate::ontology::types::*;
    vec![
        OntologyEvent::new(
            OntologyEventType::CreateEntityType,
            OntologyPayload::EntityType(EntityTypeDef {
                name: "person".into(),
                description: Some("A human person".into()),
            }),
            0,
        ),
        OntologyEvent::new(
            OntologyEventType::CreateEntityType,
            OntologyPayload::EntityType(EntityTypeDef {
                name: "organization".into(),
                description: Some("A company or group".into()),
            }),
            1,
        ),
        OntologyEvent::new(
            OntologyEventType::CreateRelationshipType,
            OntologyPayload::RelationshipType(RelationshipTypeDef {
                name: "works_at".into(),
                from_entity: "person".into(),
                to_entity: "organization".into(),
                description: Some("Employment relationship".into()),
            }),
            2,
        ),
    ]
}

fn make_ontology_registry() -> crate::ontology::types::OntologyRegistry {
    crate::ontology::replay::replay_ontology(&make_ontology_events())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_all_passes_on_valid_system() {
        let report = verify_all();
        assert!(
            report.is_all_passed(),
            "Expected all invariants to pass, but {} failed:\n{:?}",
            report.failed,
            report.results.iter().filter(|r| r.status == InvariantStatus::Failed).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn test_verify_all_returns_all_results() {
        let report = verify_all();
        assert_eq!(report.total, 12);
        assert_eq!(report.passed + report.failed, report.total);
    }

    #[test]
    fn test_each_invariant_id_in_report() {
        let report = verify_all();
        let expected_ids: BTreeSet<&str> = [
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
        ].iter().cloned().collect();

        let actual_ids: BTreeSet<&str> = report.results.iter().map(|r| r.invariant_id.0.as_str()).collect();
        assert_eq!(expected_ids, actual_ids);
    }

    #[test]
    fn test_failure_localization_contains_id_and_reason() {
        // Simulate a failure manually
        let result = VerificationResult::failed(
            InvariantId("TEST_INVARIANT".into()),
            InvariantLayer::Kernel,
            "something specific broke",
        );
        assert!(result.reason.contains("TEST_INVARIANT"));
        assert!(result.reason.contains("something specific broke"));
        assert_eq!(result.status, InvariantStatus::Failed);
    }

    #[test]
    fn test_no_panic_on_verification() {
        let report = std::panic::catch_unwind(|| {
            verify_all()
        });
        assert!(report.is_ok(), "verify_all panicked");
    }

    #[test]
    fn test_verification_serialization_determinism() {
        let report = verify_all();
        let s1 = report.to_deterministic_string();
        let s2 = report.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_stability_100_runs() {
        let first = verify_all().to_deterministic_string();
        for _ in 0..100 {
            let s = verify_all().to_deterministic_string();
            assert_eq!(first, s);
        }
    }
}
