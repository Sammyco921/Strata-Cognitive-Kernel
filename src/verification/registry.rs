use std::collections::BTreeMap;
use crate::verification::types::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantRegistry {
    invariants: BTreeMap<InvariantId, InvariantSpec>,
}

impl InvariantRegistry {
    pub fn new() -> Self {
        InvariantRegistry {
            invariants: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, spec: InvariantSpec) -> Result<(), String> {
        let id = spec.id.clone();
        if self.invariants.contains_key(&id) {
            return Err(format!("Duplicate invariant ID: {}", id.0));
        }
        self.invariants.insert(id, spec);
        Ok(())
    }

    pub fn get(&self, id: &InvariantId) -> Option<&InvariantSpec> {
        self.invariants.get(id)
    }

    pub fn all(&self) -> Vec<&InvariantSpec> {
        self.invariants.values().collect()
    }

    pub fn verification_targets(&self) -> Vec<InvariantId> {
        self.invariants.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.invariants.len()
    }

    pub fn is_empty(&self) -> bool {
        self.invariants.is_empty()
    }

    pub fn by_layer(&self, layer: &InvariantLayer) -> Vec<&InvariantSpec> {
        self.invariants.values().filter(|s| s.layer == *layer).collect()
    }
}

pub fn global_invariant_registry() -> InvariantRegistry {
    let mut r = InvariantRegistry::new();

    // Kernel invariants
    let _ = r.register(InvariantSpec {
        id: InvariantId("KERNEL_REPLAY_EQUIVALENCE".into()),
        layer: InvariantLayer::Kernel,
        description: "Replaying identical event streams produces identical GraphState".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("KERNEL_EVENT_ORDERING".into()),
        layer: InvariantLayer::Kernel,
        description: "Same ordered events produce identical projected state".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("KERNEL_SNAPSHOT_EQUIVALENCE".into()),
        layer: InvariantLayer::Kernel,
        description: "Snapshot-derived state equals replay-derived state".into(),
    });

    // Ontology invariants
    let _ = r.register(InvariantSpec {
        id: InvariantId("ONTOLOGY_REPLAY_IDEMPOTENCE".into()),
        layer: InvariantLayer::Ontology,
        description: "Replaying ontology events N times produces identical registry".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("ONTOLOGY_REGISTRY_DERIVATION".into()),
        layer: InvariantLayer::Ontology,
        description: "Registry from replay equals registry from iterative apply_event".into(),
    });

    // Semantic invariants
    let _ = r.register(InvariantSpec {
        id: InvariantId("SEMANTIC_PROJECTION_PURITY".into()),
        layer: InvariantLayer::Semantic,
        description: "semantic_project() repeated N times returns identical graph".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("SEMANTIC_QUERY_DETERMINISM".into()),
        layer: InvariantLayer::Semantic,
        description: "Same graph + same QuerySpec returns identical ResultSet".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("SEMANTIC_RULE_DETERMINISM".into()),
        layer: InvariantLayer::Semantic,
        description: "Same graph + same rules returns identical AnnotatedResultSet".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("SEMANTIC_PIPELINE_DETERMINISM".into()),
        layer: InvariantLayer::Semantic,
        description: "Same graph + same PipelineSpec returns identical PipelineResult".into(),
    });

    // ABI invariants
    let _ = r.register(InvariantSpec {
        id: InvariantId("ABI_CONTRACT_CONSISTENCY".into()),
        layer: InvariantLayer::Abi,
        description: "Global ABI contract registry validation passes".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("ABI_SCHEMA_CONSISTENCY".into()),
        layer: InvariantLayer::Abi,
        description: "Schema descriptors match ABI contract fields".into(),
    });
    let _ = r.register(InvariantSpec {
        id: InvariantId("ABI_SERIALIZATION_CONSISTENCY".into()),
        layer: InvariantLayer::Abi,
        description: "Deterministic serialization repeated yields identical output".into(),
    });

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_contains_all_invariants() {
        let r = global_invariant_registry();
        assert_eq!(r.len(), 12);
        for id in &[
            "KERNEL_REPLAY_EQUIVALENCE",
            "KERNEL_EVENT_ORDERING",
            "KERNEL_SNAPSHOT_EQUIVALENCE",
            "ONTOLOGY_REPLAY_IDEMPOTENCE",
            "ONTOLOGY_REGISTRY_DERIVATION",
            "SEMANTIC_PROJECTION_PURITY",
            "SEMANTIC_QUERY_DETERMINISM",
            "SEMANTIC_RULE_DETERMINISM",
            "SEMANTIC_PIPELINE_DETERMINISM",
            "ABI_CONTRACT_CONSISTENCY",
            "ABI_SCHEMA_CONSISTENCY",
            "ABI_SERIALIZATION_CONSISTENCY",
        ] {
            assert!(r.get(&InvariantId(id.to_string())).is_some(), "Missing invariant: {}", id);
        }
    }

    #[test]
    fn test_duplicate_id_rejected() {
        let mut r = InvariantRegistry::new();
        let spec = InvariantSpec {
            id: InvariantId("DUP".into()),
            layer: InvariantLayer::Kernel,
            description: "first".into(),
        };
        assert!(r.register(spec).is_ok());
        let spec2 = InvariantSpec {
            id: InvariantId("DUP".into()),
            layer: InvariantLayer::Kernel,
            description: "second".into(),
        };
        assert!(r.register(spec2).is_err());
    }

    #[test]
    fn test_verification_targets_deterministic() {
        let r = global_invariant_registry();
        let targets = r.verification_targets();
        for i in 1..targets.len() {
            assert!(targets[i - 1] < targets[i], "Targets not sorted");
        }
    }

    #[test]
    fn test_by_layer() {
        let r = global_invariant_registry();
        let kernel = r.by_layer(&InvariantLayer::Kernel);
        assert_eq!(kernel.len(), 3);
        let ontology = r.by_layer(&InvariantLayer::Ontology);
        assert_eq!(ontology.len(), 2);
        let semantic = r.by_layer(&InvariantLayer::Semantic);
        assert_eq!(semantic.len(), 4);
        let abi = r.by_layer(&InvariantLayer::Abi);
        assert_eq!(abi.len(), 3);
    }

    #[test]
    fn test_stability_100_runs() {
        let first = format!("{:?}", global_invariant_registry());
        for _ in 0..100 {
            assert_eq!(first, format!("{:?}", global_invariant_registry()));
        }
    }
}
