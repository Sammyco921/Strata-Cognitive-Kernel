use sha2::{Digest, Sha256};

use crate::kernel::event::Event;
use crate::kernel::graph::GraphState;

/// A deterministic SHA-256 hash of kernel graph state.
///
/// Two `StateHash` values are equal iff the corresponding `GraphState`
/// instances are semantically identical (same nodes, edges, properties).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateHash([u8; 32]);

impl StateHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Display for StateHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

/// A deterministic SHA-256 hash of an ordered event log.
///
/// Two `LogHash` values are equal iff the corresponding event sequences
/// are identical in every field (ID, timestamp, type, payload, causes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogHash([u8; 32]);

impl LogHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Display for LogHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

/// Compute the deterministic state hash of a `GraphState`.
///
/// Uses canonical JSON serialization (sorted map keys via `BTreeMap`)
/// as the pre-image for SHA-256.  The same state always produces the
/// same hash, on every platform.
pub fn state_hash(state: &GraphState) -> StateHash {
    let canonical = serde_json::to_vec(state).expect("GraphState serialization must not fail");
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    StateHash(bytes)
}

/// Compute the deterministic log hash of an ordered event sequence.
///
/// The entire event slice is serialised as a canonical JSON array before
/// hashing.  The order of events is part of the hash — reversing or
/// reordering events produces a different hash.
pub fn log_hash(events: &[Event]) -> LogHash {
    let canonical = serde_json::to_vec(events).expect("Event serialization must not fail");
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    LogHash(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::event::EventType;
    use crate::kernel::graph::GraphState;
    use serde_json::json;

    #[test]
    fn state_hash_empty() {
        let state = GraphState::empty();
        let h = state_hash(&state);
        assert_ne!(h.to_string(), "");
    }

    #[test]
    fn state_hash_deterministic() {
        let state = GraphState::empty();
        let a = state_hash(&state);
        let b = state_hash(&state);
        assert_eq!(a, b);
    }

    #[test]
    fn state_hash_different_states_differ() {
        let mut a = GraphState::empty();
        a.nodes.insert("x".into(), crate::kernel::graph::Node::new("x"));
        let b = GraphState::empty();
        assert_ne!(state_hash(&a), state_hash(&b));
    }

    #[test]
    fn state_hash_hex_length() {
        let state = GraphState::empty();
        let h = state_hash(&state);
        assert_eq!(h.to_string().len(), 64); // 32 bytes → 64 hex chars
    }

    #[test]
    fn log_hash_empty() {
        let h = log_hash(&[]);
        assert_ne!(h.to_string(), "");
    }

    #[test]
    fn log_hash_deterministic() {
        let events = vec![];
        let a = log_hash(&events);
        let b = log_hash(&events);
        assert_eq!(a, b);
    }

    #[test]
    fn log_hash_different_logs_differ() {
        let a: Vec<Event> = vec![];
        use crate::kernel::event::Event;
        let b = vec![Event::new("e1".into(), 0, EventType::CreateNode, json!({"id": "x"}))];
        assert_ne!(log_hash(&a), log_hash(&b));
    }

    #[test]
    fn log_hash_order_matters() {
        use crate::kernel::event::Event;
        let e1 = Event::new("a".into(), 0, EventType::CreateNode, json!({"id": "x"}));
        let e2 = Event::new("b".into(), 1, EventType::CreateNode, json!({"id": "y"}));
        let forward = log_hash(&[e1.clone(), e2.clone()]);
        let reverse = log_hash(&[e2, e1]);
        assert_ne!(forward, reverse);
    }

    #[test]
    fn state_hash_architecture_independent() {
        // Canonical JSON from BTreeMap + serde produces identical bytes
        // on all platforms. This test verifies the round-trip.
        let mut state = GraphState::empty();
        state.nodes.insert("n1".into(), crate::kernel::graph::Node::new("n1"));
        let json = serde_json::to_vec(&state).unwrap();
        let roundtrip: GraphState = serde_json::from_slice(&json).unwrap();
        assert_eq!(state_hash(&state), state_hash(&roundtrip));
    }
}
