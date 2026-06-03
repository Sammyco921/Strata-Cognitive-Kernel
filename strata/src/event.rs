use serde::{Deserialize, Serialize};

use crate::version::SchemaVersion;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventType {
    CreateNode,
    CreateEdge,
    SetProperty,
    DeleteNode,
    DeleteEdge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: String,
    pub timestamp: u64,
    pub event_type: EventType,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub causes: Vec<String>,
    #[serde(default)]
    pub meta_reason: Option<String>,
}

impl Event {
    pub fn new(id: String, timestamp: u64, event_type: EventType, payload: serde_json::Value) -> Self {
        Event { id, timestamp, event_type, payload, causes: Vec::new(), meta_reason: None }
    }

    pub fn with_causes(id: String, timestamp: u64, event_type: EventType, payload: serde_json::Value, causes: Vec<String>, meta_reason: Option<String>) -> Self {
        Event { id, timestamp, event_type, payload, causes, meta_reason }
    }
}

/// B7-B: Event Envelope — wraps each persisted event with schema metadata.
/// This is the canonical on-disk storage format starting from B7.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope {
    pub schema_version: SchemaVersion,
    pub event: Event,
}

impl EventEnvelope {
    pub fn new(event: Event) -> Self {
        EventEnvelope {
            schema_version: SchemaVersion::default(),
            event,
        }
    }

    pub fn with_version(event: Event, schema_version: SchemaVersion) -> Self {
        EventEnvelope { schema_version, event }
    }
}
