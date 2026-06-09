use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IntentType {
    QueryGraph,
    QueryOntology,
    QuerySemantic,
    DescribeNode,
    DescribeGraph,
    Unknown,
}

impl IntentType {
    pub fn name(&self) -> &'static str {
        match self {
            IntentType::QueryGraph => "QueryGraph",
            IntentType::QueryOntology => "QueryOntology",
            IntentType::QuerySemantic => "QuerySemantic",
            IntentType::DescribeNode => "DescribeNode",
            IntentType::DescribeGraph => "DescribeGraph",
            IntentType::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticIntent {
    pub id: String,
    pub raw_input: String,
    pub intent_type: IntentType,
    pub confidence: f64,
    pub extracted_entities: BTreeMap<String, String>,
    pub extracted_properties: BTreeMap<String, String>,
}

impl PartialEq for SemanticIntent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.raw_input == other.raw_input
            && self.intent_type == other.intent_type
            && self.confidence.to_bits() == other.confidence.to_bits()
            && self.extracted_entities == other.extracted_entities
            && self.extracted_properties == other.extracted_properties
    }
}

impl Eq for SemanticIntent {}

impl PartialOrd for SemanticIntent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemanticIntent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id
            .cmp(&other.id)
            .then_with(|| self.raw_input.cmp(&other.raw_input))
            .then_with(|| self.intent_type.cmp(&other.intent_type))
            .then_with(|| self.confidence.to_bits().cmp(&other.confidence.to_bits()))
            .then_with(|| self.extracted_entities.cmp(&other.extracted_entities))
            .then_with(|| self.extracted_properties.cmp(&other.extracted_properties))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticQuery {
    pub nodes: Option<Vec<u64>>,
    pub edges: Option<Vec<u64>>,
    pub node_filters: BTreeMap<String, String>,
    pub edge_filters: BTreeMap<String, String>,
    pub depth: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticResponse {
    pub intent: SemanticIntent,
    pub query: Option<SemanticQuery>,
    pub explanation: String,
}

fn compute_deterministic_id(input: &str) -> String {
    let mut h: u64 = 5381;
    for b in input.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    format!("int_{:016x}", h)
}

impl SemanticIntent {
    pub fn new(
        raw_input: &str,
        intent_type: IntentType,
        entities: BTreeMap<String, String>,
        properties: BTreeMap<String, String>,
    ) -> Self {
        let id = compute_deterministic_id(raw_input);
        SemanticIntent {
            id,
            raw_input: raw_input.to_string(),
            intent_type,
            confidence: 1.0,
            extracted_entities: entities,
            extracted_properties: properties,
        }
    }
}

impl SemanticQuery {
    pub fn new() -> Self {
        SemanticQuery {
            nodes: None,
            edges: None,
            node_filters: BTreeMap::new(),
            edge_filters: BTreeMap::new(),
            depth: None,
        }
    }
}

impl Default for SemanticQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_intent_creation() {
        let entities = BTreeMap::from([("Person".to_string(), "entity".to_string())]);
        let properties = BTreeMap::from([("age".to_string(), "30".to_string())]);
        let intent = SemanticIntent::new("find Person age:30", IntentType::QueryGraph, entities, properties);
        assert!(intent.id.starts_with("int_"));
        assert_eq!(intent.raw_input, "find Person age:30");
        assert_eq!(intent.intent_type, IntentType::QueryGraph);
        assert!((intent.confidence - 1.0).abs() < f64::EPSILON);
        assert_eq!(intent.extracted_entities.len(), 1);
        assert_eq!(intent.extracted_properties.len(), 1);
    }

    #[test]
    fn test_semantic_intent_id_deterministic() {
        let e = BTreeMap::new();
        let p = BTreeMap::new();
        let a = SemanticIntent::new("same input", IntentType::Unknown, e.clone(), p.clone());
        let b = SemanticIntent::new("same input", IntentType::Unknown, e, p);
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn test_different_inputs_different_ids() {
        let e = BTreeMap::new();
        let p = BTreeMap::new();
        let a = SemanticIntent::new("input one", IntentType::QueryGraph, e.clone(), p.clone());
        let b = SemanticIntent::new("input two", IntentType::QueryGraph, e, p);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_semantic_query_default() {
        let q = SemanticQuery::new();
        assert!(q.nodes.is_none());
        assert!(q.edges.is_none());
        assert!(q.node_filters.is_empty());
        assert!(q.edge_filters.is_empty());
        assert!(q.depth.is_none());
    }

    #[test]
    fn test_semantic_response_roundtrip() {
        let intent = SemanticIntent::new("test", IntentType::Unknown, BTreeMap::new(), BTreeMap::new());
        let response = SemanticResponse {
            intent,
            query: None,
            explanation: "test explanation".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: SemanticResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.explanation, parsed.explanation);
        assert_eq!(response.intent.id, parsed.intent.id);
        assert_eq!(response.intent.intent_type, parsed.intent.intent_type);
    }

    #[test]
    fn test_compute_id_deterministic() {
        let a = compute_deterministic_id("hello world");
        let b = compute_deterministic_id("hello world");
        assert_eq!(a, b);
        let c = compute_deterministic_id("hello world!");
        assert_ne!(a, c);
    }

    #[test]
    fn test_intent_type_ordering() {
        assert!(IntentType::DescribeGraph > IntentType::DescribeNode);
        assert!(IntentType::Unknown > IntentType::QuerySemantic);
    }

    #[test]
    fn test_semantic_query_with_values() {
        let mut q = SemanticQuery::new();
        q.nodes = Some(vec![1, 2, 3]);
        q.edges = Some(vec![10, 20]);
        q.depth = Some(2);
        q.node_filters.insert("type".to_string(), "Person".to_string());
        assert_eq!(q.nodes.as_ref().unwrap().len(), 3);
        assert_eq!(q.edges.as_ref().unwrap().len(), 2);
        assert_eq!(q.depth, Some(2));
        assert_eq!(q.node_filters.get("type").unwrap(), "Person");
    }
}
