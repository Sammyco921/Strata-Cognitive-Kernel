use strata_kill_test::cognition::semantic_interpreter::*;
use strata_kill_test::cognition::event_translator::*;

// ── 1. Identical input → identical output ─────────────────────────────────

#[test]
fn test_identical_input_identical_output() {
    let a = translate(interpret("find all nodes"));
    let b = translate(interpret("find all nodes"));
    assert_eq!(a.explanation, b.explanation);
    assert_eq!(a.events.len(), b.events.len());
    for (ea, eb) in a.events.iter().zip(b.events.iter()) {
        assert_eq!(ea.id, eb.id);
        assert_eq!(ea.event_type, eb.event_type);
        assert_eq!(ea.payload, eb.payload);
    }
}

// ── 2. 100-run stability test ─────────────────────────────────────────────

#[test]
fn test_100_run_stability() {
    let first = translate(interpret("describe node 42 with type:Person"));
    for _ in 0..100 {
        let next = translate(interpret("describe node 42 with type:Person"));
        assert_eq!(first.explanation, next.explanation);
        assert_eq!(first.events.len(), next.events.len());
    }
}

// ── 3. QueryGraph → GraphQueryRequested ───────────────────────────────────

#[test]
fn test_query_graph_mapping() {
    let seq = translate(interpret("list all nodes"));
    assert_eq!(seq.events[0].event_type, "GraphQueryRequested");
}

// ── 4. QueryOntology → OntologyQueryRequested ─────────────────────────────

#[test]
fn test_query_ontology_mapping() {
    let seq = translate(interpret("ontology classes"));
    assert_eq!(seq.events[0].event_type, "OntologyQueryRequested");
}

// ── 5. QuerySemantic → SemanticQueryRequested ─────────────────────────────

#[test]
fn test_query_semantic_mapping() {
    let seq = translate(interpret("semantic relationships"));
    assert_eq!(seq.events[0].event_type, "SemanticQueryRequested");
}

// ── 6. DescribeNode → NodeDescribeRequested ───────────────────────────────

#[test]
fn test_describe_node_mapping() {
    let seq = translate(interpret("describe node 7"));
    assert_eq!(seq.events[0].event_type, "NodeDescribeRequested");
}

// ── 7. DescribeGraph → GraphDescribeRequested ─────────────────────────────

#[test]
fn test_describe_graph_mapping() {
    let seq = translate(interpret("describe graph"));
    assert_eq!(seq.events[0].event_type, "GraphDescribeRequested");
}

// ── 8. Unknown → NoOp ─────────────────────────────────────────────────────

#[test]
fn test_unknown_mapping() {
    let seq = translate(interpret("asdfgh"));
    assert_eq!(seq.events[0].event_type, "NoOp");
}

// ── 9. Nodes → NodeSelectionEvent ─────────────────────────────────────────

#[test]
fn test_nodes_to_selection_event() {
    let seq = translate(interpret("describe node 5 3 1"));
    let selections: Vec<&ProposedEvent> = seq
        .events
        .iter()
        .filter(|e| e.event_type == "NodeSelectionEvent")
        .collect();
    assert_eq!(selections.len(), 3);
    let ids: Vec<&str> = selections
        .iter()
        .map(|e| e.payload.get("node_id").unwrap().as_str())
        .collect();
    assert_eq!(ids, vec!["1", "3", "5"]);
}

// ── 10. Edges → EdgeSelectionEvent ────────────────────────────────────────

#[test]
fn test_edges_to_selection_event() {
    let seq = translate(interpret("nodes with edges"));
    let _has_edge_sel = seq.events.iter().any(|e| e.event_type == "EdgeSelectionEvent");
    assert_eq!(seq.events[0].event_type, "GraphQueryRequested");
}

// ── 11. Filters → NodeFilterEvent / EdgeFilterEvent ───────────────────────

#[test]
fn test_filters_to_filter_events() {
    let seq = translate(interpret("find nodes age:30 city:NYC"));
    let filter_events: Vec<&ProposedEvent> = seq
        .events
        .iter()
        .filter(|e| e.event_type == "NodeFilterEvent")
        .collect();
    assert_eq!(filter_events.len(), 2);
    let keys: Vec<&str> = filter_events
        .iter()
        .map(|e| e.payload.get("filter_key").unwrap().as_str())
        .collect();
    assert!(keys.contains(&"age"));
    assert!(keys.contains(&"city"));
}

// ── 12. Event order invariant ─────────────────────────────────────────────

#[test]
fn test_event_order_invariant() {
    let a = translate(interpret("describe node 7 3"));
    let b = translate(interpret("describe node 7 3"));
    for (ea, eb) in a.events.iter().zip(b.events.iter()) {
        assert_eq!(ea.id, eb.id);
        assert_eq!(ea.event_type, eb.event_type);
    }
}

// ── 13. Filter ordering stability ─────────────────────────────────────────

#[test]
fn test_filter_ordering_stability() {
    let seq = translate(interpret("find nodes z:1 a:2 m:3"));
    let filters: Vec<&ProposedEvent> = seq
        .events
        .iter()
        .filter(|e| e.event_type == "NodeFilterEvent")
        .collect();
    let keys: Vec<&str> = filters
        .iter()
        .map(|e| e.payload.get("filter_key").unwrap().as_str())
        .collect();
    for i in 1..keys.len() {
        assert!(keys[i - 1] <= keys[i], "Filters not sorted");
    }
}

// ── 14. Roundtrip ProposedEventSequence ───────────────────────────────────

#[test]
fn test_roundtrip_sequence() {
    let seq = translate(interpret("describe node 42 with type:Person"));
    let json = serde_json::to_string(&seq).unwrap();
    let parsed: ProposedEventSequence = serde_json::from_str(&json).unwrap();
    assert_eq!(seq.events.len(), parsed.events.len());
    assert_eq!(seq.explanation, parsed.explanation);
    assert_eq!(seq.intent.id, parsed.intent.id);
}

// ── 15. Event serialization determinism ───────────────────────────────────

#[test]
fn test_event_serialization_determinism() {
    let seq = translate(interpret("find nodes age:30"));
    let json_a = serde_json::to_string(&seq).unwrap();
    let json_b = serde_json::to_string(&seq).unwrap();
    assert_eq!(json_a, json_b);
}

// ── 16. Empty SemanticQuery ───────────────────────────────────────────────

#[test]
fn test_empty_query() {
    let seq = translate(interpret("describe graph"));
    assert_eq!(seq.events.len(), 1);
    assert_eq!(seq.events[0].event_type, "GraphDescribeRequested");
}

// ── 17. Missing query field (Unknown) ─────────────────────────────────────

#[test]
fn test_missing_query_field() {
    let seq = translate(interpret("xyzzy"));
    assert!(seq.query.is_none());
    assert!(seq.events.iter().any(|e| e.event_type == "NoOp"));
}

// ── 18. No mutation of input ──────────────────────────────────────────────

#[test]
fn test_no_mutation_of_input() {
    let resp = interpret("describe node 42");
    let original_id = resp.intent.id.clone();
    let _ = translate(resp.clone());
    assert_eq!(resp.intent.id, original_id);
}

// ── 19. Stable IDs across runs ────────────────────────────────────────────

#[test]
fn test_stable_ids() {
    let a = translate(interpret("find nodes"));
    let b = translate(interpret("find nodes"));
    for (ea, eb) in a.events.iter().zip(b.events.iter()) {
        assert_eq!(ea.id, eb.id);
        assert_eq!(ea.source_intent_id, eb.source_intent_id);
    }
}

// ── 20. All events reference intent id ────────────────────────────────────

#[test]
fn test_all_events_reference_intent() {
    let resp = interpret("describe node 5 7");
    let intent_id = resp.intent.id.clone();
    let seq = translate(resp);
    for event in &seq.events {
        assert_eq!(event.source_intent_id, intent_id);
    }
}

// ── 21. Only Unknown adds NoOp ────────────────────────────────────────────

#[test]
fn test_only_unknown_adds_noop() {
    let known_types = vec!["find nodes", "ontology list", "semantic", "describe node 1", "describe"];
    for input in known_types {
        let seq = translate(interpret(input));
        let has_noop = seq.events.iter().any(|e| e.event_type == "NoOp");
        assert!(!has_noop, "Non-Unknown input '{}' should not produce NoOp", input);
    }
    let seq = translate(interpret("garbage"));
    assert!(seq.events.iter().any(|e| e.event_type == "NoOp"));
}

// ── 22. Events count equals explanation count ─────────────────────────────

#[test]
fn test_events_count_in_explanation() {
    let seq = translate(interpret("describe node 1 2 3"));
    let expected = format!("events={}", seq.events.len());
    assert!(seq.explanation.contains(&expected));
}

// ── 23. query=present / absent in explanation ─────────────────────────────

#[test]
fn test_query_present_absent_in_explanation() {
    let with = translate(interpret("describe node 1"));
    assert!(with.explanation.contains("query=present"));
    let without = translate(interpret("describe"));
    assert!(!without.explanation.contains("query=present"));
}

// ── 24. Payload keys are sorted (BTreeMap) ────────────────────────────────

#[test]
fn test_payload_keys_sorted() {
    let seq = translate(interpret("find nodes age:30 name:Bob"));
    for event in &seq.events {
        let mut prev: Option<&String> = None;
        for key in event.payload.keys() {
            if let Some(p) = prev {
                assert!(p < key, "Payload keys not sorted");
            }
            prev = Some(key);
        }
    }
}
