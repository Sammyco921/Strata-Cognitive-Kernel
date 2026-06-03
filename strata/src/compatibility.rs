use crate::event::EventEnvelope;
use crate::version::SchemaVersion;

/// B7-C: Compatibility upgrade trait.
///
/// Rules:
/// - Old → New supported (forward upgrade)
/// - New → Old forbidden (no downgrade)
/// - Replay automatically upgrades historical envelopes before processing
pub trait EventUpgrader {
    /// Upgrade an envelope to the current schema version.
    /// If the envelope is already at the current version, returns it unchanged.
    /// If the envelope is from a future version, returns an error.
    fn upgrade(&self, envelope: EventEnvelope) -> Result<EventEnvelope, String>;
}

/// No-op upgrader for v1.0 — schema is frozen, no transformations needed.
/// Future upgraders will implement actual field migrations here.
pub struct V1NoopUpgrader;

impl EventUpgrader for V1NoopUpgrader {
    fn upgrade(&self, envelope: EventEnvelope) -> Result<EventEnvelope, String> {
        let current = SchemaVersion::default();

        if envelope.schema_version > current {
            return Err(format!(
                "cannot replay event from future schema version {}.{} (current: {}.{})",
                envelope.schema_version.major,
                envelope.schema_version.minor,
                current.major,
                current.minor,
            ));
        }

        // v1.0 → v1.0: no transformation needed
        Ok(envelope)
    }
}

/// Apply the upgrader to a sequence of envelopes.
pub fn upgrade_all<U: EventUpgrader>(upgrader: &U, envelopes: Vec<EventEnvelope>) -> Result<Vec<EventEnvelope>, String> {
    envelopes.into_iter().map(|e| upgrader.upgrade(e)).collect()
}
