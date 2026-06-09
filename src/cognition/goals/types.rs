use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GoalId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GoalStatus {
    Pending,
    InProgress,
    Satisfied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GoalPredicate {
    EntityTypeExists { entity_type: String },
    RelationshipTypeExists { relationship_type: String },
    NodeCountAtLeast { count: u64 },
    EdgeCountAtLeast { count: u64 },
    QueryResultNonEmpty { minimum_results: u64 },
    AlwaysSatisfied,
    NeverSatisfied,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Goal {
    pub id: GoalId,
    pub description: String,
    pub predicate: GoalPredicate,
    pub status: GoalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GoalEvaluation {
    pub goal_id: GoalId,
    pub satisfied: bool,
    pub previous_status: GoalStatus,
    pub new_status: GoalStatus,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GoalState {
    pub goals: BTreeMap<GoalId, Goal>,
}
