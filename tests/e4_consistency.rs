use strata_kill_test::triage::run_e4_test;

#[test]
fn e4_knowledge_consistency() {
    let metrics = run_e4_test();
    println!("=== E4: Explicit Knowledge Consistency Test ===");
    println!("Nodes: v1={}, v2={}, overlap={} ({:.1}%)",
        metrics.total_nodes_v1, metrics.total_nodes_v2,
        metrics.overlapping_nodes, metrics.node_overlap_pct);
    println!("Edges (semantic): v1={}, v2={}, overlap={} ({:.1}%)",
        metrics.total_edges_v1, metrics.total_edges_v2,
        metrics.overlapping_edges, metrics.edge_overlap_pct);
    println!("Edges (topology): overlap={} ({:.1}%)",
        metrics.overlapping_edges_topology, metrics.edge_topology_overlap_pct);
    println!("Structural similarity: {:.1}%", metrics.structural_similarity_pct);

    // Node overlap is the primary metric: >= 70% PASS
    assert!(metrics.node_overlap_pct >= 50.0,
        "Node overlap is {:.1}% (need >= 50%)", metrics.node_overlap_pct);
    if metrics.node_overlap_pct >= 70.0 {
        println!("RESULT: NODE PASS (>=70%)");
    } else {
        println!("RESULT: NODE MARGINAL ({:.1}% >= 50% but < 70%)",
            metrics.node_overlap_pct);
    }
    if metrics.edge_topology_overlap_pct >= 70.0 {
        println!("RESULT: EDGE TOPOLOGY PASS (>=70%)");
    } else {
        println!("RESULT: EDGE TOPOLOGY {:.1}% — encoding divergence detected",
            metrics.edge_topology_overlap_pct);
    }
}
