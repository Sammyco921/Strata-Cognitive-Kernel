use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::cognition::goals::types::{GoalId, GoalState, GoalStatus};
use crate::cognition::memory::types::CognitiveMemoryState;
use crate::cognition::system::policy::types::PolicyDecision;

// ── ExecutivePriority ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutivePriority {
    pub goal_id: GoalId,
    pub priority_score: f64,
    pub memory_weight: f64,
    pub policy_weight: f64,
    pub age_weight: u64,
    pub status: GoalStatus,
}

impl PartialEq for ExecutivePriority {
    fn eq(&self, other: &Self) -> bool {
        self.goal_id == other.goal_id
            && self.priority_score.to_bits() == other.priority_score.to_bits()
            && self.memory_weight.to_bits() == other.memory_weight.to_bits()
            && self.policy_weight.to_bits() == other.policy_weight.to_bits()
            && self.age_weight == other.age_weight
            && self.status == other.status
    }
}

impl Eq for ExecutivePriority {}

impl PartialOrd for ExecutivePriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExecutivePriority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority_score
            .to_bits()
            .cmp(&other.priority_score.to_bits())
            .then_with(|| self.goal_id.cmp(&other.goal_id))
    }
}

// ── ExecutiveDecision ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutiveDecision {
    pub selected_goal: Option<GoalId>,
    pub ranked_goals: Vec<ExecutivePriority>,
    pub continue_goals: Vec<GoalId>,
    pub deferred_goals: Vec<GoalId>,
    pub completed_goals: Vec<GoalId>,
    pub failed_goals: Vec<GoalId>,
    pub explanation: String,
}

impl PartialEq for ExecutiveDecision {
    fn eq(&self, other: &Self) -> bool {
        self.selected_goal == other.selected_goal
            && self.ranked_goals == other.ranked_goals
            && self.continue_goals == other.continue_goals
            && self.deferred_goals == other.deferred_goals
            && self.completed_goals == other.completed_goals
            && self.failed_goals == other.failed_goals
            && self.explanation == other.explanation
    }
}

impl Eq for ExecutiveDecision {}

impl PartialOrd for ExecutiveDecision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExecutiveDecision {
    fn cmp(&self, other: &Self) -> Ordering {
        self.explanation.cmp(&other.explanation)
    }
}

// ── ExecutiveContext ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutiveContext {
    pub goals: GoalState,
    pub memory_state: CognitiveMemoryState,
    pub policy_decision: PolicyDecision,
}

impl PartialEq for ExecutiveContext {
    fn eq(&self, other: &Self) -> bool {
        self.goals == other.goals
            && self.memory_state == other.memory_state
            && self.policy_decision == other.policy_decision
    }
}

impl Eq for ExecutiveContext {}

impl PartialOrd for ExecutiveContext {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExecutiveContext {
    fn cmp(&self, other: &Self) -> Ordering {
        self.goals
            .cmp(&other.goals)
            .then_with(|| self.policy_decision.cmp(&other.policy_decision))
    }
}
