use std::collections::{BTreeMap, BTreeSet};

use crate::ontology::semantic::types::{SemanticGraph, TypedNode, TypedEdge};
use crate::ontology::types::OntologyRegistry;
use crate::ontology::semantic::query::types::ResultSet;
use crate::ontology::semantic::query::engine::query;
use crate::ontology::semantic::rules::engine::apply_rules;
use crate::ontology::semantic::composition::types::{
    PipelineSpec, PipelineStep, PipelineResult, StepResult, PureTransform,
};

pub fn execute_pipeline(
    graph: &SemanticGraph,
    ontology: &OntologyRegistry,
    pipeline: &PipelineSpec,
) -> PipelineResult {
    let mut steps: Vec<StepResult> = Vec::new();
    let mut current_result = ResultSet {
        nodes: graph.nodes.clone(),
        edges: graph.edges.clone(),
    };

    for (index, step) in pipeline.steps.iter().enumerate() {
        match step {
            PipelineStep::QueryStep(query_spec) => {
                current_result = query(graph, ontology, query_spec);
                steps.push(StepResult {
                    index,
                    step_type: "query".to_string(),
                    output: current_result.clone(),
                });
            }
            PipelineStep::RuleStep(rules) => {
                let _annotated = apply_rules(&current_result, graph, rules);
                steps.push(StepResult {
                    index,
                    step_type: "rule".to_string(),
                    output: current_result.clone(),
                });
            }
            PipelineStep::TransformStep(transform) => {
                current_result = apply_transform(&current_result, graph, transform);
                steps.push(StepResult {
                    index,
                    step_type: format!("transform:{}", transform.name),
                    output: current_result.clone(),
                });
            }
        }
    }

    PipelineResult {
        steps,
        final_output: current_result,
    }
}

fn apply_transform(
    result: &ResultSet,
    _graph: &SemanticGraph,
    transform: &PureTransform,
) -> ResultSet {
    match transform.name.as_str() {
        "identity" => result.clone(),
        "filter_nodes_by_type" => {
            let types_str = transform.parameters.get("types").map(|s| s.as_str()).unwrap_or("");
            if types_str.is_empty() {
                return result.clone();
            }
            let types: BTreeSet<&str> = types_str.split(',').filter(|s| !s.is_empty()).collect();
            if types.is_empty() {
                return result.clone();
            }
            let nodes: BTreeMap<u64, TypedNode> = result.nodes.iter()
                .filter(|(_, n)| types.contains(n.node_type.as_str()))
                .map(|(&id, n)| (id, n.clone()))
                .collect();
            let edges: BTreeMap<u64, TypedEdge> = result.edges.iter()
                .filter(|(_, e)| {
                    nodes.contains_key(&e.from_node) && nodes.contains_key(&e.to_node)
                })
                .map(|(&id, e)| (id, e.clone()))
                .collect();
            ResultSet { nodes, edges }
        }
        "filter_edges_by_type" => {
            let types_str = transform.parameters.get("types").map(|s| s.as_str()).unwrap_or("");
            if types_str.is_empty() {
                return result.clone();
            }
            let types: BTreeSet<&str> = types_str.split(',').filter(|s| !s.is_empty()).collect();
            if types.is_empty() {
                return result.clone();
            }
            let edges: BTreeMap<u64, TypedEdge> = result.edges.iter()
                .filter(|(_, e)| types.contains(e.edge_type.as_str()))
                .map(|(&id, e)| (id, e.clone()))
                .collect();
            ResultSet {
                nodes: result.nodes.clone(),
                edges,
            }
        }
        _ => result.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::semantic::query::types::QuerySpec;
    use crate::ontology::semantic::rules::types::RuleSpec;
    use crate::ontology::semantic::composition::types::PureTransform;
    use std::collections::BTreeMap;

    // ── Test graph (same as G4/G3) ───────────────────────────────────────
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

    fn make_empty_ontology() -> OntologyRegistry {
        OntologyRegistry::empty()
    }

    // ── Test 1: Deterministic pipeline execution ──────────────────────────

    #[test]
    fn test_deterministic_execution() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                PipelineStep::TransformStep(PureTransform {
                    name: "identity".into(),
                    parameters: BTreeMap::new(),
                }),
            ],
        };

        let r1 = execute_pipeline(&graph, &onto, &pipeline);
        let r2 = execute_pipeline(&graph, &onto, &pipeline);
        assert_eq!(r1, r2);
        assert_eq!(r1.to_deterministic_string(), r2.to_deterministic_string());
    }

    // ── Test 2: Step order invariance ─────────────────────────────────────

    #[test]
    fn test_step_order_invariance() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Same pipeline spec always produces same result
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
            ],
        };

        let r1 = execute_pipeline(&graph, &onto, &pipeline);
        let r2 = execute_pipeline(&graph, &onto, &pipeline);
        assert_eq!(r1.final_output, r2.final_output);
    }

    // ── Test 3: QueryStep correctness ─────────────────────────────────────

    #[test]
    fn test_query_step_person_filter() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        assert_eq!(result.final_output.nodes.len(), 3);
        for node in result.final_output.nodes.values() {
            assert_eq!(node.node_type, "person");
        }
    }

    // ── Test 4: RuleStep correctness ──────────────────────────────────────

    #[test]
    fn test_rule_step_preserves_result() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // First query to get persons, then apply rule
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                PipelineStep::RuleStep(BTreeSet::from([
                    RuleSpec {
                        id: "r1".into(), tag: "person_tag".into(),
                        node_type_match: Some("person".into()),
                        node_property_matches: Vec::new(),
                        specific_node_id: None,
                        edge_type_match: None,
                        edge_property_matches: Vec::new(),
                        specific_edge_id: None,
                    },
                ])),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        // Rule step doesn't change the result set
        assert_eq!(result.final_output.nodes.len(), 3);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].step_type, "query");
        assert_eq!(result.steps[1].step_type, "rule");
    }

    // ── Test 5: TransformStep identity ────────────────────────────────────

    #[test]
    fn test_identity_transform_preserves_result() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                PipelineStep::TransformStep(PureTransform {
                    name: "identity".into(),
                    parameters: BTreeMap::new(),
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        assert_eq!(result.final_output.nodes.len(), 3);
        // Query result before transform
        let query_only = query(&graph, &onto, &QuerySpec {
            node_type_filter: Some(vec!["person".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        });
        assert_eq!(result.final_output, query_only);
    }

    // ── Test 6: Multi-step pipeline correctness ───────────────────────────

    #[test]
    fn test_multi_step_pipeline() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        // Query persons → filter to only organization (will be empty) → identity
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                PipelineStep::TransformStep(PureTransform {
                    name: "filter_nodes_by_type".into(),
                    parameters: BTreeMap::from([("types".into(), "organization".into())]),
                }),
                PipelineStep::TransformStep(PureTransform {
                    name: "identity".into(),
                    parameters: BTreeMap::new(),
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        // After person query → filter to organization → should be empty
        assert!(result.final_output.nodes.is_empty());
        assert!(result.final_output.edges.is_empty());
        assert_eq!(result.steps.len(), 3);
    }

    // ── Test 7: Empty pipeline ────────────────────────────────────────────

    #[test]
    fn test_empty_pipeline() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec { steps: Vec::new() };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        // Empty pipeline returns the full graph as result
        assert_eq!(result.final_output.nodes.len(), 4);
        assert_eq!(result.final_output.edges.len(), 3);
        assert_eq!(result.steps.len(), 0);
    }

    // ── Test 8: Mixed pipeline (query → rule → transform) ────────────────

    #[test]
    fn test_mixed_pipeline() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                // Step 0: Query all person nodes
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                // Step 1: Apply rule (annotation only, doesn't change result)
                PipelineStep::RuleStep(BTreeSet::from([
                    RuleSpec {
                        id: "r1".into(), tag: "person".into(),
                        node_type_match: Some("person".into()),
                        node_property_matches: Vec::new(),
                        specific_node_id: None,
                        edge_type_match: None,
                        edge_property_matches: Vec::new(),
                        specific_edge_id: None,
                    },
                ])),
                // Step 2: Filter to only knows edges
                PipelineStep::TransformStep(PureTransform {
                    name: "filter_edges_by_type".into(),
                    parameters: BTreeMap::from([("types".into(), "knows".into())]),
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);

        // After person query → rule → filter knows edges
        // All person nodes remain (edges filtered, nodes kept)
        assert_eq!(result.final_output.nodes.len(), 3);
        assert_eq!(result.final_output.edges.len(), 2);
        for edge in result.final_output.edges.values() {
            assert_eq!(edge.edge_type, "knows");
        }
        assert_eq!(result.steps.len(), 3);
        assert_eq!(result.steps[0].step_type, "query");
        assert_eq!(result.steps[1].step_type, "rule");
        assert_eq!(result.steps[2].step_type, "transform:filter_edges_by_type");
    }

    // ── Test 9: Filter nodes transform ────────────────────────────────────

    #[test]
    fn test_filter_nodes_transform() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::TransformStep(PureTransform {
                    name: "filter_nodes_by_type".into(),
                    parameters: BTreeMap::from([("types".into(), "organization".into())]),
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        assert_eq!(result.final_output.nodes.len(), 1);
        assert!(result.final_output.nodes.contains_key(&3));
        // Edge 11 (from 1 to 3) should be excluded since node 1 is filtered out
        assert!(result.final_output.edges.is_empty());
    }

    #[test]
    fn test_filter_edges_transform() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::TransformStep(PureTransform {
                    name: "filter_edges_by_type".into(),
                    parameters: BTreeMap::from([("types".into(), "knows".into())]),
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        assert_eq!(result.final_output.nodes.len(), 4);
        assert_eq!(result.final_output.edges.len(), 2);
        for edge in result.final_output.edges.values() {
            assert_eq!(edge.edge_type, "knows");
        }
    }

    // ── Test 10: Serialization determinism ─────────────────────────────────

    #[test]
    fn test_pipeline_spec_serialization_determinism() {
        let spec = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                PipelineStep::RuleStep(BTreeSet::from([
                    RuleSpec {
                        id: "r1".into(), tag: "t".into(),
                        node_type_match: Some("person".into()),
                        node_property_matches: Vec::new(),
                        specific_node_id: None,
                        edge_type_match: None,
                        edge_property_matches: Vec::new(),
                        specific_edge_id: None,
                    },
                ])),
                PipelineStep::TransformStep(PureTransform {
                    name: "identity".into(),
                    parameters: BTreeMap::new(),
                }),
            ],
        };

        let s1 = spec.to_deterministic_string();
        let s2 = spec.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_pipeline_result_serialization_determinism() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        let s1 = result.to_deterministic_string();
        let s2 = result.to_deterministic_string();
        assert_eq!(s1, s2);
    }

    // ── Test 11: Serialization roundtrip ──────────────────────────────────

    #[test]
    fn test_pipeline_spec_serialization_roundtrip() {
        let specs = vec![
            PipelineSpec { steps: Vec::new() },
            PipelineSpec {
                steps: vec![
                    PipelineStep::QueryStep(QuerySpec {
                        node_type_filter: Some(vec!["person".into()]),
                        edge_type_filter: None,
                        property_filters: Vec::new(),
                        traversal_depth: None,
                        source_node_ids: None,
                        target_node_ids: None,
                    }),
                ],
            },
            PipelineSpec {
                steps: vec![
                    PipelineStep::RuleStep(BTreeSet::from([
                        RuleSpec {
                            id: "r1".into(), tag: "t".into(),
                            node_type_match: Some("person".into()),
                            node_property_matches: Vec::new(),
                            specific_node_id: None,
                            edge_type_match: Some("knows".into()),
                            edge_property_matches: Vec::new(),
                            specific_edge_id: None,
                        },
                    ])),
                    PipelineStep::TransformStep(PureTransform {
                        name: "identity".into(),
                        parameters: BTreeMap::new(),
                    }),
                ],
            },
        ];

        for spec in &specs {
            let s = spec.to_deterministic_string();
            let back = PipelineSpec::from_deterministic_string(&s).unwrap();
            assert_eq!(*spec, back, "Failed roundtrip for: {}", s);
        }
    }

    #[test]
    fn test_pipeline_result_serialization_roundtrip() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                PipelineStep::RuleStep(BTreeSet::from([
                    RuleSpec {
                        id: "r1".into(), tag: "t".into(),
                        node_type_match: Some("person".into()),
                        node_property_matches: Vec::new(),
                        specific_node_id: None,
                        edge_type_match: None,
                        edge_property_matches: Vec::new(),
                        specific_edge_id: None,
                    },
                ])),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        let s = result.to_deterministic_string();
        let back = PipelineResult::from_deterministic_string(&s).unwrap();
        assert_eq!(result, back);
    }

    #[test]
    fn test_step_result_serialization_roundtrip() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let query_result = query(&graph, &onto, &QuerySpec {
            node_type_filter: Some(vec!["person".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        });

        let sr = StepResult {
            index: 0,
            step_type: "query".to_string(),
            output: query_result,
        };

        let s = sr.to_deterministic_string();
        let back = StepResult::from_deterministic_string(&s).unwrap();
        assert_eq!(sr, back);
    }

    // ── Test 12: Stability across 100 runs ────────────────────────────────

    #[test]
    fn test_stability_100_runs() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();

        let pipelines = vec![
            PipelineSpec { steps: Vec::new() },
            PipelineSpec {
                steps: vec![
                    PipelineStep::QueryStep(QuerySpec {
                        node_type_filter: Some(vec!["person".into()]),
                        edge_type_filter: None,
                        property_filters: Vec::new(),
                        traversal_depth: None,
                        source_node_ids: None,
                        target_node_ids: None,
                    }),
                ],
            },
            PipelineSpec {
                steps: vec![
                    PipelineStep::QueryStep(QuerySpec {
                        node_type_filter: Some(vec!["person".into()]),
                        edge_type_filter: None,
                        property_filters: Vec::new(),
                        traversal_depth: None,
                        source_node_ids: None,
                        target_node_ids: None,
                    }),
                    PipelineStep::RuleStep(BTreeSet::from([
                        RuleSpec {
                            id: "r1".into(), tag: "t".into(),
                            node_type_match: Some("person".into()),
                            node_property_matches: Vec::new(),
                            specific_node_id: None,
                            edge_type_match: None,
                            edge_property_matches: Vec::new(),
                            specific_edge_id: None,
                        },
                    ])),
                    PipelineStep::TransformStep(PureTransform {
                        name: "filter_edges_by_type".into(),
                        parameters: BTreeMap::from([("types".into(), "knows".into())]),
                    }),
                ],
            },
        ];

        for pipeline in &pipelines {
            let first = execute_pipeline(&graph, &onto, pipeline);
            let first_str = first.to_deterministic_string();
            for _ in 0..100 {
                let result = execute_pipeline(&graph, &onto, pipeline);
                assert_eq!(first_str, result.to_deterministic_string());
            }
        }
    }

    // ── Test 13: Input immutability ───────────────────────────────────────

    #[test]
    fn test_graph_immutability() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let graph_copy = graph.clone();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
            ],
        };

        let _result = execute_pipeline(&graph, &onto, &pipeline);
        assert_eq!(graph, graph_copy);
    }

    // ── Test 14: Step isolation ───────────────────────────────────────────

    #[test]
    fn test_step_isolation() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["person".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
                PipelineStep::QueryStep(QuerySpec {
                    node_type_filter: Some(vec!["organization".into()]),
                    edge_type_filter: None,
                    property_filters: Vec::new(),
                    traversal_depth: None,
                    source_node_ids: None,
                    target_node_ids: None,
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        // First step should have person nodes
        assert_eq!(result.steps[0].output.nodes.len(), 3);
        // Second step overrides with organization nodes
        assert_eq!(result.steps[1].output.nodes.len(), 1);
        // Final output is the last step's output
        assert_eq!(result.final_output.nodes.len(), 1);
        assert!(result.final_output.nodes.contains_key(&3));
    }

    // ── Test 15: Unknown transform defaults to identity ───────────────────

    #[test]
    fn test_unknown_transform_defaults_to_identity() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::TransformStep(PureTransform {
                    name: "nonexistent_transform".into(),
                    parameters: BTreeMap::new(),
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        // Should behave as identity
        assert_eq!(result.final_output.nodes.len(), 4);
        assert_eq!(result.final_output.edges.len(), 3);
    }

    // ── Test 16: Filter nodes with empty types ────────────────────────────

    #[test]
    fn test_filter_nodes_empty_types() {
        let graph = make_test_graph();
        let onto = make_empty_ontology();
        let pipeline = PipelineSpec {
            steps: vec![
                PipelineStep::TransformStep(PureTransform {
                    name: "filter_nodes_by_type".into(),
                    parameters: BTreeMap::from([("types".into(), "".into())]),
                }),
            ],
        };

        let result = execute_pipeline(&graph, &onto, &pipeline);
        // Empty types means no filtering -> identity
        assert_eq!(result.final_output.nodes.len(), 4);
    }
}
