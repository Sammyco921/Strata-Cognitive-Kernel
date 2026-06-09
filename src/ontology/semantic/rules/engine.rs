use std::collections::{BTreeMap, BTreeSet};

use crate::ontology::semantic::types::{SemanticGraph, TypedNode, TypedEdge};
use crate::ontology::semantic::query::types::ResultSet;
use crate::ontology::semantic::rules::types::{RuleSpec, AnnotatedResultSet};

pub fn apply_rules(
    result: &ResultSet,
    _graph: &SemanticGraph,
    rules: &BTreeSet<RuleSpec>,
) -> AnnotatedResultSet {
    let mut node_tags: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    let mut edge_tags: BTreeMap<u64, Vec<String>> = BTreeMap::new();

    for rule in rules {
        // Node matching
        let has_node_conditions = rule.node_type_match.is_some()
            || !rule.node_property_matches.is_empty()
            || rule.specific_node_id.is_some();

        if has_node_conditions {
            for (&node_id, node) in &result.nodes {
                if node_matches_rule(node, rule) {
                    node_tags.entry(node_id).or_default().push(rule.tag.clone());
                }
            }
        }

        // Edge matching
        let has_edge_conditions = rule.edge_type_match.is_some()
            || !rule.edge_property_matches.is_empty()
            || rule.specific_edge_id.is_some();

        if has_edge_conditions {
            for (&edge_id, edge) in &result.edges {
                if edge_matches_rule(edge, rule) {
                    edge_tags.entry(edge_id).or_default().push(rule.tag.clone());
                }
            }
        }
    }

    AnnotatedResultSet {
        result_set: result.clone(),
        node_tags,
        edge_tags,
    }
}

fn node_matches_rule(node: &TypedNode, rule: &RuleSpec) -> bool {
    if let Some(ref nt) = rule.node_type_match {
        if node.node_type != *nt {
            return false;
        }
    }
    if let Some(sid) = rule.specific_node_id {
        if node.id != sid {
            return false;
        }
    }
    for pm in &rule.node_property_matches {
        if node.properties.get(&pm.key) != Some(&pm.value) {
            return false;
        }
    }
    true
}

fn edge_matches_rule(edge: &TypedEdge, rule: &RuleSpec) -> bool {
    if let Some(ref et) = rule.edge_type_match {
        if edge.edge_type != *et {
            return false;
        }
    }
    if let Some(sid) = rule.specific_edge_id {
        if edge.id != sid {
            return false;
        }
    }
    for pm in &rule.edge_property_matches {
        if edge.properties.get(&pm.key) != Some(&pm.value) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::semantic::query::types::ResultSet;
    use crate::ontology::semantic::rules::types::PropertyMatch;
    use std::collections::BTreeMap;

    // ── Test graph (same as G4 tests) ────────────────────────────────────
    //
    //   [1: person]  --(10: knows)-->  [2: person]
    //       |
    //    (11: works_at)
    //       v
    //   [3: organization]
    //       |
    //    (12: knows)
    //       v
    //   [4: person]
    //

    fn make_test_graph() -> SemanticGraph {
        SemanticGraph {
            nodes: BTreeMap::from([
                (1, TypedNode {
                    id: 1, node_type: "person".into(),
                    properties: BTreeMap::from([("name".into(), "Alice".into())]),
                    semantic_type_name: Some("person".into()),
                    semantic_type_description: Some("A person".into()),
                }),
                (2, TypedNode {
                    id: 2, node_type: "person".into(),
                    properties: BTreeMap::from([("name".into(), "Bob".into())]),
                    semantic_type_name: Some("person".into()),
                    semantic_type_description: Some("A person".into()),
                }),
                (3, TypedNode {
                    id: 3, node_type: "organization".into(),
                    properties: BTreeMap::from([("name".into(), "Acme".into())]),
                    semantic_type_name: Some("organization".into()),
                    semantic_type_description: Some("An org".into()),
                }),
                (4, TypedNode {
                    id: 4, node_type: "person".into(),
                    properties: BTreeMap::from([("name".into(), "Carol".into())]),
                    semantic_type_name: Some("person".into()),
                    semantic_type_description: Some("A person".into()),
                }),
            ]),
            edges: BTreeMap::from([
                (10, TypedEdge {
                    id: 10, from_node: 1, to_node: 2,
                    edge_type: "knows".into(),
                    properties: BTreeMap::new(),
                    semantic_type_name: Some("knows".into()),
                    semantic_type_description: Some("Knows relation".into()),
                    from_node_type: Some("person".into()),
                    to_node_type: Some("person".into()),
                }),
                (11, TypedEdge {
                    id: 11, from_node: 1, to_node: 3,
                    edge_type: "works_at".into(),
                    properties: BTreeMap::from([("since".into(), "2020".into())]),
                    semantic_type_name: Some("works_at".into()),
                    semantic_type_description: Some("Employment".into()),
                    from_node_type: Some("person".into()),
                    to_node_type: Some("organization".into()),
                }),
                (12, TypedEdge {
                    id: 12, from_node: 2, to_node: 4,
                    edge_type: "knows".into(),
                    properties: BTreeMap::new(),
                    semantic_type_name: Some("knows".into()),
                    semantic_type_description: Some("Knows relation".into()),
                    from_node_type: Some("person".into()),
                    to_node_type: Some("person".into()),
                }),
            ]),
        }
    }

    fn make_full_result_set() -> ResultSet {
        let graph = make_test_graph();
        ResultSet {
            nodes: graph.nodes.clone(),
            edges: graph.edges.clone(),
        }
    }

    // ── Test 1: Deterministic rule application ────────────────────────────

    #[test]
    fn test_deterministic_application() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "is_person".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let a1 = apply_rules(&result, &graph, &rules);
        let a2 = apply_rules(&result, &graph, &rules);
        assert_eq!(a1, a2);
        assert_eq!(a1.to_deterministic_string(), a2.to_deterministic_string());
    }

    // ── Test 2: Rule order invariance (BTreeSet enforces sorted by id) ────

    #[test]
    fn test_rule_order_invariance() {
        let result = make_full_result_set();
        let graph = make_test_graph();

        // Rules inserted in different order, BTreeSet ensures sorted by id
        let rules_a: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "z".into(), tag: "tag_z".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
            RuleSpec {
                id: "a".into(), tag: "tag_a".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let rules_b: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "a".into(), tag: "tag_a".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
            RuleSpec {
                id: "z".into(), tag: "tag_z".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        assert_eq!(rules_a, rules_b);
        let a1 = apply_rules(&result, &graph, &rules_a);
        let a2 = apply_rules(&result, &graph, &rules_b);
        assert_eq!(a1, a2);
    }

    // ── Test 3: Node type annotation ──────────────────────────────────────

    #[test]
    fn test_node_type_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "is_person".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Person nodes (1, 2, 4) should be tagged
        assert_eq!(annotated.node_tags.get(&1).unwrap(), &vec!["is_person".to_string()]);
        assert_eq!(annotated.node_tags.get(&2).unwrap(), &vec!["is_person".to_string()]);
        assert_eq!(annotated.node_tags.get(&4).unwrap(), &vec!["is_person".to_string()]);
        // Organization node (3) should NOT be tagged
        assert!(!annotated.node_tags.contains_key(&3));
        // No edge tags should exist
        assert!(annotated.edge_tags.is_empty());
    }

    // ── Test 4: Edge type annotation ──────────────────────────────────────

    #[test]
    fn test_edge_type_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "knows_relationship".into(),
                node_type_match: None,
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: Some("knows".into()),
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Knows edges (10, 12) should be tagged
        assert_eq!(annotated.edge_tags.get(&10).unwrap(), &vec!["knows_relationship".to_string()]);
        assert_eq!(annotated.edge_tags.get(&12).unwrap(), &vec!["knows_relationship".to_string()]);
        // Works_at edge (11) should NOT be tagged
        assert!(!annotated.edge_tags.contains_key(&11));
        // No node tags should exist
        assert!(annotated.node_tags.is_empty());
    }

    // ── Test 5: Property match annotation ─────────────────────────────────

    #[test]
    fn test_node_property_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "acme".into(),
                node_type_match: None,
                node_property_matches: vec![PropertyMatch { key: "name".into(), value: "Acme".into() }],
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Only node 3 (Acme) should be tagged
        assert_eq!(annotated.node_tags.get(&3).unwrap(), &vec!["acme".to_string()]);
        assert_eq!(annotated.node_tags.len(), 1);
    }

    #[test]
    fn test_edge_property_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "started_2020".into(),
                node_type_match: None,
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: vec![PropertyMatch { key: "since".into(), value: "2020".into() }],
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Only edge 11 (works_at, since=2020) should be tagged
        assert_eq!(annotated.edge_tags.get(&11).unwrap(), &vec!["started_2020".to_string()]);
        assert_eq!(annotated.edge_tags.len(), 1);
    }

    // ── Test 6: Specific node/edge ID annotation ──────────────────────────

    #[test]
    fn test_specific_node_id_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "alice".into(),
                node_type_match: None,
                node_property_matches: Vec::new(),
                specific_node_id: Some(1),
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        assert_eq!(annotated.node_tags.get(&1).unwrap(), &vec!["alice".to_string()]);
        assert_eq!(annotated.node_tags.len(), 1);
    }

    #[test]
    fn test_specific_edge_id_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "alice_knows_bob".into(),
                node_type_match: None,
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: Some(10),
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        assert_eq!(annotated.edge_tags.get(&10).unwrap(), &vec!["alice_knows_bob".to_string()]);
        assert_eq!(annotated.edge_tags.len(), 1);
    }

    // ── Test 7: No mutation of input ResultSet ────────────────────────────

    #[test]
    fn test_no_mutation_of_result_set() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let result_copy = result.clone();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "tag".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let _annotated = apply_rules(&result, &graph, &rules);
        assert_eq!(result, result_copy);
    }

    // ── Test 8: No mutation of SemanticGraph ──────────────────────────────

    #[test]
    fn test_no_mutation_of_graph() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let graph_copy = graph.clone();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "tag".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let _annotated = apply_rules(&result, &graph, &rules);
        assert_eq!(graph, graph_copy);
    }

    // ── Test 9: Empty rule set ────────────────────────────────────────────

    #[test]
    fn test_empty_rule_set() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::new();

        let annotated = apply_rules(&result, &graph, &rules);

        assert_eq!(annotated.node_tags.len(), 0);
        assert_eq!(annotated.edge_tags.len(), 0);
        assert_eq!(annotated.result_set, result);
    }

    // ── Test 10: Empty result set ─────────────────────────────────────────

    #[test]
    fn test_empty_result_set() {
        let result = ResultSet { nodes: BTreeMap::new(), edges: BTreeMap::new() };
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "tag".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        assert_eq!(annotated.node_tags.len(), 0);
        assert_eq!(annotated.edge_tags.len(), 0);
        assert!(annotated.result_set.nodes.is_empty());
        assert!(annotated.result_set.edges.is_empty());
    }

    // ── Test 11: Multiple rule overlap ────────────────────────────────────

    #[test]
    fn test_multiple_rule_overlap() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "person_tag".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
            RuleSpec {
                id: "r2".into(), tag: "alice_tag".into(),
                node_type_match: None,
                node_property_matches: Vec::new(),
                specific_node_id: Some(1),
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Node 1 should have both tags (in rule order: r1 then r2)
        assert_eq!(annotated.node_tags.get(&1).unwrap(), &vec!["person_tag".to_string(), "alice_tag".to_string()]);
    }

    // ── Test 12: Conflicting rule independence ────────────────────────────

    #[test]
    fn test_conflicting_rule_independence() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "person".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
            RuleSpec {
                id: "r2".into(), tag: "not_person".into(),
                node_type_match: Some("organization".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Person nodes should have "person" tag only
        assert_eq!(annotated.node_tags.get(&1).unwrap(), &vec!["person".to_string()]);
        // Organization node should have "not_person" tag only
        assert_eq!(annotated.node_tags.get(&3).unwrap(), &vec!["not_person".to_string()]);
    }

    // ── Test 13: Serialization determinism ────────────────────────────────

    #[test]
    fn test_rulespec_serialization_determinism() {
        let spec = RuleSpec {
            id: "test".into(), tag: "mytag".into(),
            node_type_match: Some("person".into()),
            node_property_matches: vec![PropertyMatch { key: "k".into(), value: "v".into() }],
            specific_node_id: Some(42),
            edge_type_match: None,
            edge_property_matches: Vec::new(),
            specific_edge_id: None,
        };
        let s1 = spec.to_deterministic_string();
        let s2 = spec.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_annotated_resultset_serialization_determinism() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "is_person".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: Some("knows".into()),
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);
        let s1 = annotated.to_deterministic_string();
        let s2 = annotated.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    // ── Test 14: Serialization roundtrip ──────────────────────────────────

    #[test]
    fn test_property_match_serialization_roundtrip() {
        let pm = PropertyMatch { key: "name".into(), value: "Alice".into() };
        let s = pm.to_deterministic_string();
        let back = PropertyMatch::from_deterministic_string(&s).unwrap();
        assert_eq!(pm, back);
    }

    #[test]
    fn test_rulespec_serialization_roundtrip() {
        let specs = vec![
            RuleSpec {
                id: "r1".into(), tag: "t1".into(),
                node_type_match: Some("person".into()),
                node_property_matches: vec![PropertyMatch { key: "k".into(), value: "v".into() }],
                specific_node_id: Some(1),
                edge_type_match: Some("knows".into()),
                edge_property_matches: vec![PropertyMatch { key: "k2".into(), value: "v2".into() }],
                specific_edge_id: Some(10),
            },
            RuleSpec {
                id: "r2".into(), tag: "t2".into(),
                node_type_match: None,
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ];
        for spec in &specs {
            let s = spec.to_deterministic_string();
            let back = RuleSpec::from_deterministic_string(&s).unwrap();
            assert_eq!(*spec, back);
        }
    }

    #[test]
    fn test_annotated_resultset_serialization_roundtrip() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "is_person".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: Some("knows".into()),
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);
        let s = annotated.to_deterministic_string();
        let back = AnnotatedResultSet::from_deterministic_string(&s).unwrap();
        assert_eq!(annotated, back);
    }

    #[test]
    fn test_annotated_resultset_empty_roundtrip() {
        let result = ResultSet { nodes: BTreeMap::new(), edges: BTreeMap::new() };
        let annotated = AnnotatedResultSet {
            result_set: result.clone(),
            node_tags: BTreeMap::new(),
            edge_tags: BTreeMap::new(),
        };
        let s = annotated.to_deterministic_string();
        let back = AnnotatedResultSet::from_deterministic_string(&s).unwrap();
        assert_eq!(annotated, back);
    }

    // ── Test 15: Stability across 100 runs ────────────────────────────────

    #[test]
    fn test_stability_100_runs() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "person".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
            RuleSpec {
                id: "r2".into(), tag: "knows".into(),
                node_type_match: None,
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: Some("knows".into()),
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let first = apply_rules(&result, &graph, &rules);
        let first_str = first.to_deterministic_string();
        for _ in 0..100 {
            let ann = apply_rules(&result, &graph, &rules);
            assert_eq!(first_str, ann.to_deterministic_string());
        }
    }

    // ── Test 16: Mixed conditions (AND semantics) ─────────────────────────

    #[test]
    fn test_and_condition_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();

        // Rule: tag "person" nodes with property name="Alice" as "alice_person"
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "alice_person".into(),
                node_type_match: Some("person".into()),
                node_property_matches: vec![PropertyMatch { key: "name".into(), value: "Alice".into() }],
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Only node 1 (Alice, person) should match
        assert_eq!(annotated.node_tags.get(&1).unwrap(), &vec!["alice_person".to_string()]);
        // Node 2 (Bob, person) should NOT match (wrong name)
        assert!(!annotated.node_tags.contains_key(&2));
        // Node 3 (Acme, organization) should NOT match (wrong type)
        assert!(!annotated.node_tags.contains_key(&3));
        assert_eq!(annotated.node_tags.len(), 1);
    }

    // ── Test 17: Both node and edge annotation in one rule ────────────────

    #[test]
    fn test_both_node_and_edge_annotation() {
        let result = make_full_result_set();
        let graph = make_test_graph();

        // Rule: tag person nodes AND knows edges
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "social".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: Some("knows".into()),
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // Person nodes (1, 2, 4) should be tagged
        assert_eq!(annotated.node_tags.get(&1).unwrap(), &vec!["social".to_string()]);
        assert_eq!(annotated.node_tags.get(&2).unwrap(), &vec!["social".to_string()]);
        assert_eq!(annotated.node_tags.get(&4).unwrap(), &vec!["social".to_string()]);
        assert_eq!(annotated.node_tags.len(), 3);
        // Knows edges (10, 12) should be tagged
        assert_eq!(annotated.edge_tags.get(&10).unwrap(), &vec!["social".to_string()]);
        assert_eq!(annotated.edge_tags.get(&12).unwrap(), &vec!["social".to_string()]);
        assert_eq!(annotated.edge_tags.len(), 2);
    }

    // ── Test 18: Original result set is preserved in output ───────────────

    #[test]
    fn test_original_result_preserved() {
        let result = make_full_result_set();
        let graph = make_test_graph();
        let rules: BTreeSet<RuleSpec> = BTreeSet::from([
            RuleSpec {
                id: "r1".into(), tag: "t".into(),
                node_type_match: Some("person".into()),
                node_property_matches: Vec::new(),
                specific_node_id: None,
                edge_type_match: None,
                edge_property_matches: Vec::new(),
                specific_edge_id: None,
            },
        ]);

        let annotated = apply_rules(&result, &graph, &rules);

        // The result_set inside annotated must equal the original
        assert_eq!(annotated.result_set, result);
        // Verify specific content
        assert_eq!(annotated.result_set.nodes.len(), 4);
        assert_eq!(annotated.result_set.edges.len(), 3);
    }
}
