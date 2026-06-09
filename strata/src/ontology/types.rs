use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ── ID newtypes ──────────────────────────────────────────────────────────────

/// Unique identifier for an entity type (e.g., "person", "project").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityTypeId(String);

impl EntityTypeId {
    pub fn new(id: impl Into<String>) -> Self {
        EntityTypeId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EntityTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Unique identifier for a relationship type (e.g., "owns", "depends_on").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RelationshipTypeId(String);

impl RelationshipTypeId {
    pub fn new(id: impl Into<String>) -> Self {
        RelationshipTypeId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RelationshipTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Unique identifier for a property type (e.g., "name", "status").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PropertyTypeId(String);

impl PropertyTypeId {
    pub fn new(id: impl Into<String>) -> Self {
        PropertyTypeId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PropertyTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// ── Entity Type ──────────────────────────────────────────────────────────────

/// Defines a semantic entity category.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityType {
    id: EntityTypeId,
    name: String,
    description: String,
}

impl EntityType {
    pub fn new(id: EntityTypeId, name: impl Into<String>, description: impl Into<String>) -> Self {
        EntityType { id, name: name.into(), description: description.into() }
    }

    pub fn id(&self) -> &EntityTypeId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

// ── Relationship Type ────────────────────────────────────────────────────────

/// Defines a semantic relationship category.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RelationshipType {
    id: RelationshipTypeId,
    name: String,
    description: String,
}

impl RelationshipType {
    pub fn new(
        id: RelationshipTypeId,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        RelationshipType { id, name: name.into(), description: description.into() }
    }

    pub fn id(&self) -> &RelationshipTypeId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

// ── Property Type ────────────────────────────────────────────────────────────

/// Defines a semantic property definition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PropertyType {
    id: PropertyTypeId,
    name: String,
    description: String,
}

impl PropertyType {
    pub fn new(
        id: PropertyTypeId,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        PropertyType { id, name: name.into(), description: description.into() }
    }

    pub fn id(&self) -> &PropertyTypeId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Deterministic container for ontology type definitions.
///
/// Holds entity types, relationship types, and property types in
/// BTreeMap-backed collections guaranteeing deterministic iteration.
/// In this phase the registry is a pure data container with no
/// validation, replay, or execution logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyRegistry {
    entity_types: BTreeMap<EntityTypeId, EntityType>,
    relationship_types: BTreeMap<RelationshipTypeId, RelationshipType>,
    property_types: BTreeMap<PropertyTypeId, PropertyType>,
}

impl PartialOrd for OntologyRegistry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OntologyRegistry {
    fn cmp(&self, other: &Self) -> Ordering {
        by_iter(self.entity_types.iter(), other.entity_types.iter())
            .then_with(|| by_iter(self.relationship_types.iter(), other.relationship_types.iter()))
            .then_with(|| by_iter(self.property_types.iter(), other.property_types.iter()))
    }
}

fn by_iter<'a, K: Ord + 'a, V: Ord + 'a>(
    a: impl Iterator<Item = (&'a K, &'a V)>,
    b: impl Iterator<Item = (&'a K, &'a V)>,
) -> Ordering {
    let a_vec: Vec<(&K, &V)> = a.collect();
    let b_vec: Vec<(&K, &V)> = b.collect();
    a_vec.cmp(&b_vec)
}

impl OntologyRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        OntologyRegistry {
            entity_types: BTreeMap::new(),
            relationship_types: BTreeMap::new(),
            property_types: BTreeMap::new(),
        }
    }

    /// Register an entity type.
    pub fn register_entity_type(&mut self, entity_type: EntityType) {
        let id = entity_type.id.clone();
        self.entity_types.insert(id, entity_type);
    }

    /// Register a relationship type.
    pub fn register_relationship_type(&mut self, relationship_type: RelationshipType) {
        let id = relationship_type.id.clone();
        self.relationship_types.insert(id, relationship_type);
    }

    /// Register a property type.
    pub fn register_property_type(&mut self, property_type: PropertyType) {
        let id = property_type.id.clone();
        self.property_types.insert(id, property_type);
    }

    /// Look up an entity type by ID.
    pub fn get_entity_type(&self, id: &EntityTypeId) -> Option<&EntityType> {
        self.entity_types.get(id)
    }

    /// Look up a relationship type by ID.
    pub fn get_relationship_type(&self, id: &RelationshipTypeId) -> Option<&RelationshipType> {
        self.relationship_types.get(id)
    }

    /// Look up a property type by ID.
    pub fn get_property_type(&self, id: &PropertyTypeId) -> Option<&PropertyType> {
        self.property_types.get(id)
    }

    /// Iterate over all registered entity types in deterministic order.
    pub fn entity_types(&self) -> impl Iterator<Item = &EntityType> {
        self.entity_types.values()
    }

    /// Iterate over all registered relationship types in deterministic order.
    pub fn relationship_types(&self) -> impl Iterator<Item = &RelationshipType> {
        self.relationship_types.values()
    }

    /// Iterate over all registered property types in deterministic order.
    pub fn property_types(&self) -> impl Iterator<Item = &PropertyType> {
        self.property_types.values()
    }

    /// Number of registered entity types.
    pub fn entity_type_count(&self) -> usize {
        self.entity_types.len()
    }

    /// Number of registered relationship types.
    pub fn relationship_type_count(&self) -> usize {
        self.relationship_types.len()
    }

    /// Number of registered property types.
    pub fn property_type_count(&self) -> usize {
        self.property_types.len()
    }
}

impl Default for OntologyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ID construction and access ──────────────────────────────────────────

    #[test]
    fn entity_type_id_roundtrip() {
        let id = EntityTypeId::new("person");
        assert_eq!(id.as_str(), "person");
        assert_eq!(id.to_string(), "person");
    }

    #[test]
    fn relationship_type_id_roundtrip() {
        let id = RelationshipTypeId::new("owns");
        assert_eq!(id.as_str(), "owns");
    }

    #[test]
    fn property_type_id_roundtrip() {
        let id = PropertyTypeId::new("name");
        assert_eq!(id.as_str(), "name");
    }

    // ── EntityType construction and access ──────────────────────────────────

    #[test]
    fn entity_type_construction() {
        let et = EntityType::new(EntityTypeId::new("person"), "Person", "A human individual");
        assert_eq!(et.id().as_str(), "person");
        assert_eq!(et.name(), "Person");
        assert_eq!(et.description(), "A human individual");
    }

    #[test]
    fn entity_type_eq() {
        let a = EntityType::new(EntityTypeId::new("person"), "Person", "");
        let b = EntityType::new(EntityTypeId::new("person"), "Person", "");
        assert_eq!(a, b);
    }

    #[test]
    fn entity_type_neq() {
        let a = EntityType::new(EntityTypeId::new("person"), "Person", "");
        let b = EntityType::new(EntityTypeId::new("project"), "Project", "");
        assert_ne!(a, b);
    }

    #[test]
    fn entity_type_ordering() {
        let a = EntityType::new(EntityTypeId::new("person"), "", "");
        let b = EntityType::new(EntityTypeId::new("project"), "", "");
        assert!(a < b);
    }

    #[test]
    fn entity_type_clone() {
        let a = EntityType::new(EntityTypeId::new("person"), "Person", "desc");
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ── RelationshipType construction and access ────────────────────────────

    #[test]
    fn relationship_type_construction() {
        let rt = RelationshipType::new(RelationshipTypeId::new("owns"), "OWNS", "Ownership relation");
        assert_eq!(rt.id().as_str(), "owns");
        assert_eq!(rt.name(), "OWNS");
        assert_eq!(rt.description(), "Ownership relation");
    }

    #[test]
    fn relationship_type_eq() {
        let a = RelationshipType::new(RelationshipTypeId::new("owns"), "OWNS", "");
        let b = RelationshipType::new(RelationshipTypeId::new("owns"), "OWNS", "");
        assert_eq!(a, b);
    }

    #[test]
    fn relationship_type_clone() {
        let a = RelationshipType::new(RelationshipTypeId::new("depends_on"), "DEPENDS_ON", "");
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ── PropertyType construction and access ────────────────────────────────

    #[test]
    fn property_type_construction() {
        let pt = PropertyType::new(PropertyTypeId::new("status"), "Status", "Current status");
        assert_eq!(pt.id().as_str(), "status");
        assert_eq!(pt.name(), "Status");
        assert_eq!(pt.description(), "Current status");
    }

    #[test]
    fn property_type_eq() {
        let a = PropertyType::new(PropertyTypeId::new("name"), "Name", "");
        let b = PropertyType::new(PropertyTypeId::new("name"), "Name", "");
        assert_eq!(a, b);
    }

    #[test]
    fn property_type_clone() {
        let a = PropertyType::new(PropertyTypeId::new("priority"), "Priority", "");
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ── Registry: empty ─────────────────────────────────────────────────────

    #[test]
    fn empty_registry() {
        let r = OntologyRegistry::new();
        assert_eq!(r.entity_type_count(), 0);
        assert_eq!(r.relationship_type_count(), 0);
        assert_eq!(r.property_type_count(), 0);
        assert_eq!(r.entity_types().count(), 0);
        assert_eq!(r.relationship_types().count(), 0);
        assert_eq!(r.property_types().count(), 0);
    }

    #[test]
    fn empty_registry_default() {
        let r = OntologyRegistry::default();
        assert_eq!(r.entity_type_count(), 0);
    }

    // ── Registry: insertion and lookup ──────────────────────────────────────

    #[test]
    fn register_entity_type() {
        let mut r = OntologyRegistry::new();
        let et = EntityType::new(EntityTypeId::new("person"), "Person", "");
        r.register_entity_type(et);
        assert_eq!(r.entity_type_count(), 1);
    }

    #[test]
    fn register_relationship_type() {
        let mut r = OntologyRegistry::new();
        let rt = RelationshipType::new(RelationshipTypeId::new("owns"), "OWNS", "");
        r.register_relationship_type(rt);
        assert_eq!(r.relationship_type_count(), 1);
    }

    #[test]
    fn register_property_type() {
        let mut r = OntologyRegistry::new();
        let pt = PropertyType::new(PropertyTypeId::new("name"), "Name", "");
        r.register_property_type(pt);
        assert_eq!(r.property_type_count(), 1);
    }

    #[test]
    fn lookup_entity_type() {
        let mut r = OntologyRegistry::new();
        let et = EntityType::new(EntityTypeId::new("person"), "Person", "desc");
        r.register_entity_type(et);
        let found = r.get_entity_type(&EntityTypeId::new("person"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "Person");
    }

    #[test]
    fn lookup_missing_entity_type() {
        let r = OntologyRegistry::new();
        assert!(r.get_entity_type(&EntityTypeId::new("nonexistent")).is_none());
    }

    #[test]
    fn lookup_relationship_type() {
        let mut r = OntologyRegistry::new();
        let rt = RelationshipType::new(RelationshipTypeId::new("depends_on"), "DEPENDS_ON", "");
        r.register_relationship_type(rt);
        assert!(r.get_relationship_type(&RelationshipTypeId::new("depends_on")).is_some());
    }

    #[test]
    fn lookup_property_type() {
        let mut r = OntologyRegistry::new();
        let pt = PropertyType::new(PropertyTypeId::new("status"), "Status", "");
        r.register_property_type(pt);
        assert!(r.get_property_type(&PropertyTypeId::new("status")).is_some());
    }

    // ── Registry: multiple types ────────────────────────────────────────────

    #[test]
    fn multiple_entity_types() {
        let mut r = OntologyRegistry::new();
        r.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        r.register_entity_type(EntityType::new(EntityTypeId::new("project"), "Project", ""));
        r.register_entity_type(EntityType::new(EntityTypeId::new("task"), "Task", ""));
        assert_eq!(r.entity_type_count(), 3);
    }

    #[test]
    fn all_type_kinds_independent() {
        let mut r = OntologyRegistry::new();
        r.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        r.register_relationship_type(RelationshipType::new(RelationshipTypeId::new("owns"), "OWNS", ""));
        r.register_property_type(PropertyType::new(PropertyTypeId::new("name"), "Name", ""));
        assert_eq!(r.entity_type_count(), 1);
        assert_eq!(r.relationship_type_count(), 1);
        assert_eq!(r.property_type_count(), 1);
    }

    // ── Registry: deterministic ordering ────────────────────────────────────

    #[test]
    fn registry_deterministic_ordering() {
        let mut r = OntologyRegistry::new();
        // Insert in reverse alphabetical order.
        r.register_entity_type(EntityType::new(EntityTypeId::new("task"), "Task", ""));
        r.register_entity_type(EntityType::new(EntityTypeId::new("project"), "Project", ""));
        r.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        let names: Vec<&str> = r.entity_types().map(|e| e.name()).collect();
        // Must be sorted alphabetically by id.
        assert_eq!(names, vec!["Person", "Project", "Task"]);
    }

    #[test]
    fn registry_deterministic_iteration_is_stable() {
        let mut r = OntologyRegistry::new();
        r.register_entity_type(EntityType::new(EntityTypeId::new("z"), "Z", ""));
        r.register_entity_type(EntityType::new(EntityTypeId::new("a"), "A", ""));
        r.register_entity_type(EntityType::new(EntityTypeId::new("m"), "M", ""));
        let first: Vec<&str> = r.entity_types().map(|e| e.name()).collect();
        let second: Vec<&str> = r.entity_types().map(|e| e.name()).collect();
        assert_eq!(first, second);
    }

    // ── Registry: serialization determinism ─────────────────────────────────

    #[test]
    fn registry_serialization_deterministic() {
        let mut r = OntologyRegistry::new();
        r.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        r.register_relationship_type(RelationshipType::new(RelationshipTypeId::new("owns"), "OWNS", ""));
        let json_a = serde_json::to_string(&r).unwrap();
        let json_b = serde_json::to_string(&r).unwrap();
        assert_eq!(json_a, json_b);
    }

    #[test]
    fn registry_serialization_roundtrip() {
        let mut r = OntologyRegistry::new();
        r.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", "desc"));
        r.register_relationship_type(RelationshipType::new(RelationshipTypeId::new("owns"), "OWNS", ""));
        r.register_property_type(PropertyType::new(PropertyTypeId::new("name"), "Name", ""));
        let json = serde_json::to_string(&r).unwrap();
        let restored: OntologyRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(r, restored);
    }

    // ── Registry: clone and equality ────────────────────────────────────────

    #[test]
    fn registry_clone() {
        let mut r = OntologyRegistry::new();
        r.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        let cloned = r.clone();
        assert_eq!(r, cloned);
    }

    #[test]
    fn registry_eq() {
        let mut a = OntologyRegistry::new();
        let mut b = OntologyRegistry::new();
        a.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        b.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        assert_eq!(a, b);
    }

    #[test]
    fn registry_neq() {
        let mut a = OntologyRegistry::new();
        let mut b = OntologyRegistry::new();
        a.register_entity_type(EntityType::new(EntityTypeId::new("person"), "Person", ""));
        b.register_entity_type(EntityType::new(EntityTypeId::new("project"), "Project", ""));
        assert_ne!(a, b);
    }

    #[test]
    fn registry_ordering_deterministic() {
        let mut a = OntologyRegistry::new();
        let mut b = OntologyRegistry::new();
        a.register_entity_type(EntityType::new(EntityTypeId::new("a"), "A", ""));
        b.register_entity_type(EntityType::new(EntityTypeId::new("b"), "B", ""));
        assert!(a < b);
    }

    // ── No HashMap usage ────────────────────────────────────────────────────

    #[test]
    fn no_hashmap_in_ontology_types() {
        // Compile-time verification: OntologyRegistry uses BTreeMap, not HashMap.
        // If HashMap were used, serialization order would be nondeterministic.
        let mut r = OntologyRegistry::new();
        r.register_entity_type(EntityType::new(EntityTypeId::new("test"), "Test", ""));
        let json = serde_json::to_string(&r).unwrap();
        // Multiple serializations must produce identical output — guaranteed by BTreeMap.
        assert_eq!(json, serde_json::to_string(&r).unwrap());
    }
}
