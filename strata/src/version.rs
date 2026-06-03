use serde::{Deserialize, Serialize};
use std::fmt;

/// Kernel version — represents the version of the Strata kernel contract.
/// Serialized in snapshots and event envelopes for compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KernelVersion {
    pub major: u16,
    pub minor: u16,
}

impl KernelVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        KernelVersion { major, minor }
    }
}

impl fmt::Display for KernelVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Default: Kernel v1.0 — initial frozen contract.
impl Default for KernelVersion {
    fn default() -> Self {
        KernelVersion { major: 1, minor: 0 }
    }
}

/// Schema version — represents the version of the event schema in the log.
/// Stored in every EventEnvelope to track schema evolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
}

impl SchemaVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        SchemaVersion { major, minor }
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Default: Schema v1.0 — initial frozen schema.
impl Default for SchemaVersion {
    fn default() -> Self {
        SchemaVersion { major: 1, minor: 0 }
    }
}

pub const CURRENT_KERNEL_VERSION: KernelVersion = KernelVersion::new(1, 0);
pub const CURRENT_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);
