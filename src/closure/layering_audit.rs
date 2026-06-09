use crate::closure::dependency_audit::build_substrate_dependency_graph;
use crate::closure::types::AuditResult;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LayerAssignment {
    pub module_path: String,
    pub layer: u32,
    pub layer_name: String,
}

fn build_layer_map() -> BTreeMap<String, LayerAssignment> {
    let mut map = BTreeMap::new();

    let assignments: Vec<(&str, u32, &str)> = vec![
        ("kernel", 0, "Kernel"),
        ("triage", 0, "Kernel"),
        ("entropy", 0, "Kernel"),
        ("ontology_types", 1, "Ontology"),
        ("ontology_replay", 1, "Ontology"),
        ("ontology_semantic_types", 2, "Semantic"),
        ("ontology_semantic_projection", 2, "Semantic"),
        ("ontology_semantic_query_types", 2, "Semantic"),
        ("ontology_semantic_query_engine", 2, "Semantic"),
        ("ontology_semantic_rules_types", 2, "Semantic"),
        ("ontology_semantic_rules_engine", 2, "Semantic"),
        ("ontology_semantic_composition_types", 2, "Semantic"),
        ("ontology_semantic_composition_engine", 2, "Semantic"),
        ("ontology_abi_version", 3, "ABI"),
        ("ontology_abi_schema", 3, "ABI"),
        ("ontology_abi_compatibility", 3, "ABI"),
        ("abi_registry", 3, "ABI"),
        ("abi_mod", 3, "ABI"),
        ("verification_types", 4, "Verification"),
        ("verification_registry", 4, "Verification"),
        ("verification_engine", 4, "Verification"),
    ];

    for (path, layer, name) in assignments {
        map.insert(
            path.to_string(),
            LayerAssignment {
                module_path: path.to_string(),
                layer,
                layer_name: name.to_string(),
            },
        );
    }

    map
}

fn allowed_direction(from_layer: u32, to_layer: u32) -> bool {
    to_layer <= from_layer
}

pub fn run_layering_audit() -> AuditResult {
    let mut result = AuditResult::new("Layering Audit");

    let layer_map = build_layer_map();
    let graph = build_substrate_dependency_graph();

    for (module_path, node) in &graph.modules {
        let from = match layer_map.get(module_path) {
            Some(a) => a,
            None => continue,
        };

        for dep in &node.depends_on {
            let to = match layer_map.get(dep) {
                Some(a) => a,
                None => continue,
            };

            if !allowed_direction(from.layer, to.layer) {
                result.add_violation(
                    "LAY001",
                    &format!(
                        "Upward dependency: '{}' (layer {}) depends on '{}' (layer {}): {} module must not depend on {} module",
                        module_path, from.layer_name, dep, to.layer_name, from.layer_name, to.layer_name
                    ),
                );
            }
        }
    }

    if layer_map.contains_key("ontology_types") {
        let onto = &layer_map["ontology_types"];
        let prohibited_upper = ["ontology_semantic", "ontology_abi", "abi_", "verification_"];
        for (path, assignment) in &layer_map {
            if assignment.layer > onto.layer {
                for prefix in &prohibited_upper {
                    if path.starts_with(prefix) {
                        if graph.modules["ontology_types"].depends_on.contains(path) {
                            result.add_violation(
                                "LAY002",
                                &format!("Ontology depends on upper layer module '{}'", path),
                            );
                        }
                    }
                }
            }
        }
    }

    if layer_map.contains_key("kernel") {
        let kernel = &layer_map["kernel"];
        for (path, assignment) in &layer_map {
            if assignment.layer > kernel.layer {
                if graph.modules["kernel"].depends_on.contains(path) {
                    result.add_violation(
                        "LAY003",
                        &format!("Kernel depends on upper layer module '{}'", path),
                    );
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layering_audit_passes() {
        let result = run_layering_audit();
        assert!(result.passed(), "Layering audit failed: {:?}", result.violations);
    }

    #[test]
    fn test_layering_audit_deterministic() {
        let first = run_layering_audit();
        for _ in 0..50 {
            let next = run_layering_audit();
            assert_eq!(first.status, next.status);
            assert_eq!(first.violations.len(), next.violations.len());
        }
    }

    #[test]
    fn test_layer_assignments_complete() {
        let map = build_layer_map();
        assert!(map.contains_key("kernel"));
        assert!(map.contains_key("ontology_types"));
        assert!(map.contains_key("ontology_semantic_types"));
        assert!(map.contains_key("abi_registry"));
        assert!(map.contains_key("verification_types"));
    }

    #[test]
    fn test_kernel_layer_zero() {
        let map = build_layer_map();
        assert_eq!(map["kernel"].layer, 0);
    }

    #[test]
    fn test_allowed_direction() {
        assert!(allowed_direction(2, 0));
        assert!(allowed_direction(2, 1));
        assert!(allowed_direction(2, 2));
        assert!(!allowed_direction(0, 1));
        assert!(!allowed_direction(1, 2));
    }
}
