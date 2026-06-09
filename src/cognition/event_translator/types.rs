use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::cognition::semantic_interpreter::types::{SemanticIntent, SemanticQuery};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedEvent {
    pub id: String,
    pub event_type: String,
    pub payload: BTreeMap<String, String>,
    pub confidence: f64,
    pub source_intent_id: String,
}

impl PartialEq for ProposedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.event_type == other.event_type
            && self.payload == other.payload
            && self.confidence.to_bits() == other.confidence.to_bits()
            && self.source_intent_id == other.source_intent_id
    }
}

impl Eq for ProposedEvent {}

impl PartialOrd for ProposedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProposedEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id
            .cmp(&other.id)
            .then_with(|| self.event_type.cmp(&other.event_type))
            .then_with(|| self.payload.cmp(&other.payload))
            .then_with(|| {
                self.confidence
                    .to_bits()
                    .cmp(&other.confidence.to_bits())
            })
            .then_with(|| self.source_intent_id.cmp(&other.source_intent_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProposedEventSequence {
    pub intent: SemanticIntent,
    pub query: Option<SemanticQuery>,
    pub events: Vec<ProposedEvent>,
    pub explanation: String,
}

impl ProposedEvent {
    pub fn new(
        id: &str,
        event_type: &str,
        payload: BTreeMap<String, String>,
        source_intent_id: &str,
    ) -> Self {
        ProposedEvent {
            id: id.to_string(),
            event_type: event_type.to_string(),
            payload,
            confidence: 1.0,
            source_intent_id: source_intent_id.to_string(),
        }
    }
}

impl ProposedEventSequence {
    pub fn new(
        intent: SemanticIntent,
        query: Option<SemanticQuery>,
        events: Vec<ProposedEvent>,
    ) -> Self {
        let explanation = format!(
            "intent={:?}; events={}; query={}",
            intent.intent_type,
            events.len(),
            if query.is_some() { "present" } else { "absent" }
        );
        ProposedEventSequence {
            intent,
            query,
            events,
            explanation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::semantic_interpreter::types::IntentType;

    fn make_sample_intent() -> SemanticIntent {
        SemanticIntent::new(
            "test",
            IntentType::QueryGraph,
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }

    #[test]
    fn test_proposed_event_creation() {
        let mut payload = BTreeMap::new();
        payload.insert("key".to_string(), "value".to_string());
        let evt = ProposedEvent::new("evt_proposed:QueryGraph:0", "GraphQueryRequested", payload, "int_abc");
        assert!(evt.id.starts_with("evt_proposed:"));
        assert_eq!(evt.event_type, "GraphQueryRequested");
        assert_eq!(evt.payload.get("key").unwrap(), "value");
        assert!((evt.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_proposed_event_eq() {
        let a = ProposedEvent::new("evt_0", "TestType", BTreeMap::new(), "int_1");
        let b = ProposedEvent::new("evt_0", "TestType", BTreeMap::new(), "int_1");
        assert_eq!(a, b);
    }

    #[test]
    fn test_proposed_event_sequence_creation() {
        let intent = make_sample_intent();
        let events = vec![
            ProposedEvent::new("evt_0", "GraphQueryRequested", BTreeMap::new(), &intent.id),
        ];
        let seq = ProposedEventSequence::new(intent.clone(), None, events);
        assert_eq!(seq.intent.intent_type, IntentType::QueryGraph);
        assert!(seq.query.is_none());
        assert_eq!(seq.events.len(), 1);
        assert!(seq.explanation.contains("intent=QueryGraph"));
        assert!(seq.explanation.contains("events=1"));
        assert!(seq.explanation.contains("query=absent"));
    }

    #[test]
    fn test_proposed_event_sequence_with_query() {
        let intent = make_sample_intent();
        let query = Some(SemanticQuery::new());
        let events = vec![
            ProposedEvent::new("evt_0", "GraphQueryRequested", BTreeMap::new(), &intent.id),
        ];
        let seq = ProposedEventSequence::new(intent, query, events);
        assert!(seq.explanation.contains("query=present"));
    }

    #[test]
    fn test_proposed_event_ordering() {
        let a = ProposedEvent::new("evt_proposed:QueryGraph:0", "A", BTreeMap::new(), "src");
        let b = ProposedEvent::new("evt_proposed:QueryGraph:1", "B", BTreeMap::new(), "src");
        assert!(a < b);
    }

    #[test]
    fn test_roundtrip_serialization() {
        let intent = make_sample_intent();
        let mut payload = BTreeMap::new();
        payload.insert("node_id".to_string(), "42".to_string());
        let events = vec![
            ProposedEvent::new("evt_0", "NodeSelectionEvent", payload, &intent.id),
        ];
        let seq = ProposedEventSequence::new(intent, None, events);
        let json = serde_json::to_string(&seq).unwrap();
        let parsed: ProposedEventSequence = serde_json::from_str(&json).unwrap();
        assert_eq!(seq.events.len(), parsed.events.len());
        assert_eq!(seq.events[0].event_type, parsed.events[0].event_type);
        assert_eq!(seq.explanation, parsed.explanation);
    }
}
