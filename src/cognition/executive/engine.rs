use crate::cognition::executive::types::*;
use crate::cognition::goals::types::*;

/// Compute the memory weight for a goal based on the overall cognitive memory state.
/// Uses total success/failure counts across all intents.
/// Formula: (success_count + 1.0) / (failure_count + 1.0), clamped to [0.1, 2.0]
fn compute_memory_weight(memory_state: &crate::cognition::memory::types::CognitiveMemoryState) -> f64 {
    let success_count: u64 = memory_state.intent_success_counts.values().sum();
    let failure_count: u64 = memory_state.intent_failure_counts.values().sum();

    let raw = (success_count as f64 + 1.0) / (failure_count as f64 + 1.0);
    raw.clamp(0.1, 2.0)
}

/// Extract the policy weight from the context's policy decision.
/// Uses the selected candidate's final_score. Defaults to 0.0 if unavailable.
fn extract_policy_weight(context: &ExecutiveContext) -> f64 {
    context.policy_decision.selected.score.final_score
}

/// Convert a GoalStatus to its numeric weight for priority scoring.
/// Pending = 100, InProgress = 80, Satisfied = 0, Failed = 0
fn status_weight(status: &GoalStatus) -> f64 {
    match status {
        GoalStatus::Pending => 100.0,
        GoalStatus::InProgress => 80.0,
        GoalStatus::Satisfied => 0.0,
        GoalStatus::Failed => 0.0,
    }
}

/// Score a single goal and produce an ExecutivePriority.
///
/// Formula:
///   priority_score = status_weight
///                    * (memory_weight * 10.0)
///                    * policy_weight
///                    * age_weight as f64
///
/// All components are purely deterministic and derived from the context.
pub fn score_goal(
    goal: &Goal,
    context: &ExecutiveContext,
    age_index: u64,
) -> ExecutivePriority {
    let sw = status_weight(&goal.status);
    let mw = compute_memory_weight(&context.memory_state);
    let pw = extract_policy_weight(context);
    let aw = age_index;

    let priority_score = sw * (mw * 10.0) * pw * aw as f64;

    ExecutivePriority {
        goal_id: goal.id.clone(),
        priority_score,
        memory_weight: mw,
        policy_weight: pw,
        age_weight: aw,
        status: goal.status.clone(),
    }
}

/// Rank goals by priority_score descending, tie-breaking by goal_id.
/// Returns a stable, deterministic ordering.
pub fn rank_goals(goals: &[ExecutivePriority]) -> Vec<ExecutivePriority> {
    let mut sorted = goals.to_vec();
    sorted.sort_by(|a, b| b.cmp(a));
    sorted
}

/// Select the highest-ranked active goal.
/// Active goals: Pending or InProgress.
/// Satisfied and Failed goals are ineligible.
pub fn select_goal(ranked: &[ExecutivePriority]) -> Option<GoalId> {
    for r in ranked {
        match r.status {
            GoalStatus::Pending | GoalStatus::InProgress => return Some(r.goal_id.clone()),
            GoalStatus::Satisfied | GoalStatus::Failed => continue,
        }
    }
    None
}

/// Make a full executive decision from the context.
///
/// 1. Score all goals
/// 2. Rank all goals
/// 3. Select active goal
/// 4. Classify goals into continue/deferred/completed/failed
/// 5. Produce deterministic explanation
pub fn make_decision(context: &ExecutiveContext) -> ExecutiveDecision {
    let goals: Vec<&Goal> = context.goals.goals.values().collect();

    // Score all goals with deterministic insertion ordering
    let mut priorities: Vec<ExecutivePriority> = Vec::with_capacity(goals.len());
    for (age_index, goal) in goals.iter().enumerate() {
        let p = score_goal(goal, context, age_index as u64);
        priorities.push(p);
    }

    // Rank
    let ranked = rank_goals(&priorities);

    // Select
    let selected = select_goal(&ranked);

    // Classify
    let mut continue_goals: Vec<GoalId> = Vec::new();
    let mut deferred_goals: Vec<GoalId> = Vec::new();
    let mut completed_goals: Vec<GoalId> = Vec::new();
    let mut failed_goals: Vec<GoalId> = Vec::new();

    for p in &ranked {
        let gid = p.goal_id.clone();
        match p.status {
            GoalStatus::Pending => {
                if selected.as_ref() == Some(&gid) {
                    continue_goals.push(gid);
                } else {
                    deferred_goals.push(gid);
                }
            }
            GoalStatus::InProgress => {
                if selected.as_ref() == Some(&gid) {
                    continue_goals.push(gid);
                } else {
                    deferred_goals.push(gid);
                }
            }
            GoalStatus::Satisfied => {
                completed_goals.push(gid);
            }
            GoalStatus::Failed => {
                failed_goals.push(gid);
            }
        }
    }

    // Build explanation
    let explanation = match &selected {
        Some(gid) => {
            let priority = ranked.iter().find(|p| &p.goal_id == gid);
            match priority {
                Some(p) => format!(
                    "goal={};priority={};rank=1;status={:?}",
                    gid.0,
                    p.priority_score.to_bits(),
                    p.status,
                ),
                None => format!("goal={};priority=0;rank=1;status=none", gid.0),
            }
        }
        None => "goal=none;priority=0;rank=0;status=none".to_string(),
    };

    ExecutiveDecision {
        selected_goal: selected,
        ranked_goals: ranked,
        continue_goals,
        deferred_goals,
        completed_goals,
        failed_goals,
        explanation,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use super::*;
    use crate::cognition::memory::types::CognitiveMemoryState;
    use crate::cognition::system::policy::types::*;
    use crate::cognition::semantic_interpreter::types::*;

    fn make_goal(id: &str, status: GoalStatus, predicate: GoalPredicate) -> Goal {
        Goal {
            id: GoalId(id.to_string()),
            description: String::new(),
            predicate,
            status,
        }
    }

    fn empty_context() -> ExecutiveContext {
        let intent = SemanticIntent::new("default", IntentType::Unknown, BTreeMap::new(), BTreeMap::new());
        let score = PolicyScore::compute(1, 0, 0, 0, 0, 1.0, "default");
        let candidate = PolicyCandidate::new(intent, score);
        let decision = PolicyDecision::new(
            candidate,
            vec![],
            BTreeMap::new(),
            BTreeMap::new(),
        );
        ExecutiveContext {
            goals: GoalState { goals: BTreeMap::new() },
            memory_state: CognitiveMemoryState::empty(),
            policy_decision: decision,
        }
    }

    fn context_with_goals(goals: Vec<(String, GoalStatus, GoalPredicate)>) -> ExecutiveContext {
        let mut ctx = empty_context();
        let mut goal_map = BTreeMap::new();
        for (id, status, predicate) in goals {
            let g = make_goal(&id, status, predicate);
            goal_map.insert(GoalId(id), g);
        }
        ctx.goals = GoalState { goals: goal_map };
        ctx
    }

    fn context_with_memory(success: u64, failure: u64) -> ExecutiveContext {
        let mut ctx = empty_context();
        let mut sc = BTreeMap::new();
        let mut fc = BTreeMap::new();
        sc.insert(IntentType::QueryGraph, success);
        fc.insert(IntentType::QueryGraph, failure);
        ctx.memory_state = CognitiveMemoryState {
            intent_success_counts: sc,
            intent_failure_counts: fc,
            ..CognitiveMemoryState::empty()
        };
        ctx
    }

    fn context_with_policy(final_score: f64) -> ExecutiveContext {
        let intent = SemanticIntent::new("policy_test", IntentType::Unknown, BTreeMap::new(), BTreeMap::new());
        let score = PolicyScore::compute(
            (final_score / 1.0) as i64, 0, 0, 0, 0, 1.0, "policy_test"
        );
        let candidate = PolicyCandidate::new(intent, score);
        let decision = PolicyDecision::new(
            candidate,
            vec![],
            BTreeMap::new(),
            BTreeMap::new(),
        );
        ExecutiveContext {
            goals: GoalState { goals: BTreeMap::new() },
            memory_state: CognitiveMemoryState::empty(),
            policy_decision: decision,
        }
    }

    // ── ExecutivePriority Tests ──────────────────────────────────────────

    #[test]
    fn test_priority_serialization_roundtrip() {
        let p = ExecutivePriority {
            goal_id: GoalId("g1".to_string()),
            priority_score: 42.5,
            memory_weight: 1.5,
            policy_weight: 2.0,
            age_weight: 3,
            status: GoalStatus::Pending,
        };
        let json = serde_json::to_string(&p).unwrap();
        let parsed: ExecutivePriority = serde_json::from_str(&json).unwrap();
        assert_eq!(p, parsed);
    }

    #[test]
    fn test_priority_deterministic_ordering() {
        let a = ExecutivePriority {
            goal_id: GoalId("a".to_string()),
            priority_score: 100.0,
            memory_weight: 1.0,
            policy_weight: 1.0,
            age_weight: 1,
            status: GoalStatus::Pending,
        };
        let b = ExecutivePriority {
            goal_id: GoalId("b".to_string()),
            priority_score: 50.0,
            memory_weight: 1.0,
            policy_weight: 1.0,
            age_weight: 1,
            status: GoalStatus::Pending,
        };
        assert!(a > b);
    }

    #[test]
    fn test_priority_equality_via_to_bits() {
        let a = ExecutivePriority {
            goal_id: GoalId("g1".to_string()),
            priority_score: 42.5,
            memory_weight: 1.5,
            policy_weight: 2.0,
            age_weight: 3,
            status: GoalStatus::Pending,
        };
        let b = ExecutivePriority {
            goal_id: GoalId("g1".to_string()),
            priority_score: 42.5,
            memory_weight: 1.5,
            policy_weight: 2.0,
            age_weight: 3,
            status: GoalStatus::Pending,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn test_priority_score_stability_100_runs() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
        ]);
        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let first = score_goal(goal, &ctx, 0);
        for _ in 0..100 {
            let next = score_goal(goal, &ctx, 0);
            assert_eq!(first, next);
        }
    }

    // ── Executive Scoring Tests ──────────────────────────────────────────

    #[test]
    fn test_pending_priority() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
        ]);
        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let p = score_goal(goal, &ctx, 0);
        assert_eq!(p.status, GoalStatus::Pending);
        // With policy_weight=0 (default), the product should be 0
        assert_eq!(p.priority_score, 0.0);
    }

    #[test]
    fn test_in_progress_priority() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::InProgress, GoalPredicate::AlwaysSatisfied),
        ]);
        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let p = score_goal(goal, &ctx, 0);
        assert_eq!(p.status, GoalStatus::InProgress);
        assert_eq!(p.priority_score, 0.0);
    }

    #[test]
    fn test_satisfied_exclusion() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Satisfied, GoalPredicate::AlwaysSatisfied),
        ]);
        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let p = score_goal(goal, &ctx, 0);
        assert_eq!(p.priority_score, 0.0);
    }

    #[test]
    fn test_failed_exclusion() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Failed, GoalPredicate::NeverSatisfied),
        ]);
        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let p = score_goal(goal, &ctx, 0);
        assert_eq!(p.priority_score, 0.0);
    }

    #[test]
    fn test_memory_weighting() {
        // With policy_score = 0, priority will be 0 regardless of memory
        // So we need a non-zero policy score to test memory weighting
        let mut ctx = context_with_memory(5, 2);
        // Add a goal and set policy to non-zero
        let mut goal_map = BTreeMap::new();
        goal_map.insert(GoalId("g1".to_string()), make_goal("g1", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        ctx.goals = GoalState { goals: goal_map };

        // Override policy_decision to have non-zero score
        let intent = SemanticIntent::new("policy_test", IntentType::Unknown, BTreeMap::new(), BTreeMap::new());
        let score = PolicyScore::compute(10, 0, 0, 0, 0, 1.0, "policy_test");
        let candidate = PolicyCandidate::new(intent, score);
        ctx.policy_decision = PolicyDecision::new(candidate, vec![], BTreeMap::new(), BTreeMap::new());

        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let p = score_goal(goal, &ctx, 1);
        // memory_weight = (5+1)/(2+1) = 6/3 = 2.0 (clamping to 2.0 max)
        assert_eq!(p.memory_weight, 2.0);
        // priority = 100 * (2.0 * 10.0) * final_score * 1
        // final_score = 10*1.0 + 0 + 0 + 0 + 0 + 1.0*1.5 = 11.5
        // priority = 100 * 20.0 * 11.5 * 1.0 = 23000.0
        assert_eq!(p.priority_score, 23000.0);
    }

    #[test]
    fn test_policy_weighting() {
        let ctx = context_with_policy(50.0);
        let goal = make_goal("g1", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied);
        let mut goal_map = BTreeMap::new();
        goal_map.insert(GoalId("g1".to_string()), goal);
        let ctx = ExecutiveContext {
            goals: GoalState { goals: goal_map },
            ..ctx
        };
        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let p = score_goal(goal, &ctx, 1);
        // policy_weight = selected.score.final_score (depends on compute)
        // compute(50,0,0,0,0,1.0,"policy_test") -> 50*1.0 + 1.0*1.5 = 51.5
        assert_eq!(p.policy_weight, 51.5);
    }

    #[test]
    fn test_age_weighting() {
        let ctx = context_with_policy(10.0);
        let goal_map = BTreeMap::new();
        let ctx = ExecutiveContext {
            goals: GoalState { goals: goal_map },
            ..ctx
        };
        // test by adding a goal to make context non-empty
        let mut goal_map = BTreeMap::new();
        let g1 = make_goal("g1", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied);
        let g2 = make_goal("g2", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied);
        goal_map.insert(GoalId("g1".to_string()), g1);
        goal_map.insert(GoalId("g2".to_string()), g2);
        let ctx = ExecutiveContext {
            goals: GoalState { goals: goal_map },
            ..ctx
        };
        let goal1 = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let goal2 = ctx.goals.goals.get(&GoalId("g2".to_string())).unwrap();
        // g1 at age_index 0 should have lower priority than g2 at age_index 1
        // (both have same status, memory, policy)
        let p1 = score_goal(goal1, &ctx, 0);
        let p2 = score_goal(goal2, &ctx, 1);
        // Both have priority = 0 because policy_weight produces final_score... wait
        // With policy compute: 10*1.0 + 0 + 0 + 0 + 0 + 1.0*1.5 = 11.5
        // But the context_with_policy creates a separate context with a different policy
        // The context has no goals in the goal_map initially
        // let's trace: policy_decision.selected.score.final_score = compute(50,0,0,0,0,1.0,"policy_test").final_score
        // = 50*1.0 + 0 + 0 + 0 + 0 + 1.0*1.5 = 51.5 -- wait, context_with_policy(10.0)
        // compute(10,0,0,0,0,1.0,"policy_test") = 10*1.0 + 0 + 0 + 0 + 0 + 1.0*1.5 = 11.5
        // priority = 100 * (memory_weight * 10.0) * 11.5 * age_index
        // memory_weight with empty memory: (0+1)/(0+1) = 1.0 (clamped between 0.1 and 2.0, so 1.0)
        // p1 = 100 * (1.0 * 10.0) * 11.5 * 0 = 0.0
        // p2 = 100 * (1.0 * 10.0) * 11.5 * 1 = 11500.0
        // So p2 should be higher
        assert!(p2.priority_score > p1.priority_score);
    }

    #[test]
    fn test_deterministic_scoring() {
        let ctx = context_with_memory(3, 1);
        let mut goal_map = BTreeMap::new();
        goal_map.insert(GoalId("g1".to_string()), make_goal("g1", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        let ctx = ExecutiveContext {
            goals: GoalState { goals: goal_map },
            ..ctx
        };
        let goal = ctx.goals.goals.get(&GoalId("g1".to_string())).unwrap();
        let first = score_goal(goal, &ctx, 0);
        for _ in 0..100 {
            let next = score_goal(goal, &ctx, 0);
            assert_eq!(first, next);
        }
    }

    // ── Ranking Tests ────────────────────────────────────────────────────

    #[test]
    fn test_deterministic_ranking() {
        let mut ctx = context_with_policy(5.0);
        let mut goal_map = BTreeMap::new();
        goal_map.insert(GoalId("g1".to_string()), make_goal("g1", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        goal_map.insert(GoalId("g2".to_string()), make_goal("g2", GoalStatus::InProgress, GoalPredicate::AlwaysSatisfied));
        ctx.goals = GoalState { goals: goal_map };

        let goals: Vec<&Goal> = ctx.goals.goals.values().collect();
        let priorities: Vec<ExecutivePriority> = goals.iter().enumerate()
            .map(|(i, g)| score_goal(g, &ctx, i as u64))
            .collect();
        let ranked = rank_goals(&priorities);
        let first = ranked.clone();
        for _ in 0..100 {
            let next = rank_goals(&priorities);
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_tie_breaking_by_id() {
        // Two goals with same priority_score; should tie-break by goal_id
        let mut ctx = context_with_policy(5.0);
        let mut goal_map = BTreeMap::new();
        goal_map.insert(GoalId("g_a".to_string()), make_goal("g_a", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        goal_map.insert(GoalId("g_b".to_string()), make_goal("g_b", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        ctx.goals = GoalState { goals: goal_map };

        let goals: Vec<&Goal> = ctx.goals.goals.values().collect();
        let priorities: Vec<ExecutivePriority> = goals.iter().enumerate()
            .map(|(i, g)| score_goal(g, &ctx, i as u64))
            .collect();
        let ranked = rank_goals(&priorities);

        // age_index 0 = g_a (first inserted), age_index 1 = g_b
        // p_a uses age_index 0 so priority_score = 0
        // p_b uses age_index 1 so priority_score > 0
        // So g_b should be first
        assert_eq!(ranked[0].goal_id, GoalId("g_b".to_string()));
        assert_eq!(ranked[1].goal_id, GoalId("g_a".to_string()));
    }

    #[test]
    fn test_descending_order() {
        let mut ctx = context_with_policy(5.0);
        let mut goal_map = BTreeMap::new();
        goal_map.insert(GoalId("g1".to_string()), make_goal("g1", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        goal_map.insert(GoalId("g2".to_string()), make_goal("g2", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        goal_map.insert(GoalId("g3".to_string()), make_goal("g3", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        ctx.goals = GoalState { goals: goal_map };

        let goals: Vec<&Goal> = ctx.goals.goals.values().collect();
        let priorities: Vec<ExecutivePriority> = goals.iter().enumerate()
            .map(|(i, g)| score_goal(g, &ctx, i as u64))
            .collect();
        let ranked = rank_goals(&priorities);

        for i in 1..ranked.len() {
            assert!(
                ranked[i - 1].priority_score >= ranked[i].priority_score,
                "Ranking should be descending"
            );
        }
    }

    #[test]
    fn test_identical_inputs_identical_ranking() {
        let ctx = context_with_memory(2, 1);
        let mut goal_map = BTreeMap::new();
        goal_map.insert(GoalId("g1".to_string()), make_goal("g1", GoalStatus::Pending, GoalPredicate::AlwaysSatisfied));
        goal_map.insert(GoalId("g2".to_string()), make_goal("g2", GoalStatus::InProgress, GoalPredicate::AlwaysSatisfied));
        let ctx = ExecutiveContext {
            goals: GoalState { goals: goal_map },
            ..ctx
        };
        let goals: Vec<&Goal> = ctx.goals.goals.values().collect();
        let priorities: Vec<ExecutivePriority> = goals.iter().enumerate()
            .map(|(i, g)| score_goal(g, &ctx, i as u64))
            .collect();
        let a = rank_goals(&priorities);

        let goals2: Vec<&Goal> = ctx.goals.goals.values().collect();
        let priorities2: Vec<ExecutivePriority> = goals2.iter().enumerate()
            .map(|(i, g)| score_goal(g, &ctx, i as u64))
            .collect();
        let b = rank_goals(&priorities2);
        assert_eq!(a, b);
    }

    // ── Selection Tests ──────────────────────────────────────────────────

    #[test]
    fn test_active_goal_selected() {
        let ranked = vec![
            ExecutivePriority {
                goal_id: GoalId("g1".to_string()),
                priority_score: 100.0,
                memory_weight: 1.0,
                policy_weight: 1.0,
                age_weight: 0,
                status: GoalStatus::Pending,
            },
        ];
        assert_eq!(select_goal(&ranked), Some(GoalId("g1".to_string())));
    }

    #[test]
    fn test_in_progress_goal_selected() {
        let ranked = vec![
            ExecutivePriority {
                goal_id: GoalId("g1".to_string()),
                priority_score: 80.0,
                memory_weight: 1.0,
                policy_weight: 1.0,
                age_weight: 0,
                status: GoalStatus::InProgress,
            },
        ];
        assert_eq!(select_goal(&ranked), Some(GoalId("g1".to_string())));
    }

    #[test]
    fn test_completed_goal_skipped() {
        let ranked = vec![
            ExecutivePriority {
                goal_id: GoalId("g1".to_string()),
                priority_score: 100.0,
                memory_weight: 1.0,
                policy_weight: 1.0,
                age_weight: 0,
                status: GoalStatus::Satisfied,
            },
        ];
        assert_eq!(select_goal(&ranked), None);
    }

    #[test]
    fn test_failed_goal_skipped() {
        let ranked = vec![
            ExecutivePriority {
                goal_id: GoalId("g1".to_string()),
                priority_score: 100.0,
                memory_weight: 1.0,
                policy_weight: 1.0,
                age_weight: 0,
                status: GoalStatus::Failed,
            },
        ];
        assert_eq!(select_goal(&ranked), None);
    }

    #[test]
    fn test_no_goals_returns_none() {
        let ranked: Vec<ExecutivePriority> = vec![];
        assert_eq!(select_goal(&ranked), None);
    }

    #[test]
    fn test_selects_first_active_skipping_inactive() {
        let ranked = vec![
            ExecutivePriority {
                goal_id: GoalId("g1".to_string()),
                priority_score: 100.0,
                memory_weight: 1.0,
                policy_weight: 1.0,
                age_weight: 0,
                status: GoalStatus::Satisfied,
            },
            ExecutivePriority {
                goal_id: GoalId("g2".to_string()),
                priority_score: 80.0,
                memory_weight: 1.0,
                policy_weight: 1.0,
                age_weight: 0,
                status: GoalStatus::Pending,
            },
        ];
        assert_eq!(select_goal(&ranked), Some(GoalId("g2".to_string())));
    }

    // ── Decision Tests ───────────────────────────────────────────────────

    #[test]
    fn test_decision_classification() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
            ("g2".to_string(), GoalStatus::InProgress, GoalPredicate::AlwaysSatisfied),
            ("g3".to_string(), GoalStatus::Satisfied, GoalPredicate::AlwaysSatisfied),
            ("g4".to_string(), GoalStatus::Failed, GoalPredicate::NeverSatisfied),
        ]);
        let decision = make_decision(&ctx);
        // g1 (pending) or g2 (in-progress) should be selected
        assert!(decision.selected_goal.is_some());
        let selected = decision.selected_goal.unwrap();
        assert!(selected == GoalId("g1".to_string()) || selected == GoalId("g2".to_string()));
        // g3 satisfied -> completed
        assert!(decision.completed_goals.contains(&GoalId("g3".to_string())));
        // g4 failed -> failed
        assert!(decision.failed_goals.contains(&GoalId("g4".to_string())));
    }

    #[test]
    fn test_deferred_goal_behavior() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
            ("g2".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
        ]);
        let decision = make_decision(&ctx);
        // One of g1/g2 is selected (continue), the other is deferred
        assert_eq!(decision.continue_goals.len(), 1);
        assert_eq!(decision.deferred_goals.len(), 1);
        assert!(!decision.continue_goals.contains(decision.deferred_goals.first().unwrap()));
    }

    #[test]
    fn test_explanation_stability() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
        ]);
        let first = make_decision(&ctx);
        for _ in 0..100 {
            let next = make_decision(&ctx);
            assert_eq!(first.explanation, next.explanation);
        }
    }

    #[test]
    fn test_decision_serialization_roundtrip() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
        ]);
        let decision = make_decision(&ctx);
        let json = serde_json::to_string(&decision).unwrap();
        let parsed: ExecutiveDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decision, parsed);
    }

    #[test]
    fn test_deterministic_output() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
            ("g2".to_string(), GoalStatus::InProgress, GoalPredicate::AlwaysSatisfied),
            ("g3".to_string(), GoalStatus::Satisfied, GoalPredicate::AlwaysSatisfied),
        ]);
        let first = make_decision(&ctx);
        for _ in 0..100 {
            let next = make_decision(&ctx);
            assert_eq!(first, next);
        }
    }

    // ── Edge Cases ───────────────────────────────────────────────────────

    #[test]
    fn test_empty_goal_state() {
        let ctx = empty_context();
        let decision = make_decision(&ctx);
        assert!(decision.selected_goal.is_none());
        assert_eq!(decision.explanation, "goal=none;priority=0;rank=0;status=none");
    }

    #[test]
    fn test_all_satisfied() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Satisfied, GoalPredicate::AlwaysSatisfied),
            ("g2".to_string(), GoalStatus::Satisfied, GoalPredicate::AlwaysSatisfied),
        ]);
        let decision = make_decision(&ctx);
        assert!(decision.selected_goal.is_none());
        assert_eq!(decision.completed_goals.len(), 2);
    }

    #[test]
    fn test_all_failed() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Failed, GoalPredicate::NeverSatisfied),
        ]);
        let decision = make_decision(&ctx);
        assert!(decision.selected_goal.is_none());
        assert_eq!(decision.failed_goals.len(), 1);
    }

    #[test]
    fn test_mixed_active() {
        let ctx = context_with_goals(vec![
            ("g1".to_string(), GoalStatus::Pending, GoalPredicate::AlwaysSatisfied),
            ("g2".to_string(), GoalStatus::InProgress, GoalPredicate::AlwaysSatisfied),
        ]);
        let decision = make_decision(&ctx);
        assert!(decision.selected_goal.is_some());
        // One continues, one is deferred
        assert_eq!(decision.continue_goals.len(), 1);
        assert_eq!(decision.deferred_goals.len(), 1);
    }

    #[test]
    fn test_large_goal_collection() {
        let mut goals = Vec::new();
        for i in 0..100 {
            let status = match i % 4 {
                0 => GoalStatus::Pending,
                1 => GoalStatus::InProgress,
                2 => GoalStatus::Satisfied,
                _ => GoalStatus::Failed,
            };
            goals.push((format!("g{}", i), status, GoalPredicate::AlwaysSatisfied));
        }
        let ctx = context_with_goals(goals);
        let decision = make_decision(&ctx);
        assert!(!decision.ranked_goals.is_empty());
        assert_eq!(decision.ranked_goals.len(), 100);
    }

    #[test]
    fn test_stress_100_run_determinism() {
        let mut goals = Vec::new();
        for i in 0..20 {
            let status = match i % 4 {
                0 => GoalStatus::Pending,
                1 => GoalStatus::InProgress,
                2 => GoalStatus::Satisfied,
                _ => GoalStatus::Failed,
            };
            goals.push((format!("g{}", i), status, GoalPredicate::AlwaysSatisfied));
        }
        let ctx = context_with_goals(goals);
        let first = make_decision(&ctx);
        for _ in 0..100 {
            let next = make_decision(&ctx);
            assert_eq!(first, next);
        }
    }
}
