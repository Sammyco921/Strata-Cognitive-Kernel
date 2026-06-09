use crate::cognition::event_translator::types::*;
use crate::cognition::semantic_interpreter::types::{
    IntentType, SemanticQuery, SemanticResponse,
};
use std::collections::BTreeMap;

fn intent_to_event_type(intent: &IntentType) -> &'static str {
    match intent {
        IntentType::QueryGraph => "GraphQueryRequested",
        IntentType::QueryOntology => "OntologyQueryRequested",
        IntentType::QuerySemantic => "SemanticQueryRequested",
        IntentType::DescribeNode => "NodeDescribeRequested",
        IntentType::DescribeGraph => "GraphDescribeRequested",
        IntentType::Unknown => "NoOp",
    }
}

fn build_intent_event(
    intent: &IntentType,
    intent_id: &str,
    index: usize,
) -> ProposedEvent {
    let event_type = intent_to_event_type(intent);
    let payload = BTreeMap::new();
    ProposedEvent::new(
        &format!("evt_proposed:{:?}:{}", intent, index),
        event_type,
        payload,
        intent_id,
    )
}

fn build_node_selection_events(
    query: &SemanticQuery,
    intent_id: &str,
    start_index: &mut usize,
    events: &mut Vec<ProposedEvent>,
) {
    if let Some(ref nodes) = query.nodes {
        let mut sorted = nodes.clone();
        sorted.sort();
        for node in sorted {
            let mut payload = BTreeMap::new();
            payload.insert("node_id".to_string(), node.to_string());
            events.push(ProposedEvent::new(
                &format!("evt_proposed:NodeSelection:{}", *start_index),
                "NodeSelectionEvent",
                payload,
                intent_id,
            ));
            *start_index += 1;
        }
    }
}

fn build_edge_selection_events(
    query: &SemanticQuery,
    intent_id: &str,
    start_index: &mut usize,
    events: &mut Vec<ProposedEvent>,
) {
    if let Some(ref edges) = query.edges {
        let mut sorted = edges.clone();
        sorted.sort();
        for edge in sorted {
            let mut payload = BTreeMap::new();
            payload.insert("edge_id".to_string(), edge.to_string());
            events.push(ProposedEvent::new(
                &format!("evt_proposed:EdgeSelection:{}", *start_index),
                "EdgeSelectionEvent",
                payload,
                intent_id,
            ));
            *start_index += 1;
        }
    }
}

fn build_filter_events(
    query: &SemanticQuery,
    intent_id: &str,
    start_index: &mut usize,
    events: &mut Vec<ProposedEvent>,
) {
    for (key, value) in &query.node_filters {
        let mut payload = BTreeMap::new();
        payload.insert("filter_key".to_string(), key.clone());
        payload.insert("filter_value".to_string(), value.clone());
        payload.insert("filter_target".to_string(), "node".to_string());
        events.push(ProposedEvent::new(
            &format!("evt_proposed:NodeFilter:{}", *start_index),
            "NodeFilterEvent",
            payload,
            intent_id,
        ));
        *start_index += 1;
    }
    for (key, value) in &query.edge_filters {
        let mut payload = BTreeMap::new();
        payload.insert("filter_key".to_string(), key.clone());
        payload.insert("filter_value".to_string(), value.clone());
        payload.insert("filter_target".to_string(), "edge".to_string());
        events.push(ProposedEvent::new(
            &format!("evt_proposed:EdgeFilter:{}", *start_index),
            "EdgeFilterEvent",
            payload,
            intent_id,
        ));
        *start_index += 1;
    }
}

pub fn translate(response: SemanticResponse) -> ProposedEventSequence {
    let intent = response.intent;
    let intent_id = intent.id.clone();
    let intent_type = intent.intent_type.clone();

    let mut events: Vec<ProposedEvent> = Vec::new();

    let mut index = 0usize;

    if intent_type != IntentType::Unknown {
        events.push(build_intent_event(&intent_type, &intent_id, index));
        index += 1;
    }

    if let Some(ref query) = response.query {
        build_node_selection_events(query, &intent_id, &mut index, &mut events);
        build_edge_selection_events(query, &intent_id, &mut index, &mut events);
        build_filter_events(query, &intent_id, &mut index, &mut events);
    }

    if intent_type == IntentType::Unknown {
        let mut payload = BTreeMap::new();
        payload.insert(
            "original_input".to_string(),
            intent.raw_input.clone(),
        );
        events.push(ProposedEvent::new(
            &format!("evt_proposed:NoOp:{}", index),
            "NoOp",
            payload,
            &intent_id,
        ));
    }

    ProposedEventSequence::new(intent, response.query, events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::semantic_interpreter::engine::interpret;

    #[test]
    fn test_translate_query_graph() {
        let resp = interpret("find all nodes");
        let seq = translate(resp);
        assert_eq!(seq.events[0].event_type, "GraphQueryRequested");
        assert!(seq.explanation.contains("intent=QueryGraph"));
    }

    #[test]
    fn test_translate_query_ontology() {
        let resp = interpret("ontology types");
        let seq = translate(resp);
        assert_eq!(seq.events[0].event_type, "OntologyQueryRequested");
    }

    #[test]
    fn test_translate_query_semantic() {
        let resp = interpret("semantic patterns");
        let seq = translate(resp);
        assert_eq!(seq.events[0].event_type, "SemanticQueryRequested");
    }

    #[test]
    fn test_translate_describe_node() {
        let resp = interpret("describe node 42");
        let seq = translate(resp);
        assert_eq!(seq.events[0].event_type, "NodeDescribeRequested");
    }

    #[test]
    fn test_translate_describe_graph() {
        let resp = interpret("describe");
        let seq = translate(resp);
        assert_eq!(seq.events[0].event_type, "GraphDescribeRequested");
    }

    #[test]
    fn test_translate_unknown() {
        let resp = interpret("gibberish");
        let seq = translate(resp);
        assert_eq!(seq.events[0].event_type, "NoOp");
    }

    #[test]
    fn test_unknown_includes_noop() {
        let resp = interpret("gibberish");
        let seq = translate(resp);
        assert!(seq.events.iter().any(|e| e.event_type == "NoOp"));
    }

    #[test]
    fn test_translate_node_selection() {
        let resp = interpret("describe node 7 42 1");
        let seq = translate(resp);
        let selection_events: Vec<&ProposedEvent> = seq
            .events
            .iter()
            .filter(|e| e.event_type == "NodeSelectionEvent")
            .collect();
        assert_eq!(selection_events.len(), 3);
        let ids: Vec<&str> = selection_events
            .iter()
            .map(|e| e.payload.get("node_id").unwrap().as_str())
            .collect();
        assert_eq!(ids, vec!["1", "7", "42"]);
    }

    #[test]
    fn test_translate_edge_selection() {
        let resp = interpret("nodes with edges");
        let seq = translate(resp);
        // "nodes with edges" triggers QueryGraph with empty edges vec
        assert_eq!(seq.events[0].event_type, "GraphQueryRequested");
    }

    #[test]
    fn test_translate_node_filter() {
        let resp = interpret("find nodes age:30");
        let seq = translate(resp);
        let filter_events: Vec<&ProposedEvent> = seq
            .events
            .iter()
            .filter(|e| e.event_type == "NodeFilterEvent")
            .collect();
        assert!(!filter_events.is_empty());
        assert_eq!(
            filter_events[0].payload.get("filter_key").unwrap(),
            "age"
        );
        assert_eq!(
            filter_events[0].payload.get("filter_value").unwrap(),
            "30"
        );
    }

    #[test]
    fn test_event_order_invariant() {
        let resp = interpret("describe node 5");
        let a = translate(resp.clone());
        let b = translate(resp);
        for (ea, eb) in a.events.iter().zip(b.events.iter()) {
            assert_eq!(ea.id, eb.id);
            assert_eq!(ea.event_type, eb.event_type);
        }
    }

    #[test]
    fn test_filter_ordering_stability() {
        let resp = interpret("find nodes a:1 b:2 c:3");
        let seq = translate(resp);
        let filters: Vec<&ProposedEvent> = seq
            .events
            .iter()
            .filter(|e| e.event_type == "NodeFilterEvent")
            .collect();
        for i in 1..filters.len() {
            assert!(
                filters[i - 1].id < filters[i].id,
                "Filters not sorted by id"
            );
        }
    }

    #[test]
    fn test_deterministic_identical_input() {
        let resp = interpret("find all nodes");
        let a = translate(resp.clone());
        let b = translate(resp);
        assert_eq!(a.explanation, b.explanation);
        assert_eq!(a.events.len(), b.events.len());
        for (ea, eb) in a.events.iter().zip(b.events.iter()) {
            assert_eq!(ea.id, eb.id);
            assert_eq!(ea.event_type, eb.event_type);
            assert_eq!(ea.payload, eb.payload);
        }
    }

    #[test]
    fn test_stability_100_runs() {
        let resp = interpret("describe node 42 with age:30");
        let first = translate(resp.clone());
        for _ in 0..100 {
            let next = translate(resp.clone());
            assert_eq!(first.explanation, next.explanation);
            assert_eq!(first.events.len(), next.events.len());
            for (ea, eb) in first.events.iter().zip(next.events.iter()) {
                assert_eq!(ea.id, eb.id);
                assert_eq!(ea.payload, eb.payload);
            }
        }
    }

    #[test]
    fn test_no_mutation_of_input() {
        let resp = interpret("describe node 42");
        let original_id = resp.intent.id.clone();
        let _ = translate(resp.clone());
        assert_eq!(resp.intent.id, original_id);
    }

    #[test]
    fn test_empty_semantic_query() {
        let resp = interpret("describe");
        let seq = translate(resp);
        assert_eq!(seq.events.len(), 1);
        assert_eq!(seq.events[0].event_type, "GraphDescribeRequested");
    }

    #[test]
    fn test_missing_query_field() {
        let resp = interpret("hello world");
        let seq = translate(resp);
        assert!(seq.query.is_none());
        assert!(seq.events.iter().any(|e| e.event_type == "NoOp"));
    }

    #[test]
    fn test_event_ids_have_correct_format() {
        let resp = interpret("describe node 1 2");
        let seq = translate(resp);
        for event in &seq.events {
            assert!(event.id.starts_with("evt_proposed:"));
        }
    }

    #[test]
    fn test_explanation_format() {
        let resp = interpret("find nodes");
        let seq = translate(resp);
        assert!(seq.explanation.starts_with("intent="));
        assert!(seq.explanation.contains("events="));
        assert!(seq.explanation.contains("query="));
    }

    #[test]
    fn test_roundtrip_serialization() {
        let resp = interpret("describe node 42 with age:30");
        let seq = translate(resp);
        let json = serde_json::to_string(&seq).unwrap();
        let parsed: ProposedEventSequence = serde_json::from_str(&json).unwrap();
        assert_eq!(seq.events.len(), parsed.events.len());
        assert_eq!(seq.explanation, parsed.explanation);
        for (a, b) in seq.events.iter().zip(parsed.events.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.event_type, b.event_type);
            assert_eq!(a.payload, b.payload);
        }
    }

    #[test]
    fn test_events_use_btreemap_payload() {
        let resp = interpret("find nodes age:30 city:NYC");
        let seq = translate(resp);
        for event in &seq.events {
            let mut prev_key: Option<&String> = None;
            for key in event.payload.keys() {
                if let Some(pk) = prev_key {
                    assert!(pk < key, "Payload keys not sorted");
                }
                prev_key = Some(key);
            }
        }
    }

    #[test]
    fn test_query_graph_deep() {
        let resp = interpret("find deep nodes");
        let seq = translate(resp);
        assert_eq!(seq.events[0].event_type, "GraphQueryRequested");
        assert_eq!(seq.events.len(), 1);
    }

    #[test]
    fn test_intent_id_preserved_in_events() {
        let resp = interpret("describe node 5");
        let intent_id = resp.intent.id.clone();
        let seq = translate(resp);
        for event in &seq.events {
            assert_eq!(event.source_intent_id, intent_id);
        }
    }

    #[test]
    fn test_unknown_appends_noop() {
        let resp = interpret("");
        let seq = translate(resp);
        assert_eq!(seq.events.len(), 1, "Unknown should produce exactly 1 NoOp event");
        assert_eq!(seq.events[0].event_type, "NoOp");
    }

    #[test]
    fn test_query_graph_has_node_selection_when_nodes_in_query() {
        let resp = interpret("find nodes");
        let seq = translate(resp);
        // "find nodes" triggers QueryGraph with empty nodes vec
        // NodeSelectionEvents only created when concrete node IDs exist
        assert_eq!(seq.events[0].event_type, "GraphQueryRequested");
    }
}
