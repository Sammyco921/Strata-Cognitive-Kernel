use crate::ontology::types::*;

pub fn replay_ontology(events: &[OntologyEvent]) -> OntologyRegistry {
    let mut registry = OntologyRegistry::empty();
    for event in events {
        registry.apply_event(event);
    }
    registry
}

pub fn replay_ontology_sorted(events: &mut [OntologyEvent]) -> OntologyRegistry {
    events.sort_by_key(|e| e.timestamp);
    replay_ontology(events)
}
