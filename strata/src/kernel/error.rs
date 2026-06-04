use std::fmt;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum KernelError {
    ValidationError(String),
    ReferenceError(String),
    PersistenceError(String),
    ReplayError(String),
    CompatibilityError(String),
    ProjectionError(String),
    CausalCycleViolation {
        event_id: String,
        cycle_path: String,
    },
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
            KernelError::CausalCycleViolation { event_id, cycle_path } => {
                write!(f, "CausalCycleViolation Error: event '{}' would create cycle: {}", event_id, cycle_path)
            }
        }
    }
}

impl std::error::Error for KernelError {}
