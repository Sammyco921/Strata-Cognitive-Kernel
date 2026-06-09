use super::types::*;
use crate::cognition::semantic_interpreter::types::SemanticQuery;
use crate::kernel::GraphState;
use crate::ontology::OntologyRegistry;

pub fn evaluate_goal(
    goal: &Goal,
    graph: &GraphState,
    ontology: &OntologyRegistry,
    query_result: Option<&SemanticQuery>,
) -> GoalEvaluation {
    let satisfied = match &goal.predicate {
        GoalPredicate::EntityTypeExists { entity_type } => {
            ontology.entity_types.contains_key(entity_type)
        }
        GoalPredicate::RelationshipTypeExists { relationship_type } => {
            ontology.relationship_types.contains_key(relationship_type)
        }
        GoalPredicate::NodeCountAtLeast { count } => {
            (graph.nodes.len() as u64) >= *count
        }
        GoalPredicate::EdgeCountAtLeast { count } => {
            (graph.edges.len() as u64) >= *count
        }
        GoalPredicate::QueryResultNonEmpty { minimum_results } => {
            match query_result {
                Some(q) => {
                    let total = q.nodes.as_ref().map_or(0, |v| v.len())
                        + q.edges.as_ref().map_or(0, |v| v.len());
                    (total as u64) >= *minimum_results
                }
                None => false,
            }
        }
        GoalPredicate::AlwaysSatisfied => true,
        GoalPredicate::NeverSatisfied => false,
    };

    let new_status = match (&goal.status, satisfied) {
        (GoalStatus::Pending, true) => GoalStatus::Satisfied,
        (GoalStatus::Pending, false) => GoalStatus::InProgress,
        (GoalStatus::InProgress, true) => GoalStatus::Satisfied,
        (GoalStatus::InProgress, false) => GoalStatus::Failed,
        (GoalStatus::Satisfied, _) => GoalStatus::Satisfied,
        (GoalStatus::Failed, _) => GoalStatus::Failed,
    };

    let explanation = format!(
        "goal={};previous={:?};new={:?};satisfied={}",
        goal.id.0, goal.status, new_status, satisfied
    );

    GoalEvaluation {
        goal_id: goal.id.clone(),
        satisfied,
        previous_status: goal.status.clone(),
        new_status,
        explanation,
    }
}

pub fn evaluate_all_goals(
    state: &GoalState,
    graph: &GraphState,
    ontology: &OntologyRegistry,
    query_result: Option<&SemanticQuery>,
) -> Vec<GoalEvaluation> {
    let mut evaluations: Vec<GoalEvaluation> = state
        .goals
        .values()
        .map(|goal| evaluate_goal(goal, graph, ontology, query_result))
        .collect();
    evaluations.sort_by(|a, b| a.goal_id.cmp(&b.goal_id));
    evaluations
}

pub fn update_goal_state(
    state: &GoalState,
    evaluations: &[GoalEvaluation],
) -> GoalState {
    let mut goals = state.goals.clone();
    for eval in evaluations {
        if let Some(goal) = goals.get_mut(&eval.goal_id) {
            goal.status = eval.new_status.clone();
        }
    }
    GoalState { goals }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_graph(nodes: u64, edges: u64) -> GraphState {
        let mut g = GraphState::empty();
        for i in 0..nodes {
            g.nodes.insert(i, crate::kernel::Node {
                id: i,
                node_type: "test".to_string(),
                properties: BTreeMap::new(),
            });
        }
        for i in 0..edges {
            g.edges.insert(i, crate::kernel::Edge {
                id: i,
                from_node: 0,
                to_node: 1,
                edge_type: "test_edge".to_string(),
                properties: BTreeMap::new(),
            });
        }
        g
    }

    fn ontology_with_entity(name: &str) -> OntologyRegistry {
        let mut o = OntologyRegistry::empty();
        o.entity_types.insert(name.to_string(), crate::ontology::EntityType {
            name: name.to_string(),
            description: None,
        });
        o
    }

    fn ontology_with_relationship(name: &str) -> OntologyRegistry {
        let mut o = OntologyRegistry::empty();
        o.relationship_types.insert(name.to_string(), crate::ontology::RelationshipType {
            name: name.to_string(),
            from_entity: String::new(),
            to_entity: String::new(),
            description: None,
        });
        o
    }

    fn pending_goal(id: &str, predicate: GoalPredicate) -> Goal {
        Goal {
            id: GoalId(id.to_string()),
            description: String::new(),
            predicate,
            status: GoalStatus::Pending,
        }
    }

    // ── EntityTypeExists ────────────────────────────────────────────────

    #[test]
    fn test_entity_type_exists_success() {
        let goal = pending_goal("g1", GoalPredicate::EntityTypeExists { entity_type: "Person".to_string() });
        let eval = evaluate_goal(&goal, &GraphState::empty(), &ontology_with_entity("Person"), None);
        assert!(eval.satisfied);
    }

    #[test]
    fn test_entity_type_exists_failure() {
        let goal = pending_goal("g1", GoalPredicate::EntityTypeExists { entity_type: "Person".to_string() });
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
    }

    // ── RelationshipTypeExists ──────────────────────────────────────────

    #[test]
    fn test_relationship_type_exists_success() {
        let goal = pending_goal("g1", GoalPredicate::RelationshipTypeExists { relationship_type: "Knows".to_string() });
        let eval = evaluate_goal(&goal, &GraphState::empty(), &ontology_with_relationship("Knows"), None);
        assert!(eval.satisfied);
    }

    #[test]
    fn test_relationship_type_exists_failure() {
        let goal = pending_goal("g1", GoalPredicate::RelationshipTypeExists { relationship_type: "Knows".to_string() });
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
    }

    // ── NodeCountAtLeast ────────────────────────────────────────────────

    #[test]
    fn test_node_count_at_least_success() {
        let goal = pending_goal("g1", GoalPredicate::NodeCountAtLeast { count: 3 });
        let g = make_graph(5, 0);
        let eval = evaluate_goal(&goal, &g, &OntologyRegistry::empty(), None);
        assert!(eval.satisfied);
    }

    #[test]
    fn test_node_count_at_least_failure() {
        let goal = pending_goal("g1", GoalPredicate::NodeCountAtLeast { count: 10 });
        let g = make_graph(5, 0);
        let eval = evaluate_goal(&goal, &g, &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
    }

    // ── EdgeCountAtLeast ────────────────────────────────────────────────

    #[test]
    fn test_edge_count_at_least_success() {
        let goal = pending_goal("g1", GoalPredicate::EdgeCountAtLeast { count: 2 });
        let g = make_graph(0, 5);
        let eval = evaluate_goal(&goal, &g, &OntologyRegistry::empty(), None);
        assert!(eval.satisfied);
    }

    #[test]
    fn test_edge_count_at_least_failure() {
        let goal = pending_goal("g1", GoalPredicate::EdgeCountAtLeast { count: 10 });
        let g = make_graph(0, 5);
        let eval = evaluate_goal(&goal, &g, &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
    }

    // ── QueryResultNonEmpty ─────────────────────────────────────────────

    #[test]
    fn test_query_result_non_empty_success() {
        let goal = pending_goal("g1", GoalPredicate::QueryResultNonEmpty { minimum_results: 2 });
        let query = SemanticQuery {
            nodes: Some(vec![1, 2, 3]),
            edges: Some(vec![1]),
            node_filters: BTreeMap::new(),
            edge_filters: BTreeMap::new(),
            depth: None,
        };
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), Some(&query));
        assert!(eval.satisfied);
    }

    #[test]
    fn test_query_result_non_empty_failure() {
        let goal = pending_goal("g1", GoalPredicate::QueryResultNonEmpty { minimum_results: 10 });
        let query = SemanticQuery {
            nodes: Some(vec![1, 2, 3]),
            edges: Some(vec![1]),
            node_filters: BTreeMap::new(),
            edge_filters: BTreeMap::new(),
            depth: None,
        };
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), Some(&query));
        assert!(!eval.satisfied);
    }

    #[test]
    fn test_query_result_non_empty_no_query() {
        let goal = pending_goal("g1", GoalPredicate::QueryResultNonEmpty { minimum_results: 0 });
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
    }

    // ── AlwaysSatisfied / NeverSatisfied ────────────────────────────────

    #[test]
    fn test_always_satisfied() {
        let goal = pending_goal("g1", GoalPredicate::AlwaysSatisfied);
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(eval.satisfied);
    }

    #[test]
    fn test_never_satisfied() {
        let goal = pending_goal("g1", GoalPredicate::NeverSatisfied);
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
    }

    // ── State transitions ──────────────────────────────────────────────

    #[test]
    fn test_pending_to_in_progress() {
        let goal = pending_goal("g1", GoalPredicate::NeverSatisfied);
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
        assert_eq!(eval.previous_status, GoalStatus::Pending);
        assert_eq!(eval.new_status, GoalStatus::InProgress);
    }

    #[test]
    fn test_pending_to_satisfied() {
        let goal = pending_goal("g1", GoalPredicate::AlwaysSatisfied);
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(eval.satisfied);
        assert_eq!(eval.previous_status, GoalStatus::Pending);
        assert_eq!(eval.new_status, GoalStatus::Satisfied);
    }

    #[test]
    fn test_in_progress_to_satisfied() {
        let goal = Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::InProgress,
        };
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(eval.satisfied);
        assert_eq!(eval.previous_status, GoalStatus::InProgress);
        assert_eq!(eval.new_status, GoalStatus::Satisfied);
    }

    #[test]
    fn test_in_progress_to_failed() {
        let goal = Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::NeverSatisfied,
            status: GoalStatus::InProgress,
        };
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(!eval.satisfied);
        assert_eq!(eval.previous_status, GoalStatus::InProgress);
        assert_eq!(eval.new_status, GoalStatus::Failed);
    }

    #[test]
    fn test_satisfied_remains_satisfied() {
        let goal = Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::NeverSatisfied,
            status: GoalStatus::Satisfied,
        };
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert_eq!(eval.new_status, GoalStatus::Satisfied);
    }

    #[test]
    fn test_failed_remains_failed() {
        let goal = Goal {
            id: GoalId("g1".to_string()),
            description: String::new(),
            predicate: GoalPredicate::AlwaysSatisfied,
            status: GoalStatus::Failed,
        };
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert_eq!(eval.new_status, GoalStatus::Failed);
    }

    // ── Evaluate all goals ──────────────────────────────────────────────

    #[test]
    fn test_evaluate_all_goals_empty() {
        let state = GoalState { goals: BTreeMap::new() };
        let evals = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(evals.is_empty());
    }

    #[test]
    fn test_evaluate_all_goals_multiple() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("b".to_string()), pending_goal("b", GoalPredicate::AlwaysSatisfied));
        goals.insert(GoalId("a".to_string()), pending_goal("a", GoalPredicate::NeverSatisfied));
        let state = GoalState { goals };
        let evals = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert_eq!(evals.len(), 2);
        assert_eq!(evals[0].goal_id, GoalId("a".to_string()));
        assert_eq!(evals[1].goal_id, GoalId("b".to_string()));
        assert!(!evals[0].satisfied);
        assert!(evals[1].satisfied);
    }

    #[test]
    fn test_deterministic_identical_evaluations() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("x".to_string()), pending_goal("x", GoalPredicate::AlwaysSatisfied));
        goals.insert(GoalId("y".to_string()), pending_goal("y", GoalPredicate::NeverSatisfied));
        let state = GoalState { goals };
        let a = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        let b = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert_eq!(a, b);
    }

    #[test]
    fn test_100_run_stability() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied));
        goals.insert(GoalId("g2".to_string()), pending_goal("g2", GoalPredicate::NeverSatisfied));
        goals.insert(GoalId("g3".to_string()), pending_goal("g3", GoalPredicate::NodeCountAtLeast { count: 5 }));
        let state = GoalState { goals };
        let g = make_graph(10, 0);
        let first = evaluate_all_goals(&state, &g, &OntologyRegistry::empty(), None);
        for _ in 0..100 {
            let next = evaluate_all_goals(&state, &g, &OntologyRegistry::empty(), None);
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_ordering_stability() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("z".to_string()), pending_goal("z", GoalPredicate::AlwaysSatisfied));
        goals.insert(GoalId("a".to_string()), pending_goal("a", GoalPredicate::AlwaysSatisfied));
        goals.insert(GoalId("m".to_string()), pending_goal("m", GoalPredicate::AlwaysSatisfied));
        let state = GoalState { goals };
        let evals = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert_eq!(evals[0].goal_id.0, "a");
        assert_eq!(evals[1].goal_id.0, "m");
        assert_eq!(evals[2].goal_id.0, "z");
    }

    // ── GoalState serialization ─────────────────────────────────────────

    #[test]
    fn test_goal_state_serialization_roundtrip() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), Goal {
            id: GoalId("g1".to_string()),
            description: "test goal".to_string(),
            predicate: GoalPredicate::NodeCountAtLeast { count: 3 },
            status: GoalStatus::Pending,
        });
        let state = GoalState { goals };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: GoalState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn test_goal_evaluation_serialization_roundtrip() {
        let eval = GoalEvaluation {
            goal_id: GoalId("g1".to_string()),
            satisfied: true,
            previous_status: GoalStatus::Pending,
            new_status: GoalStatus::Satisfied,
            explanation: "goal=g1;previous=Pending;new=Satisfied;satisfied=true".to_string(),
        };
        let json = serde_json::to_string(&eval).unwrap();
        let parsed: GoalEvaluation = serde_json::from_str(&json).unwrap();
        assert_eq!(eval, parsed);
    }

    #[test]
    fn test_goal_state_serialization_deterministic() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied));
        let state = GoalState { goals };
        let first = serde_json::to_string(&state).unwrap();
        for _ in 0..100 {
            let next = serde_json::to_string(&state).unwrap();
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_goal_evaluation_serialization_deterministic() {
        let eval = GoalEvaluation {
            goal_id: GoalId("g1".to_string()),
            satisfied: true,
            previous_status: GoalStatus::Pending,
            new_status: GoalStatus::Satisfied,
            explanation: "test".to_string(),
        };
        let first = serde_json::to_string(&eval).unwrap();
        for _ in 0..100 {
            let next = serde_json::to_string(&eval).unwrap();
            assert_eq!(first, next);
        }
    }

    // ── Update goal state ───────────────────────────────────────────────

    #[test]
    fn test_update_goal_state() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied));
        goals.insert(GoalId("g2".to_string()), pending_goal("g2", GoalPredicate::NeverSatisfied));
        let state = GoalState { goals };
        let evals = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        let updated = update_goal_state(&state, &evals);
        assert_eq!(updated.goals[&GoalId("g1".to_string())].status, GoalStatus::Satisfied);
        assert_eq!(updated.goals[&GoalId("g2".to_string())].status, GoalStatus::InProgress);
    }

    #[test]
    fn test_update_goal_state_no_mutation() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied));
        let original = GoalState { goals: goals.clone() };
        let state = GoalState { goals };
        let evals = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        let _updated = update_goal_state(&state, &evals);
        assert_eq!(original.goals[&GoalId("g1".to_string())].status, GoalStatus::Pending);
    }

    #[test]
    fn test_update_goal_state_returns_new_state() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied));
        let state = GoalState { goals };
        let evals = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        let updated = update_goal_state(&state, &evals);
        assert_ne!(
            state.goals[&GoalId("g1".to_string())].status,
            updated.goals[&GoalId("g1".to_string())].status,
        );
    }

    // ── Duplicate goal ID rejection ─────────────────────────────────────

    #[test]
    fn test_duplicate_goal_id_rejected() {
        let mut goals = BTreeMap::new();
        assert!(goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied)).is_none());
        assert!(goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::NeverSatisfied)).is_some());
    }

    // ── Large goal registry ─────────────────────────────────────────────

    #[test]
    fn test_large_goal_registry() {
        let mut goals = BTreeMap::new();
        for i in 0..1000 {
            let id = format!("goal_{}", i);
            goals.insert(GoalId(id.clone()), pending_goal(&id, GoalPredicate::AlwaysSatisfied));
        }
        let state = GoalState { goals };
        let evals = evaluate_all_goals(&state, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert_eq!(evals.len(), 1000);
        for e in &evals {
            assert!(e.satisfied);
        }
    }

    // ── Evaluate goal does not mutate inputs ────────────────────────────

    #[test]
    fn test_evaluate_goal_does_not_mutate_input() {
        let goal = pending_goal("g1", GoalPredicate::AlwaysSatisfied);
        let g = GraphState::empty();
        let o = OntologyRegistry::empty();
        let before = goal.clone();
        let _eval = evaluate_goal(&goal, &g, &o, None);
        assert_eq!(goal, before);
    }

    #[test]
    fn test_evaluate_all_goals_does_not_mutate_input() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied));
        let state = GoalState { goals };
        let g = GraphState::empty();
        let o = OntologyRegistry::empty();
        let before = state.clone();
        let _evals = evaluate_all_goals(&state, &g, &o, None);
        assert_eq!(state, before);
    }

    // ── Explanation format ──────────────────────────────────────────────

    #[test]
    fn test_explanation_format() {
        let goal = pending_goal("g1", GoalPredicate::AlwaysSatisfied);
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(eval.explanation.contains("goal=g1"));
        assert!(eval.explanation.contains("previous=Pending"));
        assert!(eval.explanation.contains("new=Satisfied"));
        assert!(eval.explanation.contains("satisfied=true"));
    }

    // ── Integration-style tests within the module ───────────────────────

    #[test]
    fn test_multiple_goals_evaluated_correctly() {
        let mut goals = BTreeMap::new();
        goals.insert(GoalId("g1".to_string()), pending_goal("g1", GoalPredicate::AlwaysSatisfied));
        goals.insert(GoalId("g2".to_string()), pending_goal("g2", GoalPredicate::NodeCountAtLeast { count: 3 }));
        goals.insert(GoalId("g3".to_string()), pending_goal("g3", GoalPredicate::NeverSatisfied));
        let state = GoalState { goals };
        let g = make_graph(5, 0);
        let evals = evaluate_all_goals(&state, &g, &OntologyRegistry::empty(), None);
        assert!(evals.iter().find(|e| e.goal_id == GoalId("g1".to_string())).unwrap().satisfied);
        assert!(evals.iter().find(|e| e.goal_id == GoalId("g2".to_string())).unwrap().satisfied);
        assert!(!evals.iter().find(|e| e.goal_id == GoalId("g3".to_string())).unwrap().satisfied);
    }

    #[test]
    fn test_goal_count_for_empty_registry() {
        let g = GraphState::empty();
        let o = OntologyRegistry::empty();
        let goal = pending_goal("g1", GoalPredicate::NodeCountAtLeast { count: 0 });
        let eval = evaluate_goal(&goal, &g, &o, None);
        assert!(eval.satisfied);
    }

    #[test]
    fn test_edge_count_zero() {
        let goal = pending_goal("g1", GoalPredicate::EdgeCountAtLeast { count: 0 });
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), None);
        assert!(eval.satisfied);
    }

    #[test]
    fn test_query_result_zero_minimum() {
        let query = SemanticQuery {
            nodes: Some(vec![]),
            edges: None,
            node_filters: BTreeMap::new(),
            edge_filters: BTreeMap::new(),
            depth: None,
        };
        let goal = pending_goal("g1", GoalPredicate::QueryResultNonEmpty { minimum_results: 0 });
        let eval = evaluate_goal(&goal, &GraphState::empty(), &OntologyRegistry::empty(), Some(&query));
        assert!(eval.satisfied);
    }
}
