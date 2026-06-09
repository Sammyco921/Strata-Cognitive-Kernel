#[cfg(test)]
mod tests {
    use crate::ontology::*;
    use crate::kernel::*;

    // ── Helpers ────────────────────────────────────────────────────────────

    fn make_entity_event(name: &str, desc: Option<&str>, ts: u64) -> OntologyEvent {
        OntologyEvent::new(
            OntologyEventType::CreateEntityType,
            OntologyPayload::EntityType(EntityTypeDef {
                name: name.to_string(),
                description: desc.map(|s| s.to_string()),
            }),
            ts,
        )
    }

    fn make_relationship_event(name: &str, from: &str, to: &str, desc: Option<&str>, ts: u64) -> OntologyEvent {
        OntologyEvent::new(
            OntologyEventType::CreateRelationshipType,
            OntologyPayload::RelationshipType(RelationshipTypeDef {
                name: name.to_string(),
                from_entity: from.to_string(),
                to_entity: to.to_string(),
                description: desc.map(|s| s.to_string()),
            }),
            ts,
        )
    }

    fn make_property_event(name: &str, value_type: &str, desc: Option<&str>, ts: u64) -> OntologyEvent {
        OntologyEvent::new(
            OntologyEventType::CreatePropertyType,
            OntologyPayload::PropertyType(PropertyTypeDef {
                name: name.to_string(),
                value_type: value_type.to_string(),
                description: desc.map(|s| s.to_string()),
            }),
            ts,
        )
    }

    fn sample_events() -> Vec<OntologyEvent> {
        vec![
            make_entity_event("person", Some("A person entity"), 0),
            make_entity_event("organization", Some("An organization"), 1),
            make_relationship_event("works_at", "person", "organization", Some("Employment"), 2),
            make_property_event("name", "string", Some("Display name"), 3),
            make_property_event("age", "u64", None, 4),
        ]
    }

    // ── Test 1: Ontology event creation determinism ──────────────────────

    #[test]
    fn test_event_creation_determinism() {
        let e1 = make_entity_event("person", Some("A person"), 0);
        let e2 = make_entity_event("person", Some("A person"), 0);

        assert_eq!(e1.id, e2.id);
        assert_eq!(e1.timestamp, e2.timestamp);
        assert_eq!(e1.event_type, e2.event_type);
        assert_eq!(e1.payload, e2.payload);
        assert_eq!(e1, e2);

        let e3 = make_entity_event("person", Some("Different"), 0);
        assert_ne!(e1, e3, "different description should produce different event");
    }

    #[test]
    fn test_event_id_determinism() {
        let e1 = make_entity_event("person", None, 5);
        let e2 = make_entity_event("person", None, 5);
        assert_eq!(e1.id, "ont:et:person");
        assert_eq!(e1.id, e2.id);
    }

    #[test]
    fn test_different_events_have_different_ids() {
        let et = make_entity_event("person", None, 0);
        let rt = make_relationship_event("works_at", "person", "org", None, 1);
        let pt = make_property_event("name", "string", None, 2);
        assert_ne!(et.id, rt.id);
        assert_ne!(et.id, pt.id);
        assert_ne!(rt.id, pt.id);
    }

    // ── Test 2: Replay reconstruction of OntologyRegistry ─────────────────

    #[test]
    fn test_replay_basic() {
        let events = sample_events();
        let registry = replay_ontology(&events);

        assert_eq!(registry.entity_types.len(), 2);
        assert_eq!(registry.relationship_types.len(), 1);
        assert_eq!(registry.property_types.len(), 2);

        assert!(registry.entity_types.contains_key("person"));
        assert!(registry.entity_types.contains_key("organization"));
        assert!(registry.relationship_types.contains_key("works_at"));
        assert!(registry.property_types.contains_key("name"));
        assert!(registry.property_types.contains_key("age"));
    }

    #[test]
    fn test_replay_field_values() {
        let events = vec![
            make_entity_event("person", Some("Human being"), 0),
            make_relationship_event("employed_by", "person", "company", Some("Job"), 1),
        ];
        let registry = replay_ontology(&events);

        let person = registry.entity_types.get("person").unwrap();
        assert_eq!(person.description.as_deref(), Some("Human being"));

        let rel = registry.relationship_types.get("employed_by").unwrap();
        assert_eq!(rel.from_entity, "person");
        assert_eq!(rel.to_entity, "company");
        assert_eq!(rel.description.as_deref(), Some("Job"));
    }

    #[test]
    fn test_replay_empty_events() {
        let registry = replay_ontology(&[]);
        assert!(registry.entity_types.is_empty());
        assert!(registry.relationship_types.is_empty());
        assert!(registry.property_types.is_empty());
    }

    // ── Test 3: Event ordering correctness ────────────────────────────────

    #[test]
    fn test_event_ordering_affects_result() {
        let events_a = vec![
            make_entity_event("person", Some("v1"), 0),
            make_entity_event("person", Some("v2"), 1),
        ];
        let registry = replay_ontology(&events_a);
        assert_eq!(
            registry.entity_types.get("person").unwrap().description.as_deref(),
            Some("v2"),
            "later event should overwrite earlier"
        );

        let events_b = vec![
            make_entity_event("person", Some("v2"), 1),
            make_entity_event("person", Some("v1"), 0),
        ];
        let registry_b = replay_ontology_sorted(&mut events_b.clone());
        assert_eq!(
            registry_b.entity_types.get("person").unwrap().description.as_deref(),
            Some("v2"),
            "sorted by timestamp should give same result"
        );
    }

    #[test]
    fn test_replay_order_mismatch() {
        // If replay is NOT sorted by timestamp, later event wins regardless of timestamp
        let events = vec![
            make_entity_event("person", Some("earlier"), 10),
            make_entity_event("person", Some("later"), 0),
        ];
        let registry = replay_ontology(&events);
        assert_eq!(
            registry.entity_types.get("person").unwrap().description.as_deref(),
            Some("later"),
            "last in sequence wins (not sorted)"
        );
    }

    // ── Test 4: Mixed event streams (kernel + ontology events) ────────────

    #[test]
    fn test_mixed_event_streams() {
        // Simulate a global event stream with shared sequencing
        // Kernel events use seq, ontology events use timestamp.
        // We interleave them to demonstrate they can coexist in one logical stream.
        let kernel_events = vec![
            SequencedEvent::from_seq(0, Event::CreateNode { id: 1, node_type: "person".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 2, node_type: "org".into() }),
            SequencedEvent::from_seq(4, Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "works_at".into() }),
        ];

        let ontology_events = vec![
            make_entity_event("person", None, 1),
            make_entity_event("organization", None, 3),
            make_relationship_event("works_at", "person", "organization", None, 5),
        ];

        // Demonstrate the mixed logical stream:
        // seq 0: kernel CreateNode{person}
        // seq 1: ontology CreateEntityType{person}
        // seq 2: kernel CreateNode{org}
        // seq 3: ontology CreateEntityType{organization}
        // seq 4: kernel CreateEdge{works_at}
        // seq 5: ontology CreateRelationshipType{works_at}

        // Replay kernel events independently
        let kernel_state = replay(&kernel_events);
        assert_eq!(kernel_state.node_count(), 2);
        assert_eq!(kernel_state.edge_count(), 1);

        // Replay ontology events independently
        let ontology_registry = replay_ontology(&ontology_events);
        assert_eq!(ontology_registry.entity_types.len(), 2);
        assert_eq!(ontology_registry.relationship_types.len(), 1);

        // Verify both state reconstructions are correct from the same conceptual stream
        let all_seqs: Vec<u64> = kernel_events.iter().map(|e| e.logical_seq).chain(ontology_events.iter().map(|e| e.timestamp)).collect();
        let mut sorted = all_seqs.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_mixed_stream_independent_replay() {
        // Verify that kernel and ontology events can be replayed independently
        // from a combined (conceptual) event log without interfering.
        let kernel_events = vec![
            SequencedEvent::from_seq(0, Event::CreateNode { id: 1, node_type: "a".into() }),
            SequencedEvent::from_seq(2, Event::CreateNode { id: 2, node_type: "b".into() }),
        ];
        let ontology_events = vec![
            make_entity_event("A", None, 1),
            make_entity_event("B", None, 3),
        ];

        let ks = replay(&kernel_events);
        let or = replay_ontology(&ontology_events);

        assert_eq!(ks.node_count(), 2);
        assert_eq!(or.entity_types.len(), 2);
        assert!(ks.get_node(1).is_some());
        assert!(or.entity_types.contains_key("A"));
    }

    // ── Test 5: Replay idempotence ────────────────────────────────────────

    #[test]
    fn test_replay_idempotence() {
        let events = sample_events();
        let registry1 = replay_ontology(&events);
        let registry2 = replay_ontology(&events);
        assert_eq!(registry1, registry2);
    }

    #[test]
    fn test_replay_idempotence_multiple_apply() {
        let events = sample_events();

        // Replay once
        let r1 = replay_ontology(&events);

        // Replay same events twice (simulating re-processing)
        let mut r2 = OntologyRegistry::empty();
        for _ in 0..2 {
            for e in &events {
                r2.apply_event(e);
            }
        }
        // Applying same events twice should be idempotent for same-event definitions
        // (later event just overwrites same key)
        assert_eq!(r1, r2);
    }

    // ── Test 6: No divergence between direct vs replayed registry ─────────

    #[test]
    fn test_no_divergence_direct_vs_replayed() {
        let events = sample_events();
        let replayed = replay_ontology(&events);

        // Build expected registry by directly applying events
        let mut expected = OntologyRegistry::empty();
        for e in &events {
            expected.apply_event(e);
        }
        assert_eq!(replayed, expected);

        // Also verify properties match
        for (name, et) in &replayed.entity_types {
            assert_eq!(expected.entity_types.get(name), Some(et));
        }
        for (name, rt) in &replayed.relationship_types {
            assert_eq!(expected.relationship_types.get(name), Some(rt));
        }
        for (name, pt) in &replayed.property_types {
            assert_eq!(expected.property_types.get(name), Some(pt));
        }
    }

    #[test]
    fn test_registry_fully_derived() {
        let events = sample_events();
        let registry = replay_ontology(&events);

        // The registry should be deterministically derivable from events.
        // No manual construction should be needed.
        // Verify: for each event, the registry has the expected type.
        for event in &events {
            match &event.payload {
                OntologyPayload::EntityType(def) => {
                    assert!(registry.entity_types.contains_key(&def.name));
                }
                OntologyPayload::RelationshipType(def) => {
                    assert!(registry.relationship_types.contains_key(&def.name));
                }
                OntologyPayload::PropertyType(def) => {
                    assert!(registry.property_types.contains_key(&def.name));
                }
            }
        }
    }

    // ── Test 7: Serialization determinism ─────────────────────────────────

    #[test]
    fn test_serialization_determinism() {
        let event = make_entity_event("test_entity", Some("A test"), 42);
        let s1 = event.to_deterministic_string();
        let s2 = event.to_deterministic_string();
        assert_eq!(s1, s2, "serialization must be deterministic");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let events = sample_events();
        for event in &events {
            let serialized = event.to_deterministic_string();
            let deserialized = OntologyEvent::from_deterministic_string(&serialized);
            assert!(deserialized.is_some(), "failed to deserialize: {}", serialized);
            assert_eq!(event, &deserialized.unwrap(), "roundtrip mismatch for: {:?}", event);
        }
    }

    #[test]
    fn test_serialization_roundtrip_all_types() {
        let test_events = vec![
            make_entity_event("alpha", Some("Entity A"), 10),
            make_entity_event("beta", None, 20),
            make_relationship_event("relates", "alpha", "beta", Some("Rel"), 30),
            make_relationship_event("owns", "alpha", "beta", None, 40),
            make_property_event("count", "u64", Some("Count value"), 50),
            make_property_event("label", "string", None, 60),
        ];
        for event in &test_events {
            let s = event.to_deterministic_string();
            let back = OntologyEvent::from_deterministic_string(&s).unwrap();
            assert_eq!(event, &back, "roundtrip failed for event: {:?}", event);
        }
    }

    #[test]
    fn test_serialization_roundtrip_with_causes() {
        let event = OntologyEvent::with_causes(
            OntologyEventType::CreateEntityType,
            OntologyPayload::EntityType(EntityTypeDef {
                name: "derived".to_string(),
                description: Some("Derived from other".to_string()),
            }),
            99,
            vec!["ont:et:base".to_string()],
        );
        let s = event.to_deterministic_string();
        let back = OntologyEvent::from_deterministic_string(&s).unwrap();
        assert_eq!(event, back);
        assert_eq!(back.causes, vec!["ont:et:base"]);
    }

    #[test]
    fn test_serialization_determinism_across_instances() {
        let e1 = make_entity_event("person", Some("A person"), 0);
        let e2 = make_entity_event("person", Some("A person"), 0);
        assert_eq!(e1.to_deterministic_string(), e2.to_deterministic_string());
    }

    // ── Test 8: Failure cases ─────────────────────────────────────────────

    #[test]
    fn test_deserialize_invalid_input() {
        assert!(OntologyEvent::from_deterministic_string("").is_none());
        assert!(OntologyEvent::from_deterministic_string("not json").is_none());
        assert!(OntologyEvent::from_deterministic_string("{}").is_none());
        assert!(OntologyEvent::from_deterministic_string(r#"{"id":"x"}"#).is_none());
    }

    #[test]
    fn test_deserialize_unknown_event_type() {
        let s = r#"{"id":"ont:xx:foo","timestamp":0,"event_type":"UnknownType","payload":{"EntityType":{"name":"foo","description":null}},"causes":[]}"#;
        assert!(OntologyEvent::from_deterministic_string(s).is_none());
    }

    #[test]
    fn test_deserialize_malformed_payload() {
        let s = r#"{"id":"ont:et:foo","timestamp":0,"event_type":"CreateEntityType","payload":{"BadType":{"name":"foo"}},"causes":[]}"#;
        assert!(OntologyEvent::from_deterministic_string(s).is_none());
    }

    // ── Additional: Event count / ordering consistency ────────────────────

    #[test]
    fn test_replay_applies_all_events() {
        let events = sample_events();
        let registry = replay_ontology(&events);
        assert_eq!(
            registry.entity_types.len() + registry.relationship_types.len() + registry.property_types.len(),
            events.len(),
            "every event should contribute to the registry"
        );
    }

    #[test]
    fn test_apply_event_twice_idempotent() {
        let mut r = OntologyRegistry::empty();
        let e = make_entity_event("x", Some("first"), 0);
        r.apply_event(&e);
        let e2 = make_entity_event("x", Some("second"), 1);
        r.apply_event(&e2);
        assert_eq!(r.entity_types.len(), 1);
        assert_eq!(r.entity_types.get("x").unwrap().description.as_deref(), Some("second"));
    }

    #[test]
    fn test_deterministic_replay_independent_of_event_reference() {
        // Events with no causal links must be order-independent when sorted
        let mut events_1 = vec![
            make_entity_event("a", None, 5),
            make_entity_event("b", None, 3),
            make_entity_event("c", None, 1),
        ];
        let mut events_2 = vec![
            make_entity_event("a", None, 5),
            make_entity_event("b", None, 3),
            make_entity_event("c", None, 1),
        ];
        events_2.reverse();

        let r1 = replay_ontology_sorted(&mut events_1);
        let r2 = replay_ontology_sorted(&mut events_2);
        assert_eq!(r1, r2, "sorted replay must be order-independent");
    }
}
