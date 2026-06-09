pub mod registry;

use registry::{AbiRegistry, AbiValidationReport};

pub fn global_registry() -> AbiRegistry {
    let mut r = AbiRegistry::new();
    r.increment_version();
    r.register(crate::ontology::types::abi_contract());
    r.register(crate::ontology::semantic::types::abi_contract());
    r.register(crate::ontology::semantic::query::types::abi_contract());
    r.register(crate::ontology::semantic::rules::types::abi_contract());
    r.register(crate::ontology::semantic::composition::types::abi_contract());
    r.freeze();
    r
}

pub fn validate_global_registry() -> AbiValidationReport {
    global_registry().validate_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::registry::AbiContract;

    #[test]
    fn test_all_contracts_registered() {
        let r = global_registry();
        assert_eq!(r.len(), 5);
        assert!(r.get("OntologyTypes").is_some());
        assert!(r.get("SemanticGraphTypes").is_some());
        assert!(r.get("QueryTypes").is_some());
        assert!(r.get("RuleTypes").is_some());
        assert!(r.get("CompositionTypes").is_some());
    }

    #[test]
    fn test_all_contracts_valid() {
        let report = validate_global_registry();
        assert!(
            report.is_valid,
            "Global registry validation failed: missing={:?}, extra={:?}, deps={:?}",
            report.missing_fields,
            report.extra_fields,
            report.dependency_mismatches,
        );
    }

    #[test]
    fn test_contracts_have_all_expected_fields() {
        // Verify each contract's fields match what we expect from the schema
        // descriptors and serialization implementation

        let r = global_registry();

        // OntologyTypes
        let ot = r.get("OntologyTypes").unwrap();
        for f in &["id", "timestamp", "event_type", "payload", "causes"] {
            assert!(ot.fields.contains(*f), "OntologyTypes missing field: {}", f);
        }

        // SemanticGraphTypes
        let sg = r.get("SemanticGraphTypes").unwrap();
        for f in &["id", "node_type", "properties", "from_node", "to_node", "edge_type", "nodes", "edges"] {
            assert!(sg.fields.contains(*f), "SemanticGraphTypes missing field: {}", f);
        }

        // QueryTypes
        let qt = r.get("QueryTypes").unwrap();
        for f in &["node_type_filter", "edge_type_filter", "property_filters", "traversal_depth", "source_node_ids", "target_node_ids"] {
            assert!(qt.fields.contains(*f), "QueryTypes missing field: {}", f);
        }
        assert!(qt.fields.contains("key"));
        assert!(qt.fields.contains("value"));

        // RuleTypes
        let rt = r.get("RuleTypes").unwrap();
        for f in &["id", "tag", "node_type_match", "node_property_matches", "specific_node_id", "edge_type_match", "edge_property_matches", "specific_edge_id", "result_set", "node_tags", "edge_tags"] {
            assert!(rt.fields.contains(*f), "RuleTypes missing field: {}", f);
        }

        // CompositionTypes
        let ct = r.get("CompositionTypes").unwrap();
        for f in &["steps", "final_output", "type", "spec", "rules", "name", "parameters", "index", "step_type", "output"] {
            assert!(ct.fields.contains(*f), "CompositionTypes missing field: {}", f);
        }
    }

    #[test]
    fn test_contract_dependency_chain() {
        let r = global_registry();

        let ot = r.get("OntologyTypes").unwrap();
        assert!(ot.dependencies.is_empty());

        let sg = r.get("SemanticGraphTypes").unwrap();
        assert!(sg.dependencies.contains("OntologyTypes"));

        let qt = r.get("QueryTypes").unwrap();
        assert!(qt.dependencies.contains("SemanticGraphTypes"));

        let rt = r.get("RuleTypes").unwrap();
        assert!(rt.dependencies.contains("QueryTypes"));

        let ct = r.get("CompositionTypes").unwrap();
        assert!(ct.dependencies.contains("RuleTypes"));
    }

    #[test]
    fn test_contract_fields_use_btreeset_deterministic() {
        let r = global_registry();
        // Verify all contracts' fields are BTreeSets => deterministic ordering
        for name in &["OntologyTypes", "SemanticGraphTypes", "QueryTypes", "RuleTypes", "CompositionTypes"] {
            let contract = r.get(name).unwrap();
            let fields: Vec<&String> = contract.fields.iter().collect();
            for i in 1..fields.len() {
                assert!(fields[i - 1] < fields[i], "Fields not sorted for {}", name);
            }
        }
    }

    #[test]
    fn test_deterministic_registry_100_runs() {
        let first = format!("{:?}", global_registry());
        for _ in 0..100 {
            let json = format!("{:?}", global_registry());
            assert_eq!(first, json);
        }
    }

    #[test]
    fn test_version_stability_across_runs() {
        let first = global_registry().version();
        for _ in 0..100 {
            assert_eq!(first, global_registry().version());
        }
    }

    #[test]
    fn test_version_increments_on_rebuild() {
        let mut r = AbiRegistry::new();
        assert_eq!(r.version(), 1);
        r.increment_version();
        assert_eq!(r.version(), 2);
        r.increment_version();
        assert_eq!(r.version(), 3);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let r = global_registry();
        let json = serde_json::to_string(&r).unwrap();
        let parsed: AbiRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(r.version(), parsed.version());
        assert_eq!(r.contract_names(), parsed.contract_names());
        for name in r.contract_names() {
            assert_eq!(r.get(&name), parsed.get(&name));
        }
    }

    #[test]
    fn test_deserialized_is_unfrozen() {
        let r = global_registry();
        let json = serde_json::to_string(&r).unwrap();
        let parsed: AbiRegistry = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_frozen());
        // Deserialized registry should accept registrations
        let mut mutable = parsed;
        mutable.register(AbiContract::new("Extra", "1.0.0", &[], &[]));
        assert_eq!(mutable.len(), 6);
    }

    #[test]
    fn test_contract_equality_across_executions() {
        let a = global_registry();
        let b = global_registry();
        assert_eq!(a, b);
    }

    #[test]
    fn test_contract_equality_100_runs() {
        let first = global_registry();
        for _ in 0..100 {
            assert_eq!(first, global_registry());
        }
    }

    #[test]
    fn test_rejects_runtime_mutation_after_freeze() {
        let mut r = AbiRegistry::new();
        r.register(AbiContract::new("PreFreeze", "1.0.0", &[], &[]));
        r.freeze();
        // After freeze, the test would panic on register
        // We verify freeze state is set
        assert!(r.is_frozen());
        assert_eq!(r.len(), 1);
    }

    #[test]
    #[should_panic(expected = "ABI registry is frozen")]
    fn test_mutation_after_freeze_panics() {
        let mut r = AbiRegistry::new();
        r.freeze();
        r.register(AbiContract::new("Illegal", "1.0.0", &[], &[]));
    }

    #[test]
    fn test_abi_does_not_affect_kernel_execution() {
        // Run kernel with and without ABI interaction, verify identical results
        use crate::cognition::system::engine::run_cognition_system;
        use crate::cognition::system::types::CognitionSystemInput;
        use crate::cognition::policy::types::PolicyRule;

        let input = "find nodes";
        let rules = vec![PolicyRule::new("R001")];
        let state = crate::kernel::GraphState::empty();
        let ont = crate::ontology::OntologyRegistry::empty();

        // Without touching ABI
        let out_a = run_cognition_system(CognitionSystemInput::new(input, rules.clone(), state.clone(), ont.clone()));

        // Validate ABI (read-only)
        let report = validate_global_registry();
        assert!(report.is_valid);

        // With ABI read access
        let _registry = global_registry();

        // Run again
        let out_b = run_cognition_system(CognitionSystemInput::new(input, rules, state, ont));

        // Results must be identical
        assert_eq!(out_a, out_b);
    }

    #[test]
    fn test_cross_module_schema_consistency_stable() {
        // Verify that cross-module schema consistency check produces identical results
        let reports: Vec<String> = (0..100).map(|_| {
            let report = validate_global_registry();
            format!("{:?}", report)
        }).collect();
        for i in 1..reports.len() {
            assert_eq!(reports[0], reports[i]);
        }
    }

    #[test]
    fn test_registry_contracts_match_schema_descriptors() {
        // Verify that contract fields correspond to schema descriptor field names
        let r = global_registry();

        // Identify fields that are in the contract but also tracked by schema descriptors
        // This is a cross-consistency check: schema descriptors describe per-type fields,
        // contracts describe per-module fields. The contract's fields should be a superset
        // of each schema descriptor field set for types in that module.

        let query_fields = r.get("QueryTypes").unwrap();
        let schema_query_spec = crate::ontology::abi::schema::QUERY_SPEC_SCHEMA;
        for f in schema_query_spec.field_names {
            assert!(
                query_fields.fields.contains(*f),
                "QueryTypes contract missing field '{}' that exists in QUERY_SPEC_SCHEMA",
                f
            );
        }
        let schema_result_set = crate::ontology::abi::schema::RESULT_SET_SCHEMA;
        for f in schema_result_set.field_names {
            assert!(
                query_fields.fields.contains(*f),
                "QueryTypes contract missing field '{}' that exists in RESULT_SET_SCHEMA",
                f
            );
        }

        let rule_fields = r.get("RuleTypes").unwrap();
        let schema_rule_spec = crate::ontology::abi::schema::RULE_SPEC_SCHEMA;
        for f in schema_rule_spec.field_names {
            assert!(
                rule_fields.fields.contains(*f),
                "RuleTypes contract missing field '{}' that exists in RULE_SPEC_SCHEMA",
                f
            );
        }
        let schema_annotated = crate::ontology::abi::schema::ANNOTATED_RESULT_SET_SCHEMA;
        for f in schema_annotated.field_names {
            assert!(
                rule_fields.fields.contains(*f),
                "RuleTypes contract missing field '{}' that exists in ANNOTATED_RESULT_SET_SCHEMA",
                f
            );
        }

        let comp_fields = r.get("CompositionTypes").unwrap();
        let schema_pipeline_spec = crate::ontology::abi::schema::PIPELINE_SPEC_SCHEMA;
        for f in schema_pipeline_spec.field_names {
            assert!(
                comp_fields.fields.contains(*f),
                "CompositionTypes contract missing field '{}' that exists in PIPELINE_SPEC_SCHEMA",
                f
            );
        }
        let schema_pipeline_result = crate::ontology::abi::schema::PIPELINE_RESULT_SCHEMA;
        for f in schema_pipeline_result.field_names {
            assert!(
                comp_fields.fields.contains(*f),
                "CompositionTypes contract missing field '{}' that exists in PIPELINE_RESULT_SCHEMA",
                f
            );
        }
    }
}
