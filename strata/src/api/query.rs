use std::collections::BTreeMap;

use serde::Serialize;

use crate::kernel::event::{Event, EventType};
use crate::projection::causal::CausalChainLink;

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// Stable, serializable node representation for external consumers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NodeView {
    pub id: String,
    pub properties: BTreeMap<String, serde_json::Value>,
}

impl NodeView {
    pub(crate) fn from_node(n: &crate::kernel::graph::Node) -> Self {
        NodeView { id: n.id.clone(), properties: n.properties.clone() }
    }
}

/// Stable, serializable edge representation for external consumers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EdgeView {
    pub id: String,
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub properties: BTreeMap<String, serde_json::Value>,
}

impl EdgeView {
    pub(crate) fn from_edge(e: &crate::kernel::graph::Edge) -> Self {
        EdgeView {
            id: e.id.clone(),
            from: e.from.clone(),
            to: e.to.clone(),
            edge_type: e.edge_type.clone(),
            properties: e.properties.clone(),
        }
    }
}

/// Stable, serializable event representation for external consumers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EventView {
    pub id: String,
    pub timestamp: u64,
    pub event_type: EventType,
    pub payload: serde_json::Value,
}

impl EventView {
    pub(crate) fn from_event(e: &Event) -> Self {
        EventView {
            id: e.id.clone(),
            timestamp: e.timestamp,
            event_type: e.event_type.clone(),
            payload: e.payload.clone(),
        }
    }
}

/// Stable, serializable explanation result for external consumers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExplanationView {
    pub target_node_id: String,
    pub property_key: Option<String>,
    pub current_value: Option<serde_json::Value>,
    pub chain: Vec<CausalChainLink>,
    pub hops: usize,
}

impl ExplanationView {
    pub(crate) fn from_explanation(ex: &crate::projection::causal::Explanation) -> Self {
        ExplanationView {
            target_node_id: ex.target_node_id.clone(),
            property_key: ex.property_key.clone(),
            current_value: ex.current_value.clone(),
            chain: ex.chain.clone(),
            hops: ex.hops,
        }
    }
}

/// Snapshot metadata — version info and object counts without raw state.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SnapshotView {
    pub kernel_version: String,
    pub schema_version: String,
    pub last_event_timestamp: u64,
    pub node_count: usize,
    pub edge_count: usize,
}

// ── Helper: events that target a given node ──────────────────────────────────

/// Returns true if the event's payload references `node_id` as its subject.
pub(crate) fn event_targets_node(event: &Event, node_id: &str) -> bool {
    match event.event_type {
        EventType::CreateNode | EventType::DeleteNode => {
            event.payload.get("id").and_then(|v| v.as_str()) == Some(node_id)
        }
        EventType::SetProperty => {
            event.payload.get("target_id").and_then(|v| v.as_str()) == Some(node_id)
        }
        EventType::CreateEdge => {
            event.payload.get("from").and_then(|v| v.as_str()) == Some(node_id)
                || event.payload.get("to").and_then(|v| v.as_str()) == Some(node_id)
        }
        EventType::DeleteEdge => false,
    }
}
