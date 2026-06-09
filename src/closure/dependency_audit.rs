use crate::closure::types::AuditResult;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleNode {
    pub path: String,
    pub depends_on: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyGraph {
    pub modules: BTreeMap<String, ModuleNode>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        DependencyGraph {
            modules: BTreeMap::new(),
        }
    }

    pub fn add_module(&mut self, path: &str, depends_on: &[&str]) {
        let deps: BTreeSet<String> = depends_on.iter().map(|s| s.to_string()).collect();
        self.modules.insert(
            path.to_string(),
            ModuleNode {
                path: path.to_string(),
                depends_on: deps,
            },
        );
    }

    pub fn has_cycle(&self) -> Option<Vec<String>> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut stack: BTreeSet<String> = BTreeSet::new();
        let mut path: Vec<String> = Vec::new();

        for module in self.modules.keys() {
            if !visited.contains(module) {
                if let Some(cycle) = self.dfs_cycle(module, &mut visited, &mut stack, &mut path)
                {
                    return Some(cycle);
                }
            }
        }
        None
    }

    fn dfs_cycle(
        &self,
        current: &str,
        visited: &mut BTreeSet<String>,
        stack: &mut BTreeSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        visited.insert(current.to_string());
        stack.insert(current.to_string());
        path.push(current.to_string());

        if let Some(node) = self.modules.get(current) {
            for dep in &node.depends_on {
                if self.modules.contains_key(dep) {
                    if stack.contains(dep) {
                        let cycle_start = path.iter().position(|p| p == dep).unwrap_or(0);
                        let cycle: Vec<String> = path[cycle_start..].to_vec();
                        return Some(cycle);
                    }
                    if !visited.contains(dep) {
                        if let Some(cycle) =
                            self.dfs_cycle(dep, visited, stack, path)
                        {
                            return Some(cycle);
                        }
                    }
                }
            }
        }

        path.pop();
        stack.remove(current);
        None
    }
}

pub(crate) fn build_substrate_dependency_graph() -> DependencyGraph {
    let mut graph = DependencyGraph::new();

    graph.add_module("kernel", &[]);
    graph.add_module("triage", &["kernel"]);
    graph.add_module("entropy", &["kernel", "triage"]);

    graph.add_module("ontology_types", &[]);
    graph.add_module("ontology_replay", &["ontology_types"]);
    graph.add_module("ontology_semantic_types", &[]);
    graph.add_module("ontology_semantic_projection", &["kernel", "ontology_types", "ontology_semantic_types"]);
    graph.add_module("ontology_semantic_query_types", &["ontology_semantic_types"]);
    graph.add_module("ontology_semantic_query_engine", &["ontology_types", "ontology_semantic_types", "ontology_semantic_query_types"]);
    graph.add_module("ontology_semantic_rules_types", &["ontology_semantic_query_types"]);
    graph.add_module("ontology_semantic_rules_engine", &["ontology_semantic_types", "ontology_semantic_query_types", "ontology_semantic_rules_types"]);
    graph.add_module("ontology_semantic_composition_types", &["ontology_semantic_query_types", "ontology_semantic_rules_types"]);
    graph.add_module("ontology_semantic_composition_engine", &["ontology_types", "ontology_semantic_types", "ontology_semantic_query_types", "ontology_semantic_query_engine", "ontology_semantic_rules_engine", "ontology_semantic_composition_types"]);
    graph.add_module("ontology_abi_version", &[]);
    graph.add_module("ontology_abi_schema", &[]);
    graph.add_module("ontology_abi_compatibility", &["ontology_abi_version", "ontology_abi_schema", "ontology_semantic_query_types", "ontology_semantic_rules_types", "ontology_semantic_composition_types"]);

    graph.add_module("abi_registry", &["ontology_types", "ontology_semantic_types", "ontology_semantic_query_types", "ontology_semantic_rules_types", "ontology_semantic_composition_types"]);
    graph.add_module("abi_mod", &["abi_registry", "ontology_types", "ontology_semantic_types", "ontology_semantic_query_types", "ontology_semantic_rules_types", "ontology_semantic_composition_types"]);

    graph.add_module("verification_types", &[]);
    graph.add_module("verification_registry", &["verification_types"]);
    graph.add_module("verification_engine", &["kernel", "ontology_types", "ontology_replay", "ontology_semantic_types", "ontology_semantic_projection", "ontology_semantic_query_types", "ontology_semantic_query_engine", "ontology_semantic_rules_types", "ontology_semantic_rules_engine", "ontology_semantic_composition_types", "ontology_semantic_composition_engine", "ontology_abi_version", "ontology_abi_schema", "ontology_abi_compatibility", "abi_registry", "verification_types", "verification_registry"]);

    graph
}

pub fn run_dependency_audit() -> AuditResult {
    let mut result = AuditResult::new("Dependency Audit");

    let graph = build_substrate_dependency_graph();

    if let Some(cycle) = graph.has_cycle() {
        result.add_violation("DEP001", &format!("Dependency cycle detected: {}", cycle.join(" -> ")));
    }

    for (path, node) in &graph.modules {
        for dep in &node.depends_on {
            if dep.starts_with("test_") || dep.starts_with("tests_") {
                if !path.starts_with("test_") && !path.starts_with("tests_") {
                    result.add_violation(
                        "DEP002",
                        &format!("Production module '{}' imports test utility '{}'", path, dep),
                    );
                }
            }
        }
    }

    if graph.modules.contains_key("workflow") {
        if graph.modules["workflow"].depends_on.contains("test_utils") {
            result.add_violation("DEP003", "workflow depends on test_utils");
        }
    }
    if graph.modules.contains_key("api") && graph.modules.contains_key("bootstrap") {
        if graph.modules["api"].depends_on.contains("bootstrap")
            && graph.modules["bootstrap"].depends_on.contains("api")
        {
            result.add_violation("DEP004", "api and bootstrap have mutual dependency");
        }
    }
    if graph.modules.contains_key("persistence") && graph.modules.contains_key("projection") {
        if graph.modules["persistence"].depends_on.contains("projection")
            && graph.modules["projection"].depends_on.contains("persistence")
        {
            result.add_violation("DEP005", "persistence and projection have mutual dependency");
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_dependency_cycles() {
        let result = run_dependency_audit();
        assert!(result.passed(), "Unexpected cycle: {:?}", result.violations);
    }

    #[test]
    fn test_dependency_audit_deterministic() {
        let first = run_dependency_audit();
        for _ in 0..50 {
            let next = run_dependency_audit();
            assert_eq!(first.audit_name, next.audit_name);
            assert_eq!(first.status, next.status);
            assert_eq!(first.violations.len(), next.violations.len());
        }
    }

    #[test]
    fn test_graph_no_cycle() {
        let graph = build_substrate_dependency_graph();
        assert!(graph.has_cycle().is_none());
    }

    #[test]
    fn test_graph_detects_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_module("a", &["b"]);
        graph.add_module("b", &["c"]);
        graph.add_module("c", &["a"]);
        assert!(graph.has_cycle().is_some());
    }

    #[test]
    fn test_workflow_test_utils_absent() {
        assert!(!build_substrate_dependency_graph().modules.contains_key("workflow"));
    }

    #[test]
    fn test_api_bootstrap_absent() {
        let g = build_substrate_dependency_graph();
        assert!(!g.modules.contains_key("api"));
        assert!(!g.modules.contains_key("bootstrap"));
    }
}
