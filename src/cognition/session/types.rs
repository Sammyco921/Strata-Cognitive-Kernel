use serde::{Deserialize, Serialize};

use crate::cognition::executive::types::ExecutiveDecision;
use crate::cognition::goals::types::GoalState;
use crate::cognition::memory::types::CognitiveMemoryState;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub goal_state: GoalState,
    pub memory_snapshot: CognitiveMemoryState,
    pub trace_ids: Vec<String>,
    pub executive_history: Vec<ExecutiveDecision>,
}

impl Session {
    pub fn new(session_id: &str) -> Self {
        Session {
            session_id: session_id.to_string(),
            goal_state: GoalState { goals: std::collections::BTreeMap::new() },
            memory_snapshot: CognitiveMemoryState::empty(),
            trace_ids: Vec::new(),
            executive_history: Vec::new(),
        }
    }

    pub fn with_state(
        session_id: &str,
        goal_state: GoalState,
        memory_snapshot: CognitiveMemoryState,
    ) -> Self {
        Session {
            session_id: session_id.to_string(),
            goal_state,
            memory_snapshot,
            trace_ids: Vec::new(),
            executive_history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionLog {
    sessions: std::collections::BTreeMap<String, Session>,
}

impl SessionLog {
    pub fn new() -> Self {
        SessionLog {
            sessions: std::collections::BTreeMap::new(),
        }
    }

    pub fn add_session(&mut self, session: Session) {
        self.sessions.insert(session.session_id.clone(), session);
    }

    pub fn get(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn all_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }
}

impl Default for SessionLog {
    fn default() -> Self {
        Self::new()
    }
}
