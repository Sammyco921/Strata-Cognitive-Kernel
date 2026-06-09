use serde::Serialize;

use crate::api::command::CommandClass;
use crate::api::envelope::TraceId;

/// A single deterministic audit record.
///
/// Every command execution produces exactly one audit record, appended
/// to the `AuditLog` after the `CommandResultV1` is produced.
///
/// ## Invariants
///
/// - `sequence` is strictly monotonic and starts at 1.
/// - `timestamp` is a deterministic u64 counter, NOT wall-clock time.
/// - `trace_id` is copied from the originating `CommandEnvelope`.
/// - `command_class` is the `CommandClass` of the executed command.
/// - `command_name` is a stable variant name (never changes for a variant).
/// - `success` is `true` iff the result payload is not `ResultPayload::Error`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuditRecord {
    pub sequence: u64,
    pub trace_id: TraceId,
    pub command_class: CommandClass,
    pub command_name: String,
    pub timestamp: u64,
    pub success: bool,
}

/// A deterministic, append-only audit log.
///
/// ## Invariants
///
/// - E1 — No kernel mutation: audit log cannot mutate kernel state.
/// - E2 — No influence: audit log cannot influence command results.
/// - E3 — Replay independence: deleting audit log must not change replay.
/// - E4 — Ordering: audit log ordering must match trace ordering.
/// - E5 — Determinism: audit records must be deterministic.
#[derive(Debug, Clone)]
pub struct AuditLog {
    records: Vec<AuditRecord>,
    sequence_counter: u64,
    timestamp_counter: u64,
}

impl AuditLog {
    /// Create an empty audit log.
    pub fn new() -> Self {
        AuditLog {
            records: Vec::new(),
            sequence_counter: 0,
            timestamp_counter: 0,
        }
    }

    /// Append a new audit record after command execution.
    ///
    /// This is the only way to add records.  Prior records are never
    /// mutated or deleted.
    pub fn append(
        &mut self,
        trace_id: TraceId,
        class: CommandClass,
        command_name: &str,
        success: bool,
    ) {
        self.sequence_counter += 1;
        self.timestamp_counter += 1;
        self.records.push(AuditRecord {
            sequence: self.sequence_counter,
            trace_id,
            command_class: class,
            command_name: command_name.to_string(),
            timestamp: self.timestamp_counter,
            success,
        });
    }

    /// Return a reference to all audit records.
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    /// Return the number of audit records.
    pub fn count(&self) -> usize {
        self.records.len()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::command::CommandClass;
    use crate::api::envelope::TraceId;

    #[test]
    fn audit_log_empty_initially() {
        let log = AuditLog::new();
        assert_eq!(log.count(), 0);
        assert!(log.records().is_empty());
    }

    #[test]
    fn audit_record_created_after_success() {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::Execution, "Ingest", true);
        assert_eq!(log.count(), 1);
        let record = &log.records()[0];
        assert_eq!(record.sequence, 1);
        assert_eq!(record.trace_id, TraceId(1));
        assert_eq!(record.command_class, CommandClass::Execution);
        assert_eq!(record.command_name, "Ingest");
        assert!(record.success);
    }

    #[test]
    fn audit_record_created_after_error() {
        let mut log = AuditLog::new();
        log.append(TraceId(2), CommandClass::System, "GetVersion", false);
        assert_eq!(log.count(), 1);
        let record = &log.records()[0];
        assert!(!record.success);
    }

    #[test]
    fn sequence_monotonic() {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::Query, "QueryState", true);
        log.append(TraceId(2), CommandClass::System, "GetVersion", true);
        log.append(TraceId(3), CommandClass::Execution, "Ingest", false);
        assert_eq!(log.count(), 3);
        for (i, record) in log.records().iter().enumerate() {
            assert_eq!(record.sequence, (i + 1) as u64);
        }
    }

    #[test]
    fn trace_id_preserved() {
        let mut log = AuditLog::new();
        log.append(TraceId(42), CommandClass::Query, "QueryState", true);
        assert_eq!(log.records()[0].trace_id, TraceId(42));
        log.append(TraceId(99), CommandClass::System, "GetVersion", true);
        assert_eq!(log.records()[1].trace_id, TraceId(99));
    }

    #[test]
    fn command_class_preserved() {
        let mut log = AuditLog::new();
        log.append(TraceId(0), CommandClass::Execution, "Ingest", true);
        log.append(TraceId(1), CommandClass::Query, "QueryState", true);
        log.append(TraceId(2), CommandClass::System, "GetVersion", true);
        assert_eq!(log.records()[0].command_class, CommandClass::Execution);
        assert_eq!(log.records()[1].command_class, CommandClass::Query);
        assert_eq!(log.records()[2].command_class, CommandClass::System);
    }

    #[test]
    fn command_name_stable() {
        let mut log = AuditLog::new();
        log.append(TraceId(0), CommandClass::Query, "QueryState", true);
        log.append(TraceId(1), CommandClass::Query, "QueryState", true);
        assert_eq!(log.records()[0].command_name, "QueryState");
        assert_eq!(log.records()[1].command_name, "QueryState");
    }

    #[test]
    fn audit_order_matches_append_order() {
        let mut log = AuditLog::new();
        log.append(TraceId(10), CommandClass::System, "GetVersion", true);
        log.append(TraceId(20), CommandClass::Query, "QueryState", true);
        log.append(TraceId(30), CommandClass::Execution, "Ingest", false);
        let recs = log.records();
        assert_eq!(recs[0].trace_id, TraceId(10));
        assert_eq!(recs[1].trace_id, TraceId(20));
        assert_eq!(recs[2].trace_id, TraceId(30));
        assert_eq!(recs[0].sequence, 1);
        assert_eq!(recs[1].sequence, 2);
        assert_eq!(recs[2].sequence, 3);
    }

    #[test]
    fn audit_log_is_append_only() {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::Query, "QueryState", true);
        let before = log.count();
        log.append(TraceId(2), CommandClass::System, "GetVersion", true);
        assert_eq!(log.count(), before + 1);
        // Prior record must be unchanged.
        assert_eq!(log.records()[0].trace_id, TraceId(1));
        assert_eq!(log.records()[0].command_name, "QueryState");
    }

    #[test]
    fn deterministic_repeated_execution() {
        let mut a = AuditLog::new();
        let mut b = AuditLog::new();
        for i in 0..5 {
            a.append(TraceId(i), CommandClass::Query, "QueryState", i % 2 == 0);
            b.append(TraceId(i), CommandClass::Query, "QueryState", i % 2 == 0);
        }
        assert_eq!(a.records(), b.records());
    }

    #[test]
    fn serialization_deterministic() {
        let mut log = AuditLog::new();
        log.append(TraceId(7), CommandClass::Execution, "Ingest", true);
        log.append(TraceId(8), CommandClass::System, "GetVersion", false);
        let json_a = serde_json::to_string(&log.records()).unwrap();
        let json_b = serde_json::to_string(&log.records()).unwrap();
        assert_eq!(json_a, json_b);
    }

    #[test]
    fn timestamp_is_deterministic() {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::Query, "Q", true);
        log.append(TraceId(2), CommandClass::Query, "Q", true);
        assert_eq!(log.records()[0].timestamp, 1);
        assert_eq!(log.records()[1].timestamp, 2);
    }

    #[test]
    fn audit_log_count_matches_records() {
        let mut log = AuditLog::new();
        assert_eq!(log.count(), log.records().len());
        log.append(TraceId(1), CommandClass::Query, "Q", true);
        assert_eq!(log.count(), log.records().len());
        log.append(TraceId(2), CommandClass::Query, "Q", false);
        assert_eq!(log.count(), log.records().len());
    }

    #[test]
    fn audit_log_survives_100_commands() {
        let mut log = AuditLog::new();
        for i in 0..100 {
            log.append(TraceId(i), CommandClass::System, "Op", i % 2 == 0);
        }
        assert_eq!(log.count(), 100);
        assert_eq!(log.records()[99].sequence, 100);
        assert_eq!(log.records()[0].sequence, 1);
    }
}
