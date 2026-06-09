use crate::cognition::semantic_interpreter::types::*;
use std::collections::BTreeMap;

fn classify_intent(input: &str) -> IntentType {
    let lower = input.to_lowercase();
    if lower.contains("describe node") {
        IntentType::DescribeNode
    } else if lower.contains("describe") {
        IntentType::DescribeGraph
    } else if lower.contains("semantic") {
        IntentType::QuerySemantic
    } else if lower.contains("ontology") {
        IntentType::QueryOntology
    } else if lower.contains("node") || lower.contains("nodes") {
        IntentType::QueryGraph
    } else {
        IntentType::Unknown
    }
}

fn extract_entities(input: &str) -> BTreeMap<String, String> {
    let mut entities = BTreeMap::new();
    for token in input.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric());
        if cleaned.is_empty() {
            continue;
        }
        if cleaned.starts_with(|c: char| c.is_uppercase())
            && !cleaned.chars().all(|c| c.is_uppercase())
            && cleaned.len() > 1
        {
            entities.insert(cleaned.to_string(), "entity".to_string());
        }
    }
    entities
}

fn extract_properties(input: &str) -> BTreeMap<String, String> {
    let mut properties = BTreeMap::new();
    for token in input.split_whitespace() {
        if let Some(idx) = token.find(':') {
            let key = token[..idx].trim();
            let value = token[idx + 1..].trim_matches(|c: char| !c.is_alphanumeric());
            if !key.is_empty() && !value.is_empty() {
                properties.insert(key.to_string(), value.to_string());
            }
        }
    }
    properties
}

fn infer_query(intent_type: &IntentType, input: &str) -> Option<SemanticQuery> {
    match intent_type {
        IntentType::QueryGraph => {
            let mut q = SemanticQuery::new();
            let lower = input.to_lowercase();
            if lower.contains("edge") || lower.contains("edges") {
                q.edges = Some(Vec::new());
            }
            if lower.contains("node") || lower.contains("nodes") {
                q.nodes = Some(Vec::new());
            }
            if lower.contains("deep") {
                q.depth = Some(3);
            }
            for (key, value) in &extract_properties(input) {
                q.node_filters.insert(key.clone(), value.clone());
            }
            Some(q)
        }
        IntentType::DescribeNode => {
            let mut q = SemanticQuery::new();
            let nodes: Vec<u64> = input
                .split_whitespace()
                .filter_map(|t| t.parse::<u64>().ok())
                .collect();
            if !nodes.is_empty() {
                q.nodes = Some(nodes);
            }
            Some(q)
        }
        IntentType::QueryOntology => Some(SemanticQuery::new()),
        _ => None,
    }
}

fn build_explanation(
    intent_type: &IntentType,
    entities: &BTreeMap<String, String>,
    properties: &BTreeMap<String, String>,
) -> String {
    let base = match intent_type {
        IntentType::QueryGraph => "Classified as QueryGraph: searching graph structure".to_string(),
        IntentType::QueryOntology => "Classified as QueryOntology: querying ontology definitions".to_string(),
        IntentType::QuerySemantic => "Classified as QuerySemantic: querying semantic layer".to_string(),
        IntentType::DescribeNode => "Classified as DescribeNode: describing specific nodes".to_string(),
        IntentType::DescribeGraph => "Classified as DescribeGraph: describing graph overview".to_string(),
        IntentType::Unknown => "Classified as Unknown: no matching intent pattern".to_string(),
    };
    let mut parts = vec![base];
    if !entities.is_empty() {
        let entity_list: Vec<&str> = entities.keys().map(|s| s.as_str()).collect();
        parts.push(format!("Entities: {}", entity_list.join(", ")));
    }
    if !properties.is_empty() {
        let prop_list: Vec<String> = properties
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        parts.push(format!("Properties: {}", prop_list.join(", ")));
    }
    parts.join(". ")
}

pub fn interpret(input: &str) -> SemanticResponse {
    let intent_type = classify_intent(input);
    let entities = extract_entities(input);
    let properties = extract_properties(input);
    let intent = SemanticIntent::new(input, intent_type.clone(), entities.clone(), properties.clone());
    let query = infer_query(&intent_type, input);
    let explanation = build_explanation(&intent_type, &entities, &properties);
    SemanticResponse {
        intent,
        query,
        explanation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpret_query_graph_nodes() {
        let res = interpret("find all nodes with Person type");
        assert_eq!(res.intent.intent_type, IntentType::QueryGraph);
        assert!(res.query.is_some());
        let q = res.query.unwrap();
        assert_eq!(q.nodes, Some(Vec::new()));
    }

    #[test]
    fn test_interpret_query_graph_edges() {
        let res = interpret("show edges between nodes");
        assert_eq!(res.intent.intent_type, IntentType::QueryGraph);
        let q = res.query.unwrap();
        assert_eq!(q.edges, Some(Vec::new()));
        assert_eq!(q.nodes, Some(Vec::new()));
    }

    #[test]
    fn test_interpret_query_ontology() {
        let res = interpret("what is in the ontology");
        assert_eq!(res.intent.intent_type, IntentType::QueryOntology);
        assert!(res.query.is_some());
    }

    #[test]
    fn test_interpret_query_semantic() {
        let res = interpret("run semantic analysis");
        assert_eq!(res.intent.intent_type, IntentType::QuerySemantic);
        assert!(res.query.is_none());
    }

    #[test]
    fn test_interpret_describe_node() {
        let res = interpret("describe node 42");
        assert_eq!(res.intent.intent_type, IntentType::DescribeNode);
        let q = res.query.unwrap();
        assert_eq!(q.nodes, Some(vec![42]));
    }

    #[test]
    fn test_interpret_describe_node_multiple() {
        let res = interpret("describe node 1 2 3");
        assert_eq!(res.intent.intent_type, IntentType::DescribeNode);
        let q = res.query.unwrap();
        assert_eq!(q.nodes, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_interpret_describe_graph() {
        let res = interpret("describe the entire graph");
        assert_eq!(res.intent.intent_type, IntentType::DescribeGraph);
        assert!(res.query.is_none());
    }

    #[test]
    fn test_interpret_describe_priority_over_describe_node() {
        let res = interpret("describe");
        assert_eq!(res.intent.intent_type, IntentType::DescribeGraph);
    }

    #[test]
    fn test_interpret_unknown() {
        let res = interpret("hello world");
        assert_eq!(res.intent.intent_type, IntentType::Unknown);
        assert!(res.query.is_none());
    }

    #[test]
    fn test_entity_extraction() {
        let res = interpret("find Person named Alice in Paris");
        assert!(res.intent.extracted_entities.contains_key("Person"));
        assert!(res.intent.extracted_entities.contains_key("Alice"));
        assert!(res.intent.extracted_entities.contains_key("Paris"));
    }

    #[test]
    fn test_property_extraction() {
        let res = interpret("find user age:30 city:NewYork");
        assert_eq!(res.intent.extracted_properties.get("age").unwrap(), "30");
        assert_eq!(res.intent.extracted_properties.get("city").unwrap(), "NewYork");
    }

    #[test]
    fn test_entities_sorted_deterministic() {
        let res = interpret("Alice Bob Charlie");
        let entities: Vec<&String> = res.intent.extracted_entities.keys().collect();
        for i in 1..entities.len() {
            assert!(entities[i - 1] < entities[i]);
        }
    }

    #[test]
    fn test_interpret_deterministic_identical_input() {
        let a = interpret("find all nodes");
        let b = interpret("find all nodes");
        assert_eq!(a.intent.id, b.intent.id);
        assert_eq!(a.intent.intent_type, b.intent.intent_type);
        assert_eq!(a.explanation, b.explanation);
        assert_eq!(a.query, b.query);
    }

    #[test]
    fn test_explanation_contains_classification() {
        let res = interpret("describe node 5");
        assert!(res.explanation.contains("DescribeNode"));
        let res2 = interpret("unknown text here");
        assert!(res2.explanation.contains("Unknown"));
    }

    #[test]
    fn test_explanation_contains_entity_info() {
        let res = interpret("find Person in City");
        assert!(res.explanation.contains("Entities:"));
        assert!(res.explanation.contains("Person"));
    }

    #[test]
    fn test_no_mutation_of_input() {
        let input = "describe node 42".to_string();
        let _ = interpret(&input);
        assert_eq!(input, "describe node 42");
    }

    #[test]
    fn test_multi_word_input() {
        let res = interpret("describe node 100 with all properties");
        assert_eq!(res.intent.intent_type, IntentType::DescribeNode);
        assert!(res.intent.extracted_entities.is_empty());
    }

    #[test]
    fn test_mixed_case_determinism() {
        let a = interpret("Describe Node 5");
        let b = interpret("describe node 5");
        assert_eq!(a.intent.intent_type, IntentType::DescribeNode);
        assert_eq!(b.intent.intent_type, IntentType::DescribeNode);
        assert_eq!(a.query, b.query);
    }

    #[test]
    fn test_whitespace_invariance() {
        let a = interpret("find  nodes ");
        let b = interpret("find nodes");
        assert_eq!(a.intent.intent_type, b.intent.intent_type);
        assert_eq!(a.explanation, b.explanation);
    }

    #[test]
    fn test_no_capitalized_entities() {
        let res = interpret("find all lowercase words here");
        assert!(res.intent.extracted_entities.is_empty());
    }

    #[test]
    fn test_key_value_with_colon_no_value() {
        let res = interpret("find key: ");
        assert!(res.intent.extracted_properties.is_empty());
    }

    #[test]
    fn test_stability_100_runs() {
        let first = interpret("describe node 42");
        for _ in 0..100 {
            let next = interpret("describe node 42");
            assert_eq!(first.intent.id, next.intent.id);
            assert_eq!(first.intent.intent_type, next.intent.intent_type);
            assert_eq!(first.explanation, next.explanation);
            assert_eq!(first.query, next.query);
        }
    }

    #[test]
    fn test_deep_query_graph() {
        let res = interpret("find deep nodes with type:Person");
        assert_eq!(res.intent.intent_type, IntentType::QueryGraph);
        let q = res.query.unwrap();
        assert_eq!(q.depth, Some(3));
        assert_eq!(q.node_filters.get("type").unwrap(), "Person");
    }

    #[test]
    fn test_entity_not_all_uppercase() {
        let res = interpret("find ABC DEF");
        assert!(res.intent.extracted_entities.is_empty());
    }

    #[test]
    fn test_semantic_priority_over_ontology() {
        let res = interpret("semantic ontology overlap");
        assert_eq!(res.intent.intent_type, IntentType::QuerySemantic);
    }

    #[test]
    fn test_describe_node_does_not_extract_numeric_as_entity() {
        let res = interpret("describe node 42");
        assert_eq!(res.intent.extracted_entities.len(), 0);
    }

    #[test]
    fn test_interpret_empty_string() {
        let res = interpret("");
        assert_eq!(res.intent.intent_type, IntentType::Unknown);
        assert!(res.query.is_none());
        assert!(res.explanation.contains("Unknown"));
    }

    #[test]
    fn test_explanation_contains_property_info() {
        let res = interpret("find user age:30");
        assert!(res.explanation.contains("Properties:"));
        assert!(res.explanation.contains("age=30"));
    }
}
