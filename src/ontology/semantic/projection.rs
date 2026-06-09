use crate::kernel::{replay, SequencedEvent};
use crate::ontology::types::OntologyRegistry;
use crate::ontology::semantic::types::*;

pub fn semantic_project(
    kernel_events: &[SequencedEvent],
    ontology: &OntologyRegistry,
) -> SemanticGraph {
    let state = replay(kernel_events);

    let mut nodes = std::collections::BTreeMap::new();
    for (&id, node) in &state.nodes {
        let semantic = ontology.entity_types.get(&node.node_type);
        nodes.insert(id, TypedNode {
            id,
            node_type: node.node_type.clone(),
            properties: node.properties.clone(),
            semantic_type_name: semantic.map(|t| t.name.clone()),
            semantic_type_description: semantic.and_then(|t| t.description.clone()),
        });
    }

    let mut edges = std::collections::BTreeMap::new();
    for (&id, edge) in &state.edges {
        let semantic = ontology.relationship_types.get(&edge.edge_type);
        let from = state.nodes.get(&edge.from_node);
        let to = state.nodes.get(&edge.to_node);
        edges.insert(id, TypedEdge {
            id,
            from_node: edge.from_node,
            to_node: edge.to_node,
            edge_type: edge.edge_type.clone(),
            properties: edge.properties.clone(),
            semantic_type_name: semantic.map(|t| t.name.clone()),
            semantic_type_description: semantic.and_then(|t| t.description.clone()),
            from_node_type: from.map(|n| n.node_type.clone()),
            to_node_type: to.map(|n| n.node_type.clone()),
        });
    }

    SemanticGraph { nodes, edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::*;
    use crate::ontology::*;
    use std::collections::BTreeMap;

    fn make_kernel_events() -> Vec<SequencedEvent> {
        vec![
            SequencedEvent::from_seq( 0, Event::CreateNode { id: 1, node_type: "person".into()}),
            SequencedEvent::from_seq( 1, Event::CreateNode { id: 2, node_type: "organization".into()}),
            SequencedEvent::from_seq( 2, Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "works_at".into()}),
            SequencedEvent::from_seq( 3, Event::SetProperty { node_id: 1, key: "name".into(), value: "Alice".into()}),
            SequencedEvent::from_seq( 4, Event::SetProperty { node_id: 2, key: "name".into(), value: "Acme".into()}),
        ]
    }

    fn make_ontology() -> OntologyRegistry {
        let events = vec![
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
        ];
        replay_ontology(&events)
    }

    fn make_empty_ontology() -> OntologyRegistry {
        OntologyRegistry::empty()
    }

    // ── Test 1: Deterministic projection ────────────────────────────────

    #[test]
    fn test_deterministic_projection() {
        let events = make_kernel_events();
        let onto = make_ontology();

        let g1 = semantic_project(&events, &onto);
        let g2 = semantic_project(&events, &onto);

        assert_eq!(g1, g2);
        assert_eq!(g1.to_deterministic_string(), g2.to_deterministic_string());
    }

    // ── Test 2: Order invariance (BTree-based iteration) ─────────────────

    #[test]
    fn test_order_invariance_btree() {
        let events = make_kernel_events();
        let onto = make_ontology();

        let g = semantic_project(&events, &onto);

        // Verify nodes are sorted by ID (BTreeMap property)
        let node_ids: Vec<u64> = g.nodes.keys().copied().collect();
        assert_eq!(node_ids, vec![1, 2]);

        // Verify edges are sorted by ID
        let edge_ids: Vec<u64> = g.edges.keys().copied().collect();
        assert_eq!(edge_ids, vec![10]);
    }

    // ── Test 3: Ontology removal independence ────────────────────────────

    #[test]
    fn test_ontology_removal_independence() {
        let events = make_kernel_events();

        // Project with ontology
        let with_onto = make_ontology();
        let g_with = semantic_project(&events, &with_onto);

        // Project without ontology
        let without_onto = make_empty_ontology();
        let g_without = semantic_project(&events, &without_onto);

        // Both should produce the same node/edge counts
        assert_eq!(g_with.nodes.len(), g_without.nodes.len());
        assert_eq!(g_with.edges.len(), g_without.edges.len());

        // Without ontology, semantic_type fields should be None
        for node in g_without.nodes.values() {
            assert_eq!(node.semantic_type_name, None);
            assert_eq!(node.semantic_type_description, None);
        }
        for edge in g_without.edges.values() {
            assert_eq!(edge.semantic_type_name, None);
            assert_eq!(edge.semantic_type_description, None);
        }

        // With ontology, semantic_type fields should be Some
        for node in g_with.nodes.values() {
            assert!(node.semantic_type_name.is_some());
        }
    }

    #[test]
    fn test_kernel_works_without_ontology() {
        // Kernel correctness must be unaffected if ontology is removed
        let events = make_kernel_events();
        let onto = make_empty_ontology();
        let g = semantic_project(&events, &onto);

        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
    }

    // ── Test 4: Kernel independence ──────────────────────────────────────

    #[test]
    fn test_empty_events_produces_empty_graph() {
        let onto = make_ontology();
        let g = semantic_project(&[], &onto);
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
    }

    #[test]
    fn test_kernel_events_without_matching_ontology() {
        // Events use types not defined in ontology
        let events = vec![
            SequencedEvent::from_seq( 0, Event::CreateNode { id: 1, node_type: "unknown_type".into()}),
        ];
        let onto = make_ontology();
        let g = semantic_project(&events, &onto);

        assert_eq!(g.nodes.len(), 1);
        let node = g.nodes.get(&1).unwrap();
        assert_eq!(node.node_type, "unknown_type");
        assert_eq!(node.semantic_type_name, None); // no ontology entry
    }

    // ── Test 5: No mutation tests ────────────────────────────────────────

    #[test]
    fn test_projection_does_not_mutate_inputs() {
        let events = make_kernel_events();
        let onto = make_ontology();

        let events_before = events.clone();
        let onto_before = onto.clone();

        let _g = semantic_project(&events, &onto);

        assert_eq!(events, events_before);
        assert_eq!(onto, onto_before);
    }

    #[test]
    fn test_projection_is_pure_function() {
        // Multiple calls with same args must produce same result
        let events = make_kernel_events();
        let onto = make_ontology();

        let results: Vec<SemanticGraph> = (0..5).map(|_| semantic_project(&events, &onto)).collect();
        for i in 1..results.len() {
            assert_eq!(results[0], results[i]);
        }
    }

    // ── Test 6: Serialization determinism ────────────────────────────────

    #[test]
    fn test_semantic_graph_serialization_determinism() {
        let events = make_kernel_events();
        let onto = make_ontology();
        let g = semantic_project(&events, &onto);

        let s1 = g.to_deterministic_string();
        let s2 = g.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_typed_node_serialization_determinism() {
        let node = TypedNode {
            id: 1,
            node_type: "person".into(),
            properties: BTreeMap::from([("name".into(), "Alice".into())]),
            semantic_type_name: Some("person".into()),
            semantic_type_description: Some("A human".into()),
        };
        let s1 = node.to_deterministic_string();
        let s2 = node.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_typed_edge_serialization_determinism() {
        let edge = TypedEdge {
            id: 10,
            from_node: 1,
            to_node: 2,
            edge_type: "works_at".into(),
            properties: BTreeMap::new(),
            semantic_type_name: Some("works_at".into()),
            semantic_type_description: Some("Employment".into()),
            from_node_type: Some("person".into()),
            to_node_type: Some("organization".into()),
        };
        let s1 = edge.to_deterministic_string();
        let s2 = edge.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_semantic_graph_serialization_roundtrip() {
        let events = make_kernel_events();
        let onto = make_ontology();
        let g = semantic_project(&events, &onto);

        let s = g.to_deterministic_string();

        // First verify individual node/edge roundtrips
        for (_id, node) in &g.nodes {
            let ns = node.to_deterministic_string();
            let parsed = TypedNode::from_deterministic_string(&ns);
            assert!(parsed.is_some(), "Failed to parse TypedNode: {}", ns);
            assert_eq!(node, &parsed.unwrap());
        }
        for (_id, edge) in &g.edges {
            let es = edge.to_deterministic_string();
            let parsed = TypedEdge::from_deterministic_string(&es);
            assert!(parsed.is_some(), "Failed to parse TypedEdge: {}", es);
            assert_eq!(edge, &parsed.unwrap());
        }

        let back = SemanticGraph::from_deterministic_string(&s).unwrap();

        assert_eq!(g, back);
        assert_eq!(g.to_deterministic_string(), back.to_deterministic_string());
    }

    #[test]
    fn test_typed_node_serialization_roundtrip() {
        let cases = vec![
            TypedNode {
                id: 1,
                node_type: "person".into(),
                properties: BTreeMap::new(),
                semantic_type_name: None,
                semantic_type_description: None,
            },
            TypedNode {
                id: 42,
                node_type: "organization".into(),
                properties: BTreeMap::from([("name".into(), "Acme".into())]),
                semantic_type_name: Some("organization".into()),
                semantic_type_description: Some("A company".into()),
            },
        ];
        for node in &cases {
            let s = node.to_deterministic_string();
            let back = TypedNode::from_deterministic_string(&s).unwrap();
            assert_eq!(node, &back);
        }
    }

    #[test]
    fn test_typed_edge_serialization_roundtrip() {
        let cases = vec![
            TypedEdge {
                id: 10,
                from_node: 1,
                to_node: 2,
                edge_type: "connects".into(),
                properties: BTreeMap::new(),
                semantic_type_name: None,
                semantic_type_description: None,
                from_node_type: None,
                to_node_type: None,
            },
            TypedEdge {
                id: 20,
                from_node: 5,
                to_node: 6,
                edge_type: "owns".into(),
                properties: BTreeMap::from([("since".into(), "2020".into())]),
                semantic_type_name: Some("owns".into()),
                semantic_type_description: Some("Ownership".into()),
                from_node_type: Some("person".into()),
                to_node_type: Some("asset".into()),
            },
        ];
        for edge in &cases {
            let s = edge.to_deterministic_string();
            let back = TypedEdge::from_deterministic_string(&s).unwrap();
            assert_eq!(edge, &back);
        }
    }

    #[test]
    fn test_semantic_graph_deserialization_verify_fields() {
        let events = make_kernel_events();
        let onto = make_ontology();
        let g = semantic_project(&events, &onto);

        let s = g.to_deterministic_string();
        let back = SemanticGraph::from_deterministic_string(&s).unwrap();

        // Spot-check specific fields
        let node = back.nodes.get(&1).unwrap();
        assert_eq!(node.node_type, "person");
        assert_eq!(node.semantic_type_name.as_deref(), Some("person"));
        assert_eq!(node.semantic_type_description.as_deref(), Some("A human person"));
        assert_eq!(node.properties.get("name").map(|s| s.as_str()), Some("Alice"));

        let edge = back.edges.get(&10).unwrap();
        assert_eq!(edge.edge_type, "works_at");
        assert_eq!(edge.from_node_type.as_deref(), Some("person"));
        assert_eq!(edge.to_node_type.as_deref(), Some("organization"));
    }

    // ── Test 7: Empty event stream handling ──────────────────────────────

    #[test]
    fn test_empty_event_stream() {
        let onto = make_ontology();
        let g = semantic_project(&[], &onto);
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
        assert_eq!(g.to_deterministic_string(), "{\"nodes\":{},\"edges\":{}}");
    }

    // ── Test 8: Mixed kernel + ontology event streams ────────────────────

    #[test]
    fn test_projection_from_pipeline() {
        // Simulate a full pipeline: ontology events → registry, kernel events → projection
        let onto_events = vec![
            OntologyEvent::new(
                OntologyEventType::CreateEntityType,
                OntologyPayload::EntityType(EntityTypeDef {
                    name: "symptom".into(),
                    description: Some("A medical symptom".into()),
                }),
                0,
            ),
        ];
        let onto = replay_ontology(&onto_events);

        let kernel_events = vec![
            SequencedEvent::from_seq( 0, Event::CreateNode { id: 1, node_type: "symptom".into()}),
            SequencedEvent::from_seq( 1, Event::SetProperty { node_id: 1, key: "name".into(), value: "cough".into()}),
        ];

        let g = semantic_project(&kernel_events, &onto);
        assert_eq!(g.nodes.len(), 1);
        let node = g.nodes.get(&1).unwrap();
        assert_eq!(node.semantic_type_name.as_deref(), Some("symptom"));
        assert_eq!(node.properties.get("name").map(|s| s.as_str()), Some("cough"));
    }

    // ── Test 9: Identity stability across repeated projections ───────────

    #[test]
    fn test_identity_stability() {
        let events = make_kernel_events();
        let onto = make_ontology();

        let g1 = semantic_project(&events, &onto);
        let g2 = semantic_project(&events, &onto);
        let g3 = semantic_project(&events, &onto);

        // Bit-for-bit identity across 3 calls
        assert_eq!(g1, g2);
        assert_eq!(g2, g3);
        assert_eq!(g1.to_deterministic_string(), g3.to_deterministic_string());
    }

    #[test]
    fn test_node_identity_stable_across_projections() {
        let events = make_kernel_events();
        let onto = make_ontology();

        let g1 = semantic_project(&events, &onto);
        let g2 = semantic_project(&events, &onto);

        for (id, n1) in &g1.nodes {
            let n2 = g2.nodes.get(id).unwrap();
            assert_eq!(n1.id, n2.id);
            assert_eq!(n1.node_type, n2.node_type);
            assert_eq!(n1.semantic_type_name, n2.semantic_type_name);
            assert_eq!(n1.properties, n2.properties);
        }
    }

    // ── Test 10: Regression tests against known event sequences ──────────

    #[test]
    fn test_regression_known_sequence() {
        // Known sequence: create two nodes with matching semantic types + one edge
        let events = vec![
            SequencedEvent::from_seq( 0, Event::CreateNode { id: 100, node_type: "person".into()}),
            SequencedEvent::from_seq( 1, Event::SetProperty { node_id: 100, key: "name".into(), value: "Bob".into()}),
            SequencedEvent::from_seq( 2, Event::CreateNode { id: 200, node_type: "organization".into()}),
            SequencedEvent::from_seq( 3, Event::SetProperty { node_id: 200, key: "name".into(), value: "Corp".into()}),
            SequencedEvent::from_seq( 4, Event::CreateEdge { id: 50, from_node: 100, to_node: 200, edge_type: "works_at".into()}),
        ];
        let onto = make_ontology();
        let g = semantic_project(&events, &onto);

        // Expected:
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);

        // Node 100 is a person
        let n100 = g.nodes.get(&100).unwrap();
        assert_eq!(n100.node_type, "person");
        assert_eq!(n100.semantic_type_name.as_deref(), Some("person"));
        assert_eq!(n100.properties.get("name").map(|s| s.as_str()), Some("Bob"));

        // Node 200 is an organization
        let n200 = g.nodes.get(&200).unwrap();
        assert_eq!(n200.node_type, "organization");
        assert_eq!(n200.semantic_type_name.as_deref(), Some("organization"));

        // Edge 50 is works_at
        let e50 = g.edges.get(&50).unwrap();
        assert_eq!(e50.edge_type, "works_at");
        assert_eq!(e50.semantic_type_name.as_deref(), Some("works_at"));
        assert_eq!(e50.from_node_type.as_deref(), Some("person"));
        assert_eq!(e50.to_node_type.as_deref(), Some("organization"));
    }

    #[test]
    fn test_regression_properties_carried_through() {
        let events = vec![
            SequencedEvent::from_seq( 0, Event::CreateNode { id: 1, node_type: "person".into()}),
            SequencedEvent::from_seq( 1, Event::SetProperty { node_id: 1, key: "age".into(), value: "30".into()}),
            SequencedEvent::from_seq( 2, Event::SetProperty { node_id: 1, key: "email".into(), value: "a@b.com".into()}),
        ];
        let onto = make_ontology();
        let g = semantic_project(&events, &onto);

        let node = g.nodes.get(&1).unwrap();
        assert_eq!(node.properties.len(), 2);
        assert_eq!(node.properties.get("age").map(|s| s.as_str()), Some("30"));
        assert_eq!(node.properties.get("email").map(|s| s.as_str()), Some("a@b.com"));
    }

    // ── Deserialization failure cases ────────────────────────────────────

    #[test]
    fn test_semantic_graph_deserialize_invalid() {
        assert!(SemanticGraph::from_deterministic_string("").is_none());
        assert!(SemanticGraph::from_deterministic_string("not json").is_none());
        assert!(SemanticGraph::from_deterministic_string("{}").is_none());
        assert!(SemanticGraph::from_deterministic_string(r#"{"nodes":{}"#).is_none());
    }

    #[test]
    fn test_typed_node_deserialize_invalid() {
        assert!(TypedNode::from_deterministic_string("").is_none());
        assert!(TypedNode::from_deterministic_string(r#"{"id":1}"#).is_none()); // missing fields
    }

    #[test]
    fn test_typed_edge_deserialize_invalid() {
        assert!(TypedEdge::from_deterministic_string("").is_none());
        assert!(TypedEdge::from_deterministic_string(r#"{"id":1}"#).is_none()); // missing fields
    }
}
