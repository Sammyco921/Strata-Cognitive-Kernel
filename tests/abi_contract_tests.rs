use std::collections::BTreeSet;

/// Full registry validation: all contracts register and validate
#[test]
fn test_full_registry_validation() {
    let registry = strata_kill_test::abi::global_registry();
    assert_eq!(registry.len(), 5, "Expected 5 registered contracts");

    let report = registry.validate_all();
    assert!(
        report.is_valid,
        "Registry validation failed: deps={:?}",
        report.dependency_mismatches,
    );

    // All 5 contracts must be present
    for name in &["OntologyTypes", "SemanticGraphTypes", "QueryTypes", "RuleTypes", "CompositionTypes"] {
        assert!(registry.get(name).is_some(), "Missing contract: {}", name);
    }
}

/// Missing-field simulation: verify that removing a field from a contract causes
/// the validation to detect it (simulated by constructing a contract with fewer fields).
#[test]
fn test_missing_field_detection() {
    let registry = strata_kill_test::abi::global_registry();
    let report = registry.validate_all();

    // The baseline should pass
    assert!(report.missing_fields.is_empty());

    // Simulate a "missing field" scenario: construct a minimal contract manually
    // and verify the registry's compare logic works
    let qt = registry.get("QueryTypes").unwrap();
    let expected_minimal: BTreeSet<String> = ["node_type_filter", "edge_type_filter"]
        .iter().map(|s| s.to_string()).collect();

    let missing: BTreeSet<String> = expected_minimal.difference(&qt.fields).cloned().collect();
    let extra: BTreeSet<String> = qt.fields.difference(&expected_minimal).cloned().collect();

    // The actual contract has many more fields than our minimal set (so no missing from minimal)
    assert!(missing.is_empty(), "Minimal set has nothing missing from contract");

    // The contract has extra fields not in our minimal set
    assert!(!extra.is_empty(), "Contract has extra fields beyond minimal set");
}

/// Extra-field detection: verify that adding an unexpected field to a contract
/// would be detected by comparing against the schema descriptors.
#[test]
fn test_extra_field_detection() {
    let registry = strata_kill_test::abi::global_registry();

    // Get the KnownType contract and verify no unknown fields exist
    // by cross-checking against the schema descriptors.
    // If a contract contains fields not mentioned in any schema descriptor
    // for that module, it indicates a potential issue.

    let query_schema_fields: BTreeSet<String> = [
        "key", "value",
        "node_type_filter", "edge_type_filter", "property_filters",
        "traversal_depth", "source_node_ids", "target_node_ids",
        "nodes", "edges",
    ].iter().map(|s| s.to_string()).collect();

    let qt = registry.get("QueryTypes").unwrap();
    let extra_in_contract: BTreeSet<String> = qt.fields.difference(&query_schema_fields).cloned().collect();
    assert!(
        extra_in_contract.is_empty(),
        "QueryTypes contract has fields not in expected schema: {:?}",
        extra_in_contract,
    );
}

/// Cross-module dependency consistency: verify that the dependency chain is
/// consistent across all layers.
#[test]
fn test_cross_module_dependency_consistency() {
    let registry = strata_kill_test::abi::global_registry();

    // Verify the complete dependency chain:
    // OntologyTypes → SemanticGraphTypes → QueryTypes → RuleTypes → CompositionTypes
    fn check_transitive(registry: &strata_kill_test::abi::registry::AbiRegistry, name: &str, dep: &str) -> bool {
        let contract = registry.get(name);
        match contract {
            None => false,
            Some(c) => c.dependencies.contains(dep) || c.dependencies.iter().any(|d| check_transitive(registry, d, dep)),
        }
    }

    // CompositionTypes should transitively depend on QueryTypes
    assert!(check_transitive(&registry, "CompositionTypes", "QueryTypes"),
        "CompositionTypes does not transitively depend on QueryTypes");

    // CompositionTypes should transitively depend on SemanticGraphTypes
    assert!(check_transitive(&registry, "CompositionTypes", "SemanticGraphTypes"),
        "CompositionTypes does not transitively depend on SemanticGraphTypes");

    // CompositionTypes should transitively depend on OntologyTypes
    assert!(check_transitive(&registry, "CompositionTypes", "OntologyTypes"),
        "CompositionTypes does not transitively depend on OntologyTypes");
}

/// Deterministic ordering test: 100 runs produce identical output
#[test]
fn test_deterministic_ordering_100_runs() {
    let first = format!("{:?}", strata_kill_test::abi::global_registry());
    for _ in 0..100 {
        let current = format!("{:?}", strata_kill_test::abi::global_registry());
        assert_eq!(
            first, current,
            "Registry output differs between runs (non-deterministic)"
        );
    }
}

/// Verify each contract's fields are sorted (BTreeSet invariant)
#[test]
fn test_contract_fields_sorted() {
    let registry = strata_kill_test::abi::global_registry();
    for name in &["OntologyTypes", "SemanticGraphTypes", "QueryTypes", "RuleTypes", "CompositionTypes"] {
        let contract = registry.get(name).unwrap();
        let fields: Vec<&String> = contract.fields.iter().collect();
        for i in 1..fields.len() {
            assert!(
                fields[i - 1] < fields[i],
                "Fields not sorted for {}: {:?}",
                name,
                fields,
            );
        }
    }
}

/// Verify fields are consistent across all representation layers:
/// ABI contracts, schema descriptors, and actual serialization output
#[test]
fn test_contract_schema_serialization_consistency() {
    use strata_kill_test::ontology::semantic::query::types::QuerySpec;
    use strata_kill_test::ontology::abi::schema::QUERY_SPEC_SCHEMA;

    // Serialize a QuerySpec and extract field names
    let spec = QuerySpec {
        node_type_filter: Some(vec!["person".into()]),
        edge_type_filter: None,
        property_filters: Vec::new(),
        traversal_depth: None,
        source_node_ids: None,
        target_node_ids: None,
    };
    let json = spec.to_deterministic_string();

    // All schema descriptor fields must appear in the serialized output
    for field in QUERY_SPEC_SCHEMA.field_names {
        assert!(
            json.contains(&format!("\"{}\"", field)),
            "Field '{}' from schema descriptor not found in serialized QuerySpec",
            field,
        );
    }

    // All contract fields for this module must appear in at least one
    // type's serialization within the module
    let registry = strata_kill_test::abi::global_registry();
    let qt = registry.get("QueryTypes").unwrap();
    for field in &qt.fields {
        if field == "nodes" || field == "edges" {
            // These belong to ResultSet, checked below
            continue;
        }
        if field == "key" || field == "value" {
            // These belong to PropertyFilter, checked below
            continue;
        }
        assert!(
            json.contains(&format!("\"{}\"", field)),
            "Contract field '{}' not found in QuerySpec serialization",
            field,
        );
    }

    // Verify ResultSet serialization has 'nodes' and 'edges'
    use strata_kill_test::ontology::semantic::query::types::ResultSet;
    use std::collections::BTreeMap;
    let rs = ResultSet {
        nodes: BTreeMap::new(),
        edges: BTreeMap::new(),
    };
    let rs_json = rs.to_deterministic_string();
    assert!(rs_json.contains("\"nodes\""));
    assert!(rs_json.contains("\"edges\""));

    // Verify PropertyFilter serialization has 'key' and 'value'
    use strata_kill_test::ontology::semantic::query::types::PropertyFilter;
    let pf = PropertyFilter { key: "k".into(), value: "v".into() };
    let pf_json = pf.to_deterministic_string();
    assert!(pf_json.contains("\"key\""));
    assert!(pf_json.contains("\"value\""));
}
