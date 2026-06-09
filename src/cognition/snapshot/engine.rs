use crate::cognition::goals::types::GoalState;
use crate::cognition::memory::types::CognitiveMemoryState;
use crate::cognition::snapshot::types::Snapshot;

pub fn create_snapshot(
    snapshot_id: &str,
    event_log_pointer: u64,
    kernel_state: &crate::kernel::GraphState,
    memory_snapshot: CognitiveMemoryState,
    goal_state: GoalState,
) -> Snapshot {
    let hash = format!("{:?}", kernel_state);
    Snapshot::new(snapshot_id, event_log_pointer, &hash, memory_snapshot, goal_state)
}

pub fn restore_from_snapshot(
    snapshot: &Snapshot,
) -> (u64, String, CognitiveMemoryState, GoalState) {
    (
        snapshot.event_log_pointer,
        snapshot.kernel_state_hash.clone(),
        snapshot.memory_snapshot.clone(),
        snapshot.goal_state.clone(),
    )
}

pub fn verify_snapshot_consistency(
    snapshot: &Snapshot,
    kernel_state: &crate::kernel::GraphState,
) -> bool {
    let current_hash = format!("{:?}", kernel_state);
    current_hash == snapshot.kernel_state_hash
}

pub fn event_log_pointer_from_snapshot(snapshot: &Snapshot) -> u64 {
    snapshot.event_log_pointer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::goals::types::*;
    use crate::cognition::memory::types::CognitiveMemoryState;
    use crate::cognition::snapshot::types::SnapshotStore;
    use crate::kernel::GraphState;

    fn make_state() -> GraphState {
        let mut state = GraphState::empty();
        state.nodes.insert(1, crate::kernel::Node {
            id: 1,
            node_type: "test".to_string(),
            properties: std::collections::BTreeMap::new(),
        });
        state
    }

    #[test]
    fn test_create_snapshot() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let snap = create_snapshot("snap1", 5, &state, ms.clone(), gs.clone());
        assert_eq!(snap.snapshot_id, "snap1");
        assert_eq!(snap.event_log_pointer, 5);
        assert_eq!(snap.goal_state, gs);
        assert_eq!(snap.memory_snapshot, ms);
        assert!(snap.timestamp.is_none());
    }

    #[test]
    fn test_roundtrip_correctness() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let snap = create_snapshot("snap1", 10, &state, ms, gs);
        let (ptr, hash, _ms, _gs) = restore_from_snapshot(&snap);
        assert_eq!(ptr, 10);
        let state2 = make_state();
        let expected_hash = format!("{:?}", state2);
        assert_eq!(hash, expected_hash);
    }

    #[test]
    fn test_verify_consistency() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let snap = create_snapshot("snap1", 5, &state, ms, gs);
        assert!(verify_snapshot_consistency(&snap, &state));
    }

    #[test]
    fn test_verify_inconsistency_detected() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let snap = create_snapshot("snap1", 5, &state, ms, gs);
        let other_state = GraphState::empty();
        assert!(!verify_snapshot_consistency(&snap, &other_state));
    }

    #[test]
    fn test_no_mutation_during_creation() {
        let state = make_state();
        let before = state.clone();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let _snap = create_snapshot("snap1", 5, &state, ms, gs);
        assert_eq!(state, before);
    }

    #[test]
    fn test_identical_snapshots_from_identical_state() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let a = create_snapshot("s1", 5, &state, ms.clone(), gs.clone());
        let b = create_snapshot("s1", 5, &state, ms, gs);
        assert_eq!(a, b);
    }

    #[test]
    fn test_deterministic_snapshot_100_runs() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let first = create_snapshot("s1", 5, &state, ms.clone(), gs.clone());
        for _ in 0..100 {
            let next = create_snapshot("s1", 5, &state, ms.clone(), gs.clone());
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_event_replay_consistency_after_restore() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let snap = create_snapshot("snap1", 5, &state, ms, gs);
        let (ptr, hash, _ms, _gs) = restore_from_snapshot(&snap);
        // Restore should give us pointer to continue event log from
        assert_eq!(ptr, 5);
        let state_after = make_state();
        assert_eq!(format!("{:?}", state_after), hash);
    }

    #[test]
    fn test_snapshot_with_timestamp() {
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let snap = Snapshot::with_timestamp("s1", 5, &format!("{:?}", state), ms, gs, 12345);
        assert_eq!(snap.timestamp, Some(12345));
    }

    #[test]
    fn test_snapshot_store() {
        let mut store = SnapshotStore::new();
        let state = make_state();
        let ms = CognitiveMemoryState::empty();
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let snap = create_snapshot("s1", 5, &state, ms, gs);
        store.store(snap);
        assert_eq!(store.len(), 1);
        assert!(store.get("s1").is_some());
        assert!(store.get("s2").is_none());
    }
}
