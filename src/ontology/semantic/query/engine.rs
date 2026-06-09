use std::collections::{BTreeMap, BTreeSet};

use crate::ontology::semantic::types::{SemanticGraph, TypedNode, TypedEdge};
use crate::ontology::types::OntologyRegistry;
use crate::ontology::semantic::query::types::{QuerySpec, ResultSet};

pub fn query(
    graph: &SemanticGraph,
    _ontology: &OntologyRegistry,
    spec: &QuerySpec,
) -> ResultSet {
    // Step 1: Filter nodes by all applicable constraints
    let mut candidate_nodes: BTreeMap<u64, TypedNode> = BTreeMap::new();

    for (&id, node) in &graph.nodes {
        // node_type_filter
        if let Some(ref types) = spec.node_type_filter {
            if !types.contains(&node.node_type) {
                continue;
            }
        }
        // source_node_ids (when NOT doing traversal)
        if spec.traversal_depth.is_none() {
            if let Some(ref source_ids) = spec.source_node_ids {
                if !source_ids.contains(&id) {
                    continue;
                }
            }
        }
        // target_node_ids
        if let Some(ref target_ids) = spec.target_node_ids {
            if !target_ids.contains(&id) {
                continue;
            }
        }
        // property_filters
        if !spec.property_filters.is_empty() {
            let mut passes = true;
            for pf in &spec.property_filters {
                if node.properties.get(&pf.key) != Some(&pf.value) {
                    passes = false;
                    break;
                }
            }
            if !passes {
                continue;
            }
        }
        candidate_nodes.insert(id, node.clone());
    }

    // Step 2: If traversal is specified, expand from source nodes
    let mut final_node_ids: BTreeSet<u64>;
    let mut final_edge_ids: BTreeSet<u64> = BTreeSet::new();

    if let Some(depth) = spec.traversal_depth {
        if let Some(ref source_ids) = spec.source_node_ids {
            let (visited, traversed) = bfs_traverse(graph, source_ids, depth);
            final_node_ids = visited;
            final_edge_ids = traversed;

            // Apply node_type_filter to visited nodes (if any)
            if let Some(ref types) = spec.node_type_filter {
                let mut filtered: BTreeSet<u64> = BTreeSet::new();
                for &id in &final_node_ids {
                    if let Some(node) = graph.nodes.get(&id) {
                        if types.contains(&node.node_type) {
                            filtered.insert(id);
                        }
                    }
                }
                final_node_ids = filtered;
            }

            // Apply property_filters to visited nodes (if any)
            if !spec.property_filters.is_empty() {
                let mut filtered: BTreeSet<u64> = BTreeSet::new();
                for &id in &final_node_ids {
                    if let Some(node) = graph.nodes.get(&id) {
                        let mut passes = true;
                        for pf in &spec.property_filters {
                            if node.properties.get(&pf.key) != Some(&pf.value) {
                                passes = false;
                                break;
                            }
                        }
                        if passes {
                            filtered.insert(id);
                        }
                    }
                }
                final_node_ids = filtered;
            }
        } else {
            // No source IDs but traversal depth set -> no traversal possible
            final_node_ids = candidate_nodes.keys().copied().collect();
        }
    } else {
        // No traversal: use filtered candidates as-is
        final_node_ids = candidate_nodes.keys().copied().collect();
    }

    // Step 3: Collect edges where both endpoints are in final node set
    //         (unless traversal already provided traversed edges)
    let mut result_nodes: BTreeMap<u64, TypedNode> = BTreeMap::new();
    let mut result_edges: BTreeMap<u64, TypedEdge> = BTreeMap::new();

    for &id in &final_node_ids {
        if let Some(node) = graph.nodes.get(&id) {
            result_nodes.insert(id, node.clone());
        }
    }

    if !final_edge_ids.is_empty() {
        // Traversal provided specific edges
        for &id in &final_edge_ids {
            if let Some(edge) = graph.edges.get(&id) {
                if let Some(ref edge_types) = spec.edge_type_filter {
                    if edge_types.contains(&edge.edge_type) {
                        result_edges.insert(id, edge.clone());
                    }
                } else {
                    result_edges.insert(id, edge.clone());
                }
            }
        }
    } else {
        // Collect edges where both endpoints are in result nodes
        for (&id, edge) in &graph.edges {
            if result_nodes.contains_key(&edge.from_node) && result_nodes.contains_key(&edge.to_node) {
                if let Some(ref edge_types) = spec.edge_type_filter {
                    if edge_types.contains(&edge.edge_type) {
                        result_edges.insert(id, edge.clone());
                    }
                } else {
                    result_edges.insert(id, edge.clone());
                }
            }
        }
    }

    ResultSet { nodes: result_nodes, edges: result_edges }
}

/// Deterministic BFS traversal from source nodes up to max_depth.
/// Uses BTreeSet for deterministic ordering at each frontier level.
/// Returns (visited_node_ids, traversed_edge_ids).
fn bfs_traverse(
    graph: &SemanticGraph,
    source_ids: &[u64],
    max_depth: usize,
) -> (BTreeSet<u64>, BTreeSet<u64>) {
    let mut visited: BTreeSet<u64> = BTreeSet::new();
    let mut traversed_edges: BTreeSet<u64> = BTreeSet::new();
    let mut frontier: BTreeSet<u64> = BTreeSet::new();

    for &sid in source_ids {
        if graph.nodes.contains_key(&sid) {
            visited.insert(sid);
            frontier.insert(sid);
        }
    }

    for _depth in 0..max_depth {
        if frontier.is_empty() {
            break;
        }

        // Find all edges from current frontier nodes
        let mut next_frontier: BTreeSet<u64> = BTreeSet::new();
        for &node_id in &frontier {
            for (&edge_id, edge) in &graph.edges {
                if edge.from_node == node_id {
                    traversed_edges.insert(edge_id);
                    if !visited.contains(&edge.to_node) {
                        next_frontier.insert(edge.to_node);
                    }
                }
            }
        }

        for &nid in &next_frontier {
            visited.insert(nid);
        }
        frontier = next_frontier;
    }

    (visited, traversed_edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::*;
    use crate::ontology::semantic::query::types::PropertyFilter;
    use std::collections::BTreeMap;

    // ── Test graph: 3 nodes, 2 edges ─────────────────────────────────────
    //
    //   [1: person]  --(10: knows)-->  [2: person]
    //       |
    //    (11: works_at)
    //       v
    //   [3: organization]
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

    fn make_empty_ontology() -> OntologyRegistry {
        OntologyRegistry::empty()
    }

    // ── Test 1: Deterministic query results ───────────────────────────────

    #[test]
    fn test_deterministic_results() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };

        let r1 = query(&graph, &onto, &spec);
        let r2 = query(&graph, &onto, &spec);
        assert_eq!(r1, r2);
        assert_eq!(r1.to_deterministic_string(), r2.to_deterministic_string());
    }

    // ── Test 2: Order invariance across input graph construction ─────────

    #[test]
    fn test_order_invariance() {
        // Build graph with different insertion order but same semantics
        let mut nodes1 = BTreeMap::new();
        nodes1.insert(1, make_test_graph().nodes.get(&1).unwrap().clone());
        nodes1.insert(2, make_test_graph().nodes.get(&2).unwrap().clone());
        let mut nodes2 = BTreeMap::new();
        nodes2.insert(2, make_test_graph().nodes.get(&2).unwrap().clone());
        nodes2.insert(1, make_test_graph().nodes.get(&1).unwrap().clone());

        let g1 = SemanticGraph { nodes: nodes1, edges: BTreeMap::new() };
        let g2 = SemanticGraph { nodes: nodes2, edges: BTreeMap::new() };

        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: Some(vec!["person".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };

        // BTreeMap ensures both graphs have the same iteration order
        assert_eq!(g1, g2);
        let r1 = query(&g1, &onto, &spec);
        let r2 = query(&g2, &onto, &spec);
        assert_eq!(r1, r2);
    }

    // ── Test 3: Node type filtering ──────────────────────────────────────

    #[test]
    fn test_node_type_filter() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: Some(vec!["person".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert_eq!(result.nodes.len(), 3);
        for node in result.nodes.values() {
            assert_eq!(node.node_type, "person");
        }
    }

    #[test]
    fn test_node_type_filter_organization() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: Some(vec!["organization".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes.get(&3).unwrap().node_type, "organization");
    }

    // ── Test 4: Edge type filtering ──────────────────────────────────────

    #[test]
    fn test_edge_type_filter() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: Some(vec!["knows".into()]),
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        // Only knows edges should match: 10 and 12
        assert_eq!(result.edges.len(), 2);
        for edge in result.edges.values() {
            assert_eq!(edge.edge_type, "knows");
        }
    }

    // ── Test 5: Property filtering ───────────────────────────────────────

    #[test]
    fn test_property_filter() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: vec![PropertyFilter { key: "name".into(), value: "Alice".into() }],
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes.get(&1).unwrap().properties.get("name").unwrap(), "Alice");
    }

    // ── Test 6: Depth-limited traversal ──────────────────────────────────

    #[test]
    fn test_traversal_depth_1() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Start from node 1 (Alice), traverse 1 step
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: Some(1),
            source_node_ids: Some(vec![1]),
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        // Should reach nodes 2 (Bob) and 3 (Acme) in 1 step
        assert_eq!(result.nodes.len(), 3);
        assert!(result.nodes.contains_key(&1));
        assert!(result.nodes.contains_key(&2));
        assert!(result.nodes.contains_key(&3));
        assert_eq!(result.edges.len(), 2);
    }

    #[test]
    fn test_traversal_depth_2() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Start from node 1 (Alice), traverse 2 steps
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: Some(2),
            source_node_ids: Some(vec![1]),
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        // Should reach all 4 nodes in 2 steps (1→2→4, 1→3)
        assert_eq!(result.nodes.len(), 4);
        assert_eq!(result.edges.len(), 3);
    }

    #[test]
    fn test_traversal_depth_0() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Depth 0 means only the source node itself
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: Some(0),
            source_node_ids: Some(vec![1]),
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert_eq!(result.nodes.len(), 1);
        assert!(result.nodes.contains_key(&1));
        assert_eq!(result.edges.len(), 0);
    }

    // ── Test 7: Empty graph handling ─────────────────────────────────────

    #[test]
    fn test_empty_graph() {
        let graph = SemanticGraph { nodes: BTreeMap::new(), edges: BTreeMap::new() };
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    // ── Test 8: Non-matching query ───────────────────────────────────────

    #[test]
    fn test_non_matching_query() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Query for a non-existent node type
        let spec = QuerySpec {
            node_type_filter: Some(vec!["robot".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    #[test]
    fn test_non_matching_traversal() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Traverse from a non-existent node
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: Some(2),
            source_node_ids: Some(vec![99]),
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }

    // ── Test 9: Mixed filter intersection ─────────────────────────────────

    #[test]
    fn test_mixed_filter_intersection() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Node type = person AND property (name = "Alice")
        let spec = QuerySpec {
            node_type_filter: Some(vec!["person".into()]),
            edge_type_filter: None,
            property_filters: vec![PropertyFilter { key: "name".into(), value: "Alice".into() }],
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert_eq!(result.nodes.len(), 1);
        assert!(result.nodes.contains_key(&1));
    }

    #[test]
    fn test_traversal_with_type_filter() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Traverse from node 1, depth 2, only person nodes
        let spec = QuerySpec {
            node_type_filter: Some(vec!["person".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: Some(2),
            source_node_ids: Some(vec![1]),
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        // During traversal, all nodes are visited (1→2→4, 1→3).
        // Then node_type_filter keeps only person nodes (1, 2, 4)
        assert_eq!(result.nodes.len(), 3);
        assert!(result.nodes.contains_key(&1));
        assert!(result.nodes.contains_key(&2));
        assert!(result.nodes.contains_key(&4));
        assert!(!result.nodes.contains_key(&3)); // org filtered out
    }

    #[test]
    fn test_traversal_with_edge_type_filter() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Traverse from node 1, depth 2, only "knows" edges
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: Some(vec!["knows".into()]),
            property_filters: Vec::new(),
            traversal_depth: Some(2),
            source_node_ids: Some(vec![1]),
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        // Traversal traverses all edges (10, 11, 12)
        // Then edge_type_filter keeps only "knows" edges (10, 12)
        assert_eq!(result.edges.len(), 2);
        for edge in result.edges.values() {
            assert_eq!(edge.edge_type, "knows");
        }
    }

    // ── Test 10: Stability across repeated executions ────────────────────

    #[test]
    fn test_stability_100_runs() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let specs = vec![
            QuerySpec { node_type_filter: Some(vec!["person".into()]), edge_type_filter: None, property_filters: Vec::new(), traversal_depth: None, source_node_ids: None, target_node_ids: None },
            QuerySpec { node_type_filter: None, edge_type_filter: Some(vec!["knows".into()]), property_filters: Vec::new(), traversal_depth: None, source_node_ids: None, target_node_ids: None },
            QuerySpec { node_type_filter: None, edge_type_filter: None, property_filters: Vec::new(), traversal_depth: Some(1), source_node_ids: Some(vec![1]), target_node_ids: None },
            QuerySpec { node_type_filter: Some(vec!["person".into()]), edge_type_filter: None, property_filters: vec![PropertyFilter { key: "name".into(), value: "Alice".into() }], traversal_depth: None, source_node_ids: None, target_node_ids: None },
        ];

        for spec in &specs {
            let first = query(&graph, &onto, spec);
            let first_str = first.to_deterministic_string();
            for _ in 0..100 {
                let result = query(&graph, &onto, spec);
                assert_eq!(first_str, result.to_deterministic_string());
            }
        }
    }

    // ── Test 11: Serialization determinism ────────────────────────────────

    #[test]
    fn test_queryspec_serialization_determinism() {
        let spec = QuerySpec {
            node_type_filter: Some(vec!["person".into(), "organization".into()]),
            edge_type_filter: Some(vec!["knows".into()]),
            property_filters: vec![PropertyFilter { key: "name".into(), value: "Alice".into() }],
            traversal_depth: Some(2),
            source_node_ids: Some(vec![1, 2]),
            target_node_ids: Some(vec![3]),
        };
        let s1 = spec.to_deterministic_string();
        let s2 = spec.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_queryspec_serialization_roundtrip() {
        let specs = vec![
            QuerySpec { node_type_filter: None, edge_type_filter: None, property_filters: Vec::new(), traversal_depth: None, source_node_ids: None, target_node_ids: None },
            QuerySpec { node_type_filter: Some(vec!["person".into()]), edge_type_filter: None, property_filters: Vec::new(), traversal_depth: None, source_node_ids: None, target_node_ids: None },
            QuerySpec { node_type_filter: None, edge_type_filter: Some(vec!["knows".into()]), property_filters: Vec::new(), traversal_depth: Some(1), source_node_ids: Some(vec![1]), target_node_ids: None },
            QuerySpec { node_type_filter: Some(vec!["person".into()]), edge_type_filter: None, property_filters: vec![PropertyFilter { key: "name".into(), value: "Alice".into() }], traversal_depth: Some(2), source_node_ids: Some(vec![1, 2]), target_node_ids: Some(vec![3]) },
        ];
        for spec in &specs {
            let s = spec.to_deterministic_string();
            let back = QuerySpec::from_deterministic_string(&s);
            assert!(back.is_some(), "Failed to deserialize: {}", s);
            assert_eq!(spec, &back.unwrap());
        }
    }

    #[test]
    fn test_resultset_serialization_determinism() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec { node_type_filter: None, edge_type_filter: None, property_filters: Vec::new(), traversal_depth: None, source_node_ids: None, target_node_ids: None };
        let result = query(&graph, &onto, &spec);
        let s1 = result.to_deterministic_string();
        let s2 = result.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_resultset_serialization_roundtrip() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec { node_type_filter: None, edge_type_filter: None, property_filters: Vec::new(), traversal_depth: None, source_node_ids: None, target_node_ids: None };
        let result = query(&graph, &onto, &spec);
        let s = result.to_deterministic_string();
        let back = ResultSet::from_deterministic_string(&s).unwrap();
        assert_eq!(result, back);
    }

    // ── Edge: source_node_ids without traversal ──────────────────────────

    #[test]
    fn test_source_filter_without_traversal() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // When no traversal_depth, source_node_ids acts as a node filter
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: Some(vec![1, 3]),
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert_eq!(result.nodes.len(), 2);
        assert!(result.nodes.contains_key(&1));
        assert!(result.nodes.contains_key(&3));
    }

    // ── Edge: target_node_ids filter ─────────────────────────────────────

    #[test]
    fn test_target_node_filter() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: Some(vec![2]),
        };
        let result = query(&graph, &onto, &spec);
        assert_eq!(result.nodes.len(), 1);
        assert!(result.nodes.contains_key(&2));
    }

    // ── Edge: PropertyFilter on non-existent key ────────────────────────

    #[test]
    fn test_property_filter_no_match() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let spec = QuerySpec {
            node_type_filter: None,
            edge_type_filter: None,
            property_filters: vec![PropertyFilter { key: "nonexistent".into(), value: "value".into() }],
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        };
        let result = query(&graph, &onto, &spec);
        assert!(result.nodes.is_empty());
    }
}
