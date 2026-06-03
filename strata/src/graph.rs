use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    pub properties: BTreeMap<String, serde_json::Value>,
}

impl Node {
    pub fn new(id: &str) -> Self {
        Node { id: id.to_string(), properties: BTreeMap::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub properties: BTreeMap<String, serde_json::Value>,
}

impl Edge {
    pub fn new(id: &str, from: &str, to: &str, edge_type: &str) -> Self {
        Edge {
            id: id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            edge_type: edge_type.to_string(),
            properties: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphState {
    pub nodes: BTreeMap<String, Node>,
    pub edges: BTreeMap<String, Edge>,
}

impl GraphState {
    pub fn empty() -> Self {
        GraphState { nodes: BTreeMap::new(), edges: BTreeMap::new() }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}
