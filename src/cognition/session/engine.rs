use super::types::*;
use crate::cognition::executive::types::ExecutiveDecision;
use crate::cognition::goals::types::GoalState;
use crate::cognition::memory::types::CognitiveMemoryState;

pub fn create_session(session_id: &str) -> Session {
    Session::new(session_id)
}

pub fn create_session_with_state(
    session_id: &str,
    goal_state: GoalState,
    memory_snapshot: CognitiveMemoryState,
) -> Session {
    Session::with_state(session_id, goal_state, memory_snapshot)
}

pub fn append_trace_to_session(mut session: Session, trace_id: String) -> Session {
    session.trace_ids.push(trace_id);
    session
}

pub fn append_executive_to_session(mut session: Session, decision: ExecutiveDecision) -> Session {
    session.executive_history.push(decision);
    session
}

pub fn reconstruct_from_traces(
    session_id: &str,
    trace_ids: Vec<String>,
    goal_state: GoalState,
    memory_snapshot: CognitiveMemoryState,
) -> Session {
    let mut session = Session::new(session_id);
    session.goal_state = goal_state;
    session.memory_snapshot = memory_snapshot;
    session.trace_ids = trace_ids;
    session
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognition::goals::types::*;
    use crate::cognition::memory::types::CognitiveMemoryState;

    #[test]
    fn test_create_session() {
        let s = create_session("test_session");
        assert_eq!(s.session_id, "test_session");
        assert!(s.trace_ids.is_empty());
        assert!(s.executive_history.is_empty());
    }

    #[test]
    fn test_create_session_with_state() {
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let ms = CognitiveMemoryState::empty();
        let s = create_session_with_state("s1", gs.clone(), ms.clone());
        assert_eq!(s.goal_state, gs);
        assert_eq!(s.memory_snapshot, ms);
    }

    #[test]
    fn test_append_trace() {
        let s = create_session("s1");
        let s = append_trace_to_session(s, "trace_1".to_string());
        assert_eq!(s.trace_ids.len(), 1);
        assert_eq!(s.trace_ids[0], "trace_1");
    }

    #[test]
    fn test_append_executive() {
        use crate::cognition::executive::types::ExecutiveDecision;
        let s = create_session("s1");
        let dec = ExecutiveDecision {
            selected_goal: None,
            ranked_goals: Vec::new(),
            continue_goals: Vec::new(),
            deferred_goals: Vec::new(),
            completed_goals: Vec::new(),
            failed_goals: Vec::new(),
            explanation: "test".to_string(),
        };
        let s = append_executive_to_session(s, dec.clone());
        assert_eq!(s.executive_history.len(), 1);
        assert_eq!(s.executive_history[0], dec);
    }

    #[test]
    fn test_reconstruct_from_traces() {
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let ms = CognitiveMemoryState::empty();
        let trace_ids = vec!["t1".to_string(), "t2".to_string()];
        let s = reconstruct_from_traces("reconstructed", trace_ids.clone(), gs.clone(), ms.clone());
        assert_eq!(s.session_id, "reconstructed");
        assert_eq!(s.trace_ids, trace_ids);
        assert_eq!(s.goal_state, gs);
        assert_eq!(s.memory_snapshot, ms);
    }

    #[test]
    fn test_deterministic_session_across_runs() {
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let ms = CognitiveMemoryState::empty();
        let first = create_session_with_state("s1", gs.clone(), ms.clone());
        for _ in 0..100 {
            let next = create_session_with_state("s1", gs.clone(), ms.clone());
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_no_mutation_of_kernel_state() {
        let s = create_session("s1");
        // Session is a container; it does not reference or mutate kernel state
        assert!(s.trace_ids.is_empty());
        let s2 = append_trace_to_session(s.clone(), "t1".to_string());
        // Original session is unchanged
        assert!(s.trace_ids.is_empty());
        assert_eq!(s2.trace_ids.len(), 1);
    }

    #[test]
    fn test_session_log() {
        let mut log = SessionLog::new();
        assert!(log.is_empty());
        log.add_session(Session::new("s1"));
        log.add_session(Session::new("s2"));
        assert_eq!(log.len(), 2);
        assert!(log.get("s1").is_some());
        assert!(log.get("s3").is_none());
    }

    #[test]
    fn test_session_log_ordering() {
        let mut log = SessionLog::new();
        log.add_session(Session::new("z"));
        log.add_session(Session::new("a"));
        let all = log.all_sessions();
        assert_eq!(all[0].session_id, "a");
        assert_eq!(all[1].session_id, "z");
    }

    #[test]
    fn test_identical_trace_inputs_identical_session() {
        let gs = GoalState { goals: std::collections::BTreeMap::new() };
        let ms = CognitiveMemoryState::empty();
        let tids = vec!["t1".to_string(), "t2".to_string()];
        let a = reconstruct_from_traces("s1", tids.clone(), gs.clone(), ms.clone());
        let b = reconstruct_from_traces("s1", tids, gs, ms);
        assert_eq!(a, b);
    }
}
