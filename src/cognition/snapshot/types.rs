use serde::{Deserialize, Serialize};

use crate::cognition::goals::types::GoalState;
use crate::cognition::memory::types::CognitiveMemoryState;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Snapshot {
    pub snapshot_id: String,
    pub event_log_pointer: u64,
    pub kernel_state_hash: String,
    pub memory_snapshot: CognitiveMemoryState,
    pub goal_state: GoalState,
    pub timestamp: Option<u64>,
}

impl Snapshot {
    pub fn new(
        snapshot_id: &str,
        event_log_pointer: u64,
        kernel_state_hash: &str,
        memory_snapshot: CognitiveMemoryState,
        goal_state: GoalState,
    ) -> Self {
        Snapshot {
            snapshot_id: snapshot_id.to_string(),
            event_log_pointer,
            kernel_state_hash: kernel_state_hash.to_string(),
            memory_snapshot,
            goal_state,
            timestamp: None,
        }
    }

    pub fn with_timestamp(
        snapshot_id: &str,
        event_log_pointer: u64,
        kernel_state_hash: &str,
        memory_snapshot: CognitiveMemoryState,
        goal_state: GoalState,
        timestamp: u64,
    ) -> Self {
        Snapshot {
            snapshot_id: snapshot_id.to_string(),
            event_log_pointer,
            kernel_state_hash: kernel_state_hash.to_string(),
            memory_snapshot,
            goal_state,
            timestamp: Some(timestamp),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotStore {
    snapshots: std::collections::BTreeMap<String, Snapshot>,
}

impl SnapshotStore {
    pub fn new() -> Self {
        SnapshotStore {
            snapshots: std::collections::BTreeMap::new(),
        }
    }

    pub fn store(&mut self, snapshot: Snapshot) {
        self.snapshots.insert(snapshot.snapshot_id.clone(), snapshot);
    }

    pub fn get(&self, snapshot_id: &str) -> Option<&Snapshot> {
        self.snapshots.get(snapshot_id)
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn all_snapshots(&self) -> Vec<&Snapshot> {
        self.snapshots.values().collect()
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}
