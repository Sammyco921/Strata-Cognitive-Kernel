use crate::kernel::event::{Event, EventType};
use crate::kernel::graph::GraphState;

/// Adversarial event generator for stress-testing causal drift.
pub struct AdversarialEventGenerator {
    counter: u64,
}

impl AdversarialEventGenerator {
    pub fn new() -> Self {
        AdversarialEventGenerator { counter: 0 }
    }

    fn next_id(&mut self) -> String {
        self.counter += 1;
        format!("evt-{}", self.counter)
    }

    /// Pattern F1: Rapid fire — interleaved Create/Delete of the same node ID.
    pub fn rapid_fire(&mut self, cycles: usize) -> Vec<Event> {
        let mut events = Vec::new();
        let base_id = "adversarial_target".to_string();
        for cycle in 0..cycles {
            let ts = (cycle * 2 + 1) as u64;
            events.push(Event::new(
                self.next_id(),
                ts,
                EventType::CreateNode,
                serde_json::json!({"id": base_id}),
            ));
            let ts2 = (cycle * 2 + 2) as u64;
            events.push(Event::new(
                self.next_id(),
                ts2,
                EventType::DeleteNode,
                serde_json::json!({"id": base_id}),
            ));
        }
        events
    }

    /// Pattern F2: Deep overwrite — many SetProperty on the same key.
    pub fn deep_overwrite(&mut self, layers: usize) -> Vec<Event> {
        let mut events = Vec::new();
        events.push(Event::new(
            self.next_id(),
            1,
            EventType::CreateNode,
            serde_json::json!({"id": "overwrite_target"}),
        ));
        for i in 0..layers {
            let ts = (i + 2) as u64;
            events.push(Event::new(
                self.next_id(),
                ts,
                EventType::SetProperty,
                serde_json::json!({"target_id": "overwrite_target", "key": "value", "value": i}),
            ));
        }
        events
    }

    /// Pattern F3: Edge explosion — F node, each with F edges.
    pub fn edge_explosion(&mut self, n: usize) -> Vec<Event> {
        let mut events = Vec::new();
        let node_ids: Vec<String> = (0..n).map(|i| format!("enode_{}", i)).collect();
        for (i, nid) in node_ids.iter().enumerate() {
            let ts = (i + 1) as u64;
            events.push(Event::new(
                self.next_id(),
                ts,
                EventType::CreateNode,
                serde_json::json!({"id": nid}),
            ));
        }
        let mut edge_ts = (n + 1) as u64;
        for from in &node_ids {
            for to in &node_ids {
                if from != to {
                    events.push(Event::new(
                        self.next_id(),
                        edge_ts,
                        EventType::CreateEdge,
                        serde_json::json!({"id": format!("edge_{}->{}", from, to), "from": from, "to": to, "type": "connects"}),
                    ));
                    edge_ts += 1;
                }
            }
        }
        events
    }

    /// Pattern F4: Null-op cascade — SetProperty on nonexistent node.
    pub fn null_op_cascade(&mut self, count: usize) -> Vec<Event> {
        let mut events = Vec::new();
        for i in 0..count {
            let ts = (i + 1) as u64;
            events.push(Event::new(
                self.next_id(),
                ts,
                EventType::SetProperty,
                serde_json::json!({"target_id": "nonexistent", "key": "x", "value": i}),
            ));
        }
        events
    }
}

/// Compare replay outputs between two runs and report mismatches.
pub struct ReplayComparator;

impl ReplayComparator {
    pub fn compare(a: &GraphState, b: &GraphState) -> Vec<String> {
        let mut diffs = Vec::new();
        for (id, node_a) in &a.nodes {
            match b.nodes.get(id) {
                Some(node_b) => {
                    if node_a.properties != node_b.properties {
                        diffs.push(format!(
                            "node '{}' properties differ: {:?} vs {:?}",
                            id, node_a.properties, node_b.properties
                        ));
                    }
                }
                None => {
                    diffs.push(format!("node '{}' missing in second state", id));
                }
            }
        }
        for (id, _) in &b.nodes {
            if !a.nodes.contains_key(id) {
                diffs.push(format!("node '{}' missing in first state", id));
            }
        }
        diffs
    }
}

/// Classify failure modes (F1–F5) from stress test output.
pub struct FailureClassifier;

impl FailureClassifier {
    pub fn classify(diffs: &[String]) -> Vec<String> {
        let mut labels = Vec::new();
        if diffs.is_empty() {
            labels.push("OK".to_string());
            return labels;
        }
        let all: String = diffs.join(" ");
        if all.contains("adversarial_target") {
            labels.push("F1: Rapid-fire create/delete cycle".to_string());
        }
        if all.contains("overwrite_target") {
            labels.push("F2: Deep overwrite chain".to_string());
        }
        if all.contains("enode_") || all.contains("edge_") {
            labels.push("F3: Edge explosion".to_string());
        }
        if all.contains("nonexistent") {
            labels.push("F4: Null-op cascade".to_string());
        }
        if labels.is_empty() {
            labels.push("F5: Unknown divergence".to_string());
        }
        labels
    }
}

/// Causal drift metrics for stress test evaluation.
pub struct CausalDriftMetrics {
    pub total_events: usize,
    pub total_causal_edges: usize,
    pub cis_score: f64,
}

/// Compute Causal Invariant Score (CIS) = number of matching edges / max edges.
pub fn compute_cis(full_edges: usize, pruned_edges: usize) -> f64 {
    let max = full_edges.max(pruned_edges);
    if max == 0 {
        return 1.0;
    }
    let matching = full_edges.min(pruned_edges);
    matching as f64 / max as f64
}

pub fn run_all_experiments() -> String {
    use crate::kernel::replay::replay;
    use crate::projection::causal::{project_default, replay_causal};

    let mut report = String::new();
    report.push_str("=== Strata Stress Test Report ===\n\n");
    let mut gen = AdversarialEventGenerator::new();

    // E-S1: Basic create + set property (baseline)
    {
        let mut g = AdversarialEventGenerator::new();
        let mut events = Vec::new();
        events.push(Event::new(g.next_id(), 1, EventType::CreateNode, serde_json::json!({"id": "s1"})));
        for i in 0..5 {
            events.push(Event::new(
                g.next_id(),
                (i + 2) as u64,
                EventType::SetProperty,
                serde_json::json!({"target_id": "s1", "key": "v", "value": i}),
            ));
        }
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-S1 (Basic): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    // E-S2: Two independent nodes
    {
        let mut g = AdversarialEventGenerator::new();
        let mut events = Vec::new();
        events.push(Event::new(g.next_id(), 1, EventType::CreateNode, serde_json::json!({"id": "a"})));
        events.push(Event::new(g.next_id(), 2, EventType::CreateNode, serde_json::json!({"id": "b"})));
        events.push(Event::new(g.next_id(), 3, EventType::SetProperty, serde_json::json!({"target_id": "a", "key": "x", "value": 1})));
        events.push(Event::new(g.next_id(), 4, EventType::SetProperty, serde_json::json!({"target_id": "b", "key": "y", "value": 2})));
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-S2 (Two nodes): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    // E-S3: Chain of edges (n=10)
    {
        let mut g = AdversarialEventGenerator::new();
        let mut events = Vec::new();
        for i in 0..10 {
            events.push(Event::new(
                g.next_id(),
                (i + 1) as u64,
                EventType::CreateNode,
                serde_json::json!({"id": format!("n{}", i)}),
            ));
        }
        for i in 0..9 {
            events.push(Event::new(
                g.next_id(),
                (11 + i) as u64,
                EventType::CreateEdge,
                serde_json::json!({"id": format!("e{}", i), "from": format!("n{}", i), "to": format!("n{}", i+1), "type": "chain"}),
            ));
        }
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-S3 (Chain 10): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    // E-S4: Delete-node cascade
    {
        let mut g = AdversarialEventGenerator::new();
        let mut events = Vec::new();
        events.push(Event::new(g.next_id(), 1, EventType::CreateNode, serde_json::json!({"id": "hub"})));
        for i in 0..5 {
            events.push(Event::new(
                g.next_id(),
                (i + 2) as u64,
                EventType::CreateNode,
                serde_json::json!({"id": format!("leaf_{}", i)}),
            ));
        }
        for i in 0..5 {
            events.push(Event::new(
                g.next_id(),
                (7 + i) as u64,
                EventType::CreateEdge,
                serde_json::json!({"id": format!("e{}", i), "from": "hub", "to": format!("leaf_{}", i), "type": "attached"}),
            ));
        }
        events.push(Event::new(g.next_id(), 13, EventType::DeleteNode, serde_json::json!({"id": "hub"})));
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-S4 (Delete cascade): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    // E-C1 through E-C4: adversarial patterns
    {
        let events = gen.rapid_fire(5);
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-C1 (Rapid fire): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    {
        let events = gen.deep_overwrite(10);
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-C2 (Deep overwrite): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    {
        let events = gen.edge_explosion(5);
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-C3 (Edge explosion): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    {
        let events = gen.null_op_cascade(10);
        let _state = replay(&events);
        let cg = replay_causal(&events);
        report.push_str(&format!("E-C4 (Null-op cascade): {} events -> {} causal edges\n", events.len(), cg.edges.len()));
        report.push_str(&format!("  CIS: {:.4}\n", compute_cis(cg.edges.len(), project_default(&events).edges.len())));
    }

    report
}
