use std::fmt;

/// B7 frozen error taxonomy — all kernel errors are one of these variants.
/// No stringly-typed public errors. Stable error codes for CLI and API consumers.
#[derive(Debug, Clone, PartialEq)]
pub enum KernelError {
    /// Event validation failed (e.g., duplicate node, missing dependency).
    ValidationError(String),
    /// A referenced node, edge, or event does not exist.
    ReferenceError(String),
    /// I/O or serialization error in persistence layer.
    PersistenceError(String),
    /// Replay failed due to corrupted or incompatible log.
    ReplayError(String),
    /// Schema version mismatch or upgrade failure.
    CompatibilityError(String),
    /// G₁ projection error (should not happen in normal operation).
    ProjectionError(String),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::ValidationError(msg) => write!(f, "ValidationError: {}", msg),
            KernelError::ReferenceError(msg) => write!(f, "ReferenceError: {}", msg),
            KernelError::PersistenceError(msg) => write!(f, "PersistenceError: {}", msg),
            KernelError::ReplayError(msg) => write!(f, "ReplayError: {}", msg),
            KernelError::CompatibilityError(msg) => write!(f, "CompatibilityError: {}", msg),
            KernelError::ProjectionError(msg) => write!(f, "ProjectionError: {}", msg),
        }
    }
}

impl std::error::Error for KernelError {}
