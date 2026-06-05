use crate::kernel::event::EventEnvelope;
use crate::kernel::version::SchemaVersion;

pub trait EventUpgrader {
    fn upgrade(&self, envelope: EventEnvelope) -> Result<EventEnvelope, String>;
}

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

        Ok(envelope)
    }
}

pub fn upgrade_all<U: EventUpgrader>(upgrader: &U, envelopes: Vec<EventEnvelope>) -> Result<Vec<EventEnvelope>, String> {
    envelopes.into_iter().map(|e| upgrader.upgrade(e)).collect()
}
