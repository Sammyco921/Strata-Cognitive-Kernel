use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::api::command::Command;

/// Current envelope schema version.
pub const ENVELOPE_VERSION: u32 = 1;

/// A deterministic, monotonically increasing trace identifier.
///
/// Generated at envelope creation time using an process-local atomic
/// counter.  Within a single process the sequence is fully deterministic:
/// the first call to `generate()` returns `TraceId(0)`, the second
/// `TraceId(1)`, and so on.
///
/// For a single CLI invocation (one command per process) the trace ID is
/// always `TraceId(0)`.  Integration tests that create multiple envelopes
/// see a predictable ascending sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TraceId(pub u64);

impl TraceId {
    /// Create the next trace identifier in the process-global sequence.
    ///
    /// ## Determinism
    ///
    /// The first call returns `0`, the second `1`, etc.  This satisfies
    /// I4 (Deterministic Construction): given the same ordering of
    /// envelope creations, the resulting `TraceId` values are identical
    /// across process runs.
    pub fn generate() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        TraceId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A versioned command envelope for CLI → kernel communication.
///
/// Every request carries a schema `version`, a deterministic `trace_id`,
/// and a `command` payload.  No command may be sent to the kernel without
/// an envelope.
///
/// ## Invariants (non-negotiable)
///
/// - **I1 — Envelope Integrity**: every executed command originates from a
///   `CommandEnvelope`.
/// - **I2 — Immutability**: once created, `version`, `trace_id`, and
///   `command` are never modified.
/// - **I3 — No Bypass Path**: no execution path accepts a raw `Command`.
/// - **I4 — Deterministic Construction**: with the same command and
///   creation ordering, the `CommandEnvelope` is identical.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub version: u32,
    pub trace_id: TraceId,
    pub command: Command,
    /// Optional session identifier for REPL session grouping.
    /// Purely informational — does not affect determinism or execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub session_id: Option<u64>,
}

impl CommandEnvelope {
    /// Create a new envelope with a generated `trace_id` and no session.
    pub fn new(command: Command) -> Self {
        CommandEnvelope {
            version: ENVELOPE_VERSION,
            trace_id: TraceId::generate(),
            command,
            session_id: None,
        }
    }

    /// Create a new envelope with an explicit `trace_id` (for testing).
    pub fn with_id(trace_id: TraceId, command: Command) -> Self {
        CommandEnvelope {
            version: ENVELOPE_VERSION,
            trace_id,
            command,
            session_id: None,
        }
    }

    /// Attach a session identifier, returning a new envelope.
    /// This is purely informational and does not affect execution.
    pub fn with_session_id(self, session_id: u64) -> Self {
        CommandEnvelope {
            session_id: Some(session_id),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_sequence_is_deterministic() {
        // Must be first among envelope tests to guarantee counter starts at 0.
        let a = TraceId::generate();
        let b = TraceId::generate();
        let c = TraceId::generate();
        assert_eq!(a.0, 0, "first call must return 0");
        assert_eq!(b.0, 1, "second call must return 1");
        assert_eq!(c.0, 2, "third call must return 2");
    }

    #[test]
    fn envelope_with_id_uses_provided_trace_id() {
        // Use explicit IDs to avoid consuming the global counter.
        let tid_a = TraceId(100);
        let tid_b = TraceId(200);
        let env_a = CommandEnvelope::with_id(tid_a, Command::QueryState);
        let env_b = CommandEnvelope::with_id(tid_b, Command::QueryState);
        assert_eq!(env_a.trace_id, tid_a);
        assert_eq!(env_b.trace_id, tid_b);
        assert!(env_b.trace_id > env_a.trace_id);
    }

    #[test]
    fn envelope_default_version() {
        let tid = TraceId(42);
        let env = CommandEnvelope::with_id(tid, Command::QueryState);
        assert_eq!(env.version, ENVELOPE_VERSION);
    }
}

