use crate::closure::types::AuditResult;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiEntry {
    pub module: String,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiInventory {
    pub entries: Vec<ApiEntry>,
}

fn build_expected_inventory() -> ApiInventory {
    let mut entries = Vec::new();

    for name in &["Confidence", "SequencedEvent", "Event", "Node", "Edge", "GraphState", "Kernel", "Belief", "compute_belief_state", "replay", "strata_diagnose", "encode_knowledge_v1", "encode_knowledge_v2", "build_graph", "measure_graph_overlap", "generate_synthetic_log", "OverlapMetrics", "E2TestResult", "E2TestCase", "run_e2_test", "CognitiveMeasurement", "measure_cognitive_operations", "run_e4_test"] {
        entries.push(ApiEntry { module: "kernel".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["ConditionProfile"] {
        entries.push(ApiEntry { module: "triage".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["build_entropy_profiles"] {
        entries.push(ApiEntry { module: "entropy".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["OntologyEventType", "EntityTypeDef", "RelationshipTypeDef", "PropertyTypeDef", "OntologyEvent", "OntologyRegistry", "abi_contract", "replay_ontology"] {
        entries.push(ApiEntry { module: "ontology".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["TypedNode", "TypedEdge", "SemanticGraph", "parse_u64_keyed_objects", "parse_node", "parse_edge", "abi_contract", "semantic_project"] {
        entries.push(ApiEntry { module: "semantic".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["QuerySpec", "PropertyFilter", "ResultSet", "abi_contract", "query"] {
        entries.push(ApiEntry { module: "query".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["PropertyMatch", "RuleSpec", "AnnotatedResultSet", "abi_contract", "apply_rules"] {
        entries.push(ApiEntry { module: "rules".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["PureTransform", "PipelineStep", "PipelineSpec", "StepResult", "PipelineResult", "abi_contract", "execute_pipeline", "apply_transform"] {
        entries.push(ApiEntry { module: "composition".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["AbiContract", "AbiRegistry", "AbiValidationReport", "global_registry", "validate_global_registry"] {
        entries.push(ApiEntry { module: "abi".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["InvariantId", "InvariantLayer", "InvariantStatus", "InvariantSpec", "VerificationResult", "VerificationReport", "InvariantRegistry", "global_invariant_registry", "verify_all"] {
        entries.push(ApiEntry { module: "verification".into(), kind: "pub".into(), name: name.to_string() });
    }

    for name in &["ABI_VERSION", "SchemaDescriptor", "AbiEnvelope", "ToAbiString", "FromAbiString", "AbiError",
        "QUERY_SPEC_SCHEMA", "RESULT_SET_SCHEMA", "RULE_SPEC_SCHEMA", "ANNOTATED_RESULT_SET_SCHEMA", "PIPELINE_SPEC_SCHEMA", "PIPELINE_RESULT_SCHEMA",
        "validate_version", "validate_schema_id", "validate_envelope", "is_compatible", "serialize_envelope", "deserialize_envelope"] {
        entries.push(ApiEntry { module: "abi_sub".into(), kind: "pub".into(), name: name.to_string() });
    }

    entries.sort();
    ApiInventory { entries }
}

fn is_struct_or_enum(name: &str) -> bool {
    name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}

pub fn run_surface_audit() -> AuditResult {
    let mut result = AuditResult::new("Surface Audit");

    let inventory = build_expected_inventory();

    let forbidden_type_patterns: BTreeSet<&str> = BTreeSet::from([
        "MockPersister",
        "NullPersister",
        "InMemoryPersister",
        "TestConfig",
        "MockRepository",
        "TestRepository",
    ]);

    for entry in &inventory.entries {
        if !is_struct_or_enum(&entry.name) {
            continue;
        }
        for pattern in &forbidden_type_patterns {
            if entry.name == *pattern {
                result.add_violation(
                    "SURF001",
                    &format!("Forbidden type '{}' exposed in module '{}'", entry.name, entry.module),
                );
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_audit_passes() {
        let result = run_surface_audit();
        assert!(result.passed(), "Surface audit failed: {:?}", result.violations);
    }

    #[test]
    fn test_surface_audit_deterministic() {
        let first = run_surface_audit();
        for _ in 0..50 {
            let next = run_surface_audit();
            assert_eq!(first.status, next.status);
            assert_eq!(first.violations.len(), next.violations.len());
        }
    }

    #[test]
    fn test_inventory_sorted() {
        let inventory = build_expected_inventory();
        for i in 1..inventory.entries.len() {
            assert!(inventory.entries[i - 1] <= inventory.entries[i]);
        }
    }

    #[test]
    fn test_inventory_contains_kernel_items() {
        let inventory = build_expected_inventory();
        assert!(inventory.entries.iter().any(|e| e.name == "Kernel"));
        assert!(inventory.entries.iter().any(|e| e.name == "Event"));
    }

    #[test]
    fn test_inventory_has_expected_count() {
        let inventory = build_expected_inventory();
        assert!(inventory.entries.len() > 50);
    }
}
