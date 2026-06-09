use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiContract {
    pub name: String,
    pub version: String,
    pub fields: BTreeSet<String>,
    pub dependencies: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiRegistry {
    version: u64,
    #[serde(skip)]
    frozen: bool,
    contracts: BTreeMap<String, AbiContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AbiValidationReport {
    pub missing_fields: BTreeMap<String, BTreeSet<String>>,
    pub extra_fields: BTreeMap<String, BTreeSet<String>>,
    pub dependency_mismatches: BTreeMap<String, Vec<String>>,
    pub is_valid: bool,
}

impl AbiContract {
    pub fn new(
        name: &str,
        version: &str,
        fields: &[&str],
        dependencies: &[&str],
    ) -> Self {
        AbiContract {
            name: name.to_string(),
            version: version.to_string(),
            fields: fields.iter().map(|s| s.to_string()).collect(),
            dependencies: dependencies.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl AbiRegistry {
    pub fn new() -> Self {
        AbiRegistry {
            version: 1,
            frozen: false,
            contracts: BTreeMap::new(),
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn increment_version(&mut self) {
        self.version += 1;
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn register(&mut self, contract: AbiContract) {
        assert!(!self.frozen, "ABI registry is frozen and cannot be mutated at runtime");
        self.contracts.insert(contract.name.clone(), contract);
    }

    pub fn get(&self, name: &str) -> Option<&AbiContract> {
        self.contracts.get(name)
    }

    pub fn len(&self) -> usize {
        self.contracts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }

    pub fn contract_names(&self) -> BTreeSet<String> {
        self.contracts.keys().cloned().collect()
    }

    pub fn validate_all(&self) -> AbiValidationReport {
        let missing_fields: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let extra_fields: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut dependency_mismatches: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (name, contract) in &self.contracts {
            let mut missing_deps: Vec<String> = Vec::new();
            for dep in &contract.dependencies {
                if !self.contracts.contains_key(dep) {
                    missing_deps.push(dep.clone());
                }
            }
            if !missing_deps.is_empty() {
                dependency_mismatches.insert(name.clone(), missing_deps);
            }
        }

        let is_valid = missing_fields.is_empty()
            && extra_fields.is_empty()
            && dependency_mismatches.is_empty();

        AbiValidationReport {
            missing_fields,
            extra_fields,
            dependency_mismatches,
            is_valid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry_valid() {
        let registry = AbiRegistry::new();
        let report = registry.validate_all();
        assert!(report.is_valid);
    }

    #[test]
    fn test_register_and_retrieve() {
        let mut registry = AbiRegistry::new();
        let contract = AbiContract::new("TestType", "1.0.0", &["a", "b"], &[]);
        registry.register(contract);
        assert_eq!(registry.len(), 1);
        let retrieved = registry.get("TestType").unwrap();
        assert_eq!(retrieved.name, "TestType");
        assert!(retrieved.fields.contains("a"));
        assert!(retrieved.fields.contains("b"));
        assert!(!retrieved.fields.contains("c"));
    }

    #[test]
    fn test_dependency_mismatch_detected() {
        let mut registry = AbiRegistry::new();
        let contract = AbiContract::new("Child", "1.0.0", &["x"], &["Parent"]);
        registry.register(contract);
        let report = registry.validate_all();
        assert!(!report.is_valid);
        assert!(report.dependency_mismatches.contains_key("Child"));
    }

    #[test]
    fn test_all_dependencies_satisfied() {
        let mut registry = AbiRegistry::new();
        registry.register(AbiContract::new("Parent", "1.0.0", &["a"], &[]));
        registry.register(AbiContract::new("Child", "1.0.0", &["b"], &["Parent"]));
        let report = registry.validate_all();
        assert!(report.is_valid);
    }

    #[test]
    fn test_contract_names_ordered() {
        let mut registry = AbiRegistry::new();
        registry.register(AbiContract::new("Z", "1.0.0", &[], &[]));
        registry.register(AbiContract::new("A", "1.0.0", &[], &[]));
        registry.register(AbiContract::new("M", "1.0.0", &[], &[]));
        let names: Vec<String> = registry.contract_names().into_iter().collect();
        assert_eq!(names, vec!["A", "M", "Z"]);
    }

    #[test]
    fn test_duplicate_registration_overwrites() {
        let mut registry = AbiRegistry::new();
        registry.register(AbiContract::new("X", "1.0.0", &["old"], &[]));
        registry.register(AbiContract::new("X", "2.0.0", &["new"], &[]));
        let contract = registry.get("X").unwrap();
        assert_eq!(contract.version, "2.0.0");
        assert!(contract.fields.contains("new"));
        assert!(!contract.fields.contains("old"));
    }

    #[test]
    fn test_stability_100_runs() {
        let mut registry = AbiRegistry::new();
        registry.register(AbiContract::new("A", "1.0.0", &["f1", "f2"], &[]));
        registry.register(AbiContract::new("B", "1.0.0", &["f3"], &["A"]));
        let first = format!("{:?}", registry);
        for _ in 0..100 {
            let json = format!("{:?}", registry);
            assert_eq!(first, json);
        }
    }
}
