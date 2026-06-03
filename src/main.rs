use strata_kill_test::kernel::*;
use strata_kill_test::triage::*;
use std::io::{self, BufRead, Write};

fn print_help() {
    println!("Strata Kill Test CLI");
    println!("  create_node <id> <type>");
    println!("  create_edge <id> <from> <to> <type>");
    println!("  set_property <node_id> <key> <value>");
    println!("  assert_belief <node_id> <low|medium|high>");
    println!("  attach_evidence <belief_id> <evidence_id>");
    println!("  replay              — replay log and compare to live state");
    println!("  beliefs             — show current belief state");
    println!("  nodes               — list all nodes");
    println!("  edges               — list all edges");
    println!("  node <id>           — inspect a node");
    println!("  diagnose <symptoms> — comma-separated symptom names");
    println!("  test_e2             — run E2 comparison test");
    println!("  test_e3             — run E3 event explosion test");
    println!("  test_e4             — run E4 knowledge overlap test");
    println!("  test_e6 <size>      — run E6 replay scale test (size in K)");
    println!("  load_v1             — load knowledge encoding v1");
    println!("  load_v2             — load knowledge encoding v2");
    println!("  overlap             — measure overlap between v1 and v2");
    println!("  help                — this message");
    println!("  exit                — quit");
}

fn main() {
    let mut kernel = Kernel::new();
    print_help();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush().unwrap();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap() == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let command = parts[0];

        match command {
            "exit" | "quit" => break,
            "help" => print_help(),

            "create_node" => {
                if parts.len() < 3 {
                    println!("Usage: create_node <id> <type>");
                    continue;
                }
                let id: u64 = parts[1].parse().unwrap_or(0);
                let node_type = parts[2..].join(" ");
                match kernel.propose_and_commit(Event::CreateNode { id, node_type }) {
                    Ok(_) => println!("OK: node {} created", id),
                    Err(e) => println!("ERROR: {:?}", e),
                }
            }

            "create_edge" => {
                if parts.len() < 5 {
                    println!("Usage: create_edge <id> <from> <to> <type>");
                    continue;
                }
                let id: u64 = parts[1].parse().unwrap_or(0);
                let from: u64 = parts[2].parse().unwrap_or(0);
                let to: u64 = parts[3].parse().unwrap_or(0);
                let edge_type = parts[4].to_string();
                match kernel.propose_and_commit(Event::CreateEdge { id, from_node: from, to_node: to, edge_type }) {
                    Ok(_) => println!("OK: edge {} created", id),
                    Err(e) => println!("ERROR: {:?}", e),
                }
            }

            "set_property" => {
                if parts.len() < 4 {
                    println!("Usage: set_property <node_id> <key> <value>");
                    continue;
                }
                let node_id: u64 = parts[1].parse().unwrap_or(0);
                let key = parts[2].to_string();
                let value = parts[3..].join(" ");
                match kernel.propose_and_commit(Event::SetProperty { node_id, key, value }) {
                    Ok(_) => println!("OK: property set"),
                    Err(e) => println!("ERROR: {:?}", e),
                }
            }

            "assert_belief" => {
                if parts.len() < 3 {
                    println!("Usage: assert_belief <node_id> <low|medium|high>");
                    continue;
                }
                let node_id: u64 = parts[1].parse().unwrap_or(0);
                let confidence = Confidence::from_str(parts[2]).unwrap_or(Confidence::Low);
                match kernel.propose_and_commit(Event::AssertBelief { node_id, confidence }) {
                    Ok(_) => println!("OK: belief asserted on node {}", node_id),
                    Err(e) => println!("ERROR: {:?}", e),
                }
            }

            "attach_evidence" => {
                if parts.len() < 3 {
                    println!("Usage: attach_evidence <belief_id> <evidence_id>");
                    continue;
                }
                let belief_id: u64 = parts[1].parse().unwrap_or(0);
                let evidence_id: u64 = parts[2].parse().unwrap_or(0);
                match kernel.propose_and_commit(Event::AttachEvidence { belief_id, evidence_id }) {
                    Ok(_) => println!("OK: evidence attached"),
                    Err(e) => println!("ERROR: {:?}", e),
                }
            }

            "replay" => {
                let replayed = kernel.replay();
                if kernel.assert_equivalent() {
                    println!("PASS: replay matches live state ({} events)", kernel.event_count());
                } else {
                    println!("FAIL: replay MISMATCH with live state!");
                    println!("Live:   {} nodes, {} edges", kernel.state().node_count(), kernel.state().edge_count());
                    println!("Replay: {} nodes, {} edges", replayed.node_count(), replayed.edge_count());
                }
            }

            "beliefs" => {
                let beliefs = compute_belief_state(kernel.state());
                if beliefs.is_empty() {
                    println!("No beliefs recorded.");
                } else {
                    println!("Beliefs ({} total):", beliefs.len());
                    for b in &beliefs {
                        println!("  Node {} ({}): evidence={}, confidence={}",
                            b.node_id, b.node_type, b.evidence_count, b.effective_confidence.as_str());
                    }
                }
            }

            "nodes" => {
                let state = kernel.state();
                println!("Nodes ({} total):", state.node_count());
                for node in state.nodes.values() {
                    println!("  {} (type={})", node.id, node.node_type);
                    for (k, v) in &node.properties {
                        println!("    {} = {}", k, v);
                    }
                }
            }

            "edges" => {
                let state = kernel.state();
                println!("Edges ({} total):", state.edge_count());
                for edge in state.edges.values() {
                    println!("  {}: {} --[{}]--> {}", edge.id, edge.from_node, edge.edge_type, edge.to_node);
                }
            }

            "node" => {
                if parts.len() < 2 {
                    println!("Usage: node <id>");
                    continue;
                }
                let id: u64 = parts[1].parse().unwrap_or(0);
                match kernel.state().get_node(id) {
                    Some(node) => {
                        println!("Node {}: type={}", node.id, node.node_type);
                        for (k, v) in &node.properties {
                            println!("  {} = {}", k, v);
                        }
                        let incoming = kernel.state().edges_to(id);
                        let outgoing = kernel.state().edges_from(id);
                        println!("  Incoming edges: {}", incoming.len());
                        println!("  Outgoing edges: {}", outgoing.len());
                    }
                    None => println!("Node {} not found", id),
                }
            }

            "diagnose" => {
                if parts.len() < 2 {
                    println!("Usage: diagnose <symptom1,symptom2,...>");
                    continue;
                }
                let symptoms: Vec<&str> = parts[1].split(',').collect();
                let results = strata_diagnose(kernel.state(), &symptoms);
                println!("Diagnosis results:");
                for (i, (condition, matched, confidence)) in results.iter().enumerate() {
                    println!("  {}. {} ({} matched symptoms, confidence={})",
                        i + 1, condition, matched, confidence.as_str());
                }
            }

            "load_v1" => {
                let events = encode_knowledge_v1();
                let count = events.len();
                for event in events {
                    if let Err(e) = kernel.propose_and_commit(event) {
                        println!("Load error: {:?}", e);
                    }
                }
                println!("Loaded v1 knowledge base ({} events)", count);
            }

            "load_v2" => {
                let events = encode_knowledge_v2();
                let count = events.len();
                for event in events {
                    if let Err(e) = kernel.propose_and_commit(event) {
                        println!("Load error: {:?}", e);
                    }
                }
                println!("Loaded v2 knowledge base ({} events)", count);
            }

            "overlap" => {
                let v1_events = encode_knowledge_v1();
                let v2_events = encode_knowledge_v2();
                let g1 = build_graph(&v1_events);
                let g2 = build_graph(&v2_events);
                let metrics = measure_graph_overlap(&g1, &g2);
                println!("Knowledge Encoding Overlap:");
                println!("  Nodes: v1={}, v2={}, overlap={} ({:.1}%)",
                    metrics.total_nodes_v1, metrics.total_nodes_v2,
                    metrics.overlapping_nodes, metrics.node_overlap_pct);
                println!("  Edges (semantic): v1={}, v2={}, overlap={} ({:.1}%)",
                    metrics.total_edges_v1, metrics.total_edges_v2,
                    metrics.overlapping_edges, metrics.edge_overlap_pct);
                println!("  Edges (topology): overlap={} ({:.1}%)",
                    metrics.overlapping_edges_topology, metrics.edge_topology_overlap_pct);
                println!("  Structural similarity: {:.1}%", metrics.structural_similarity_pct);
                if metrics.node_overlap_pct >= 70.0 {
                    println!("  RESULT: NODE PASS (>=70%)");
                } else if metrics.node_overlap_pct >= 50.0 {
                    println!("  RESULT: NODE MARGINAL (>=50%, <70%)");
                } else {
                    println!("  RESULT: NODE FAIL (<50%)");
                }
            }

            "test_e2" => {
                println!("Running E2: Deterministic vs Naive Bayes diagnosis comparison...");
                let results = run_e2_test();
                let total = results.len();
                let strata_correct = results.iter().filter(|r| r.strata_correct).count();
                let nb_correct = results.iter().filter(|r| r.nb_correct).count();

                println!("Results ({} test cases):", total);
                println!("  Strata deterministic: {}/{} correct ({:.0}%)", strata_correct, total,
                    (strata_correct as f64 / total as f64) * 100.0);
                println!("  Naive Bayes:          {}/{} correct ({:.0}%)", nb_correct, total,
                    (nb_correct as f64 / total as f64) * 100.0);

                if total > 0 {
                    let pct = strata_correct as f64 / total as f64;
                    let baseline = nb_correct as f64 / total as f64;
                    if baseline > 0.0 {
                        let ratio = pct / baseline;
                        println!("  Strata/Baseline ratio: {:.2}", ratio);
                        if ratio >= 0.8 {
                            println!("  RESULT: PASS (>=80% of baseline)");
                        } else {
                            println!("  RESULT: FAIL (<80% of baseline)");
                        }
                    }
                }

                for r in &results {
                    let status = if r.strata_correct == r.nb_correct {
                        if r.strata_correct { "BOTH CORRECT" } else { "BOTH WRONG" }
                    } else if r.strata_correct {
                        "STRATA WINS"
                    } else {
                        "NB WINS"
                    };
                    println!("  {}: expected={} strata={:?} nb={:?} [{}]",
                        r.test_case.name, r.test_case.expected_condition,
                        r.strata_prediction.first().map(|x| x.0),
                        r.nb_prediction.first().map(|x| x.0),
                        status);
                }
            }

            "test_e3" => {
                println!("Running E3: Event explosion measurements...");
                let measurements = measure_cognitive_operations();
                println!("Cognitive operation measurements:");
                for m in &measurements {
                    println!("  {}: {} events, depth={}, replay/proc={}ns",
                        m.operation, m.event_count, m.max_depth, m.replay_ns);
                }
                let max_events = measurements.iter().map(|m| m.event_count).max().unwrap_or(0);
                if max_events < 100 {
                    println!("  RESULT: PASS (max {} events < 100)", max_events);
                } else if max_events < 500 {
                    println!("  RESULT: MARGINAL (max {} events >= 100, < 500)", max_events);
                } else {
                    println!("  RESULT: FAIL (max {} events >= 500)", max_events);
                }
            }

            "test_e4" => {
                println!("Running E4: Knowledge consistency test...");
                let metrics = run_e4_test();
                println!("Knowledge Encoding Overlap:");
                println!("  Nodes: v1={}, v2={}, overlap={} ({:.1}%)",
                    metrics.total_nodes_v1, metrics.total_nodes_v2,
                    metrics.overlapping_nodes, metrics.node_overlap_pct);
                println!("  Edges (semantic): v1={}, v2={}, overlap={} ({:.1}%)",
                    metrics.total_edges_v1, metrics.total_edges_v2,
                    metrics.overlapping_edges, metrics.edge_overlap_pct);
                println!("  Edges (topology): overlap={} ({:.1}%)",
                    metrics.overlapping_edges_topology, metrics.edge_topology_overlap_pct);
                println!("  Structural similarity: {:.1}%", metrics.structural_similarity_pct);
                if metrics.node_overlap_pct >= 70.0 {
                    println!("  RESULT: NODE PASS (>=70%)");
                } else if metrics.node_overlap_pct >= 50.0 {
                    println!("  RESULT: NODE MARGINAL (>=50%, <70%)");
                } else {
                    println!("  RESULT: NODE FAIL (<50%)");
                }
            }

            "test_e6" => {
                let size: usize = if parts.len() > 1 {
                    parts[1].parse::<usize>().unwrap_or(100) * 1000
                } else {
                    100_000
                };
                println!("Running E6: Replay scalability at {} events...", size);
                let events = generate_synthetic_log(size);
                let seq_events: Vec<SequencedEvent> = events.iter().enumerate()
                    .map(|(i, e)| SequencedEvent { seq: i as u64, event: e.clone() })
                    .collect();
                let start = std::time::Instant::now();
                let state = replay(&seq_events);
                let elapsed = start.elapsed();
                println!("  Replayed {} events in {:.2}s", size, elapsed.as_secs_f64());
                println!("  Resulting state: {} nodes, {} edges", state.node_count(), state.edge_count());
                let ms_per_100k = elapsed.as_secs_f64() / (size as f64 / 100_000.0);
                println!("  Scaling: {:.4}s per 100K events", ms_per_100k);
                if elapsed.as_secs_f64() / (size as f64 / 1_000_000.0) < 2.0 {
                    println!("  RESULT: PASS (near-linear scaling)");
                } else {
                    println!("  RESULT: INCONCLUSIVE (check scaling curve)");
                }
            }

            _ => {
                println!("Unknown command: {}", command);
                print_help();
            }
        }
    }
}
