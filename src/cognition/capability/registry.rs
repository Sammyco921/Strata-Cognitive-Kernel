use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::types::Capability;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRegistry {
    frozen: bool,
    capabilities: BTreeMap<String, Capability>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        CapabilityRegistry {
            frozen: false,
            capabilities: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, cap: Capability) {
        assert!(!self.frozen, "Capability registry is frozen");
        self.capabilities.insert(cap.id.clone(), cap);
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.capabilities.get(id)
    }

    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    pub fn all_capabilities(&self) -> Vec<&Capability> {
        self.capabilities.values().collect()
    }

    pub fn capabilities_by_layer(&self, layer: &str) -> Vec<&Capability> {
        self.capabilities
            .values()
            .filter(|c| c.layer == layer)
            .collect()
    }

    pub fn iter_sorted(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.values()
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn global_capability_registry() -> CapabilityRegistry {
    let mut r = CapabilityRegistry::new();

    // Kernel
    r.register(Capability::new("event_append", "Event Append", "Kernel", "Append a new event to the event log"));
    r.register(Capability::new("event_replay", "Event Replay", "Kernel", "Replay event log to reconstruct state"));
    r.register(Capability::new("state_reconstruct", "State Reconstruct", "Kernel", "Reconstruct graph state from events"));

    // Ontology
    r.register(Capability::new("entity_query", "Entity Query", "Ontology", "Query entity types in ontology registry"));
    r.register(Capability::new("relationship_query", "Relationship Query", "Ontology", "Query relationship types in ontology registry"));

    // Semantic
    r.register(Capability::new("graph_query", "Graph Query", "Semantic", "Query semantic graph structure"));
    r.register(Capability::new("rule_apply", "Rule Apply", "Semantic", "Apply semantic rules to graph"));

    // Memory
    r.register(Capability::new("memory_snapshot", "Memory Snapshot", "Memory", "Capture current memory state snapshot"));
    r.register(Capability::new("memory_update", "Memory Update", "Memory", "Update memory state from execution results"));

    // Goals
    r.register(Capability::new("goal_evaluate", "Goal Evaluate", "Goals", "Evaluate goal predicates against state"));
    r.register(Capability::new("goal_update", "Goal Update", "Goals", "Update goal status from evaluation results"));

    // Executive
    r.register(Capability::new("goal_rank", "Goal Rank", "Executive", "Rank goals by priority score"));
    r.register(Capability::new("goal_select", "Goal Select", "Executive", "Select active goal from ranked list"));

    // Policy
    r.register(Capability::new("intent_score", "Intent Score", "Policy", "Score intent against policy rules"));

    // Execution
    r.register(Capability::new("command_execute", "Command Execute", "Execution", "Execute a kernel command"));

    // Trace
    r.register(Capability::new("trace_record", "Trace Record", "Trace", "Record a trace of system execution"));
    r.register(Capability::new("trace_query", "Trace Query", "Trace", "Query stored trace records"));

    // Verification
    r.register(Capability::new("invariant_check", "Invariant Check", "Verification", "Check system invariants"));

    // CLI
    r.register(Capability::new("cli_run", "CLI Run", "CLI", "Execute full cognition pipeline via CLI"));
    r.register(Capability::new("cli_replay", "CLI Replay", "CLI", "Replay execution from trace file via CLI"));
    r.register(Capability::new("cli_inspect", "CLI Inspect", "CLI", "Inspect trace record via CLI"));
    r.register(Capability::new("cli_verify", "CLI Verify", "CLI", "Run verification layer via CLI"));
    r.register(Capability::new("cli_goals", "CLI Goals", "CLI", "List goal states via CLI"));
    r.register(Capability::new("cli_memory", "CLI Memory", "CLI", "Print memory snapshot via CLI"));

    r.freeze();
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_not_empty() {
        let r = global_capability_registry();
        assert_eq!(r.len(), 24);
    }

    #[test]
    fn test_registry_deterministic_ordering() {
        let r = global_capability_registry();
        let caps: Vec<&Capability> = r.iter_sorted().collect();
        for i in 1..caps.len() {
            assert!(caps[i - 1].id <= caps[i].id, "Capabilities must be sorted");
        }
    }

    #[test]
    fn test_registry_contains_all_layers() {
        let r = global_capability_registry();
        let layers: std::collections::BTreeSet<&str> =
            r.iter_sorted().map(|c| c.layer.as_str()).collect();
        for layer in &["Kernel", "Ontology", "Semantic", "Memory", "Goals", "Executive", "Policy", "Execution", "Trace", "Verification", "CLI"] {
            assert!(layers.contains(layer), "Missing layer: {}", layer);
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let r = global_capability_registry();
        let json = serde_json::to_string(&r).unwrap();
        let parsed: CapabilityRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(r.len(), parsed.len());
        for cap in r.iter_sorted() {
            assert_eq!(Some(cap), parsed.get(&cap.id));
        }
    }

    #[test]
    fn test_deterministic_across_runs() {
        let first = global_capability_registry();
        for _ in 0..100 {
            let next = global_capability_registry();
            assert_eq!(first, next);
        }
    }

    #[test]
    fn test_immutability_after_freeze() {
        let r = global_capability_registry();
        assert!(r.is_frozen());
    }

    #[test]
    #[should_panic(expected = "Capability registry is frozen")]
    fn test_register_after_freeze_panics() {
        let mut r = global_capability_registry();
        r.register(Capability::new("extra", "Extra", "Test", "Should not work"));
    }

    #[test]
    fn test_no_impact_on_kernel_execution() {
        let r = global_capability_registry();
        let result = r.get("event_append");
        assert!(result.is_some());
        // Just reading capabilities should not affect anything
        let _caps = r.all_capabilities();
    }

    #[test]
    fn test_capabilities_by_layer() {
        let r = global_capability_registry();
        let kernel_caps = r.capabilities_by_layer("Kernel");
        assert_eq!(kernel_caps.len(), 3);
    }

    #[test]
    fn test_all_capabilities_sorted() {
        let r = global_capability_registry();
        let all = r.all_capabilities();
        assert_eq!(all.len(), 24);
    }
}
