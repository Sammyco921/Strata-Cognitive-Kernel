use crate::observability::AuditLog;
use crate::api::command::CommandClass;

/// A deterministic metrics snapshot derived from `AuditLog`.
///
/// All counters are computed by a single O(n) pass over `audit.records()`.
/// The snapshot is a pure projection — it never reads kernel state,
/// never mutates the audit log, and never accesses persistence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MetricsSnapshot {
    pub total_commands: u64,
    pub execution_commands: u64,
    pub query_commands: u64,
    pub system_commands: u64,
    pub successful_commands: u64,
    pub failed_commands: u64,
    pub last_sequence: u64,
}

/// Pure-function collector that produces a `MetricsSnapshot` from an
/// `AuditLog`.
///
/// ## Invariants
///
/// - Read-only: never mutates the audit log.
/// - Deterministic: same audit log → same snapshot.
/// - No global state: all state is local to the function.
/// - No kernel access: never inspects `GraphState`, replay, or persistence.
pub struct MetricsCollector;

impl MetricsCollector {
    /// Compute a metrics snapshot from audit records.
    ///
    /// This is a pure O(n) scan with no allocation beyond the result struct.
    pub fn from_audit_log(audit: &AuditLog) -> MetricsSnapshot {
        let mut total = 0u64;
        let mut execution = 0u64;
        let mut query = 0u64;
        let mut system = 0u64;
        let mut success = 0u64;
        let mut failed = 0u64;
        let mut last_seq = 0u64;

        for record in audit.records() {
            total += 1;
            match record.command_class {
                CommandClass::Execution => execution += 1,
                CommandClass::Query => query += 1,
                CommandClass::System => system += 1,
            }
            if record.success {
                success += 1;
            } else {
                failed += 1;
            }
            last_seq = record.sequence;
        }

        MetricsSnapshot {
            total_commands: total,
            execution_commands: execution,
            query_commands: query,
            system_commands: system,
            successful_commands: success,
            failed_commands: failed,
            last_sequence: last_seq,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::command::CommandClass;
    use crate::api::envelope::TraceId;

    fn log_with(records: Vec<(TraceId, CommandClass, &'static str, bool)>) -> AuditLog {
        let mut log = AuditLog::new();
        for (tid, cls, name, ok) in records {
            log.append(tid, cls, name, ok);
        }
        log
    }

    #[test]
    fn empty_log() {
        let log = AuditLog::new();
        let m = MetricsCollector::from_audit_log(&log);
        assert_eq!(m.total_commands, 0);
        assert_eq!(m.execution_commands, 0);
        assert_eq!(m.query_commands, 0);
        assert_eq!(m.system_commands, 0);
        assert_eq!(m.successful_commands, 0);
        assert_eq!(m.failed_commands, 0);
        assert_eq!(m.last_sequence, 0);
    }

    #[test]
    fn single_command() {
        let log = log_with(vec![(TraceId(1), CommandClass::Query, "QueryState", true)]);
        let m = MetricsCollector::from_audit_log(&log);
        assert_eq!(m.total_commands, 1);
        assert_eq!(m.query_commands, 1);
        assert_eq!(m.execution_commands, 0);
        assert_eq!(m.system_commands, 0);
        assert_eq!(m.successful_commands, 1);
        assert_eq!(m.failed_commands, 0);
        assert_eq!(m.last_sequence, 1);
    }

    #[test]
    fn mixed_command_classes() {
        let log = log_with(vec![
            (TraceId(1), CommandClass::Execution, "Ingest", true),
            (TraceId(2), CommandClass::Query, "QueryState", true),
            (TraceId(3), CommandClass::System, "GetVersion", true),
            (TraceId(4), CommandClass::Execution, "Ingest", false),
            (TraceId(5), CommandClass::Query, "ListNodes", true),
        ]);
        let m = MetricsCollector::from_audit_log(&log);
        assert_eq!(m.total_commands, 5);
        assert_eq!(m.execution_commands, 2);
        assert_eq!(m.query_commands, 2);
        assert_eq!(m.system_commands, 1);
        assert_eq!(m.successful_commands, 4);
        assert_eq!(m.failed_commands, 1);
        assert_eq!(m.last_sequence, 5);
    }

    #[test]
    fn success_failure_counting() {
        let log = log_with(vec![
            (TraceId(1), CommandClass::Query, "Q", true),
            (TraceId(2), CommandClass::System, "V", false),
            (TraceId(3), CommandClass::Execution, "I", true),
            (TraceId(4), CommandClass::Query, "L", false),
            (TraceId(5), CommandClass::System, "S", true),
        ]);
        let m = MetricsCollector::from_audit_log(&log);
        assert_eq!(m.total_commands, 5);
        assert_eq!(m.successful_commands, 3);
        assert_eq!(m.failed_commands, 2);
        assert_eq!(m.successful_commands + m.failed_commands, m.total_commands);
    }

    #[test]
    fn determinism() {
        let log = log_with(vec![
            (TraceId(1), CommandClass::Execution, "Ingest", true),
            (TraceId(2), CommandClass::Query, "QueryState", true),
        ]);
        let a = MetricsCollector::from_audit_log(&log);
        let b = MetricsCollector::from_audit_log(&log);
        assert_eq!(a, b);
        // Also verify that calling twice on the same log gives the same result.
        let c = MetricsCollector::from_audit_log(&log);
        assert_eq!(a, c);
    }

    #[test]
    fn append_only_growth() {
        let mut log = AuditLog::new();
        let m0 = MetricsCollector::from_audit_log(&log);
        assert_eq!(m0.total_commands, 0);

        log.append(TraceId(1), CommandClass::Query, "Q", true);
        let m1 = MetricsCollector::from_audit_log(&log);
        assert_eq!(m1.total_commands, 1);

        log.append(TraceId(2), CommandClass::System, "V", false);
        let m2 = MetricsCollector::from_audit_log(&log);
        assert_eq!(m2.total_commands, 2);
        assert_eq!(m2.query_commands, 1);
        assert_eq!(m2.system_commands, 1);
        assert_eq!(m1.total_commands, 1, "prior snapshot must remain valid");
    }

    #[test]
    fn snapshot_reproducibility() {
        let log = log_with(vec![
            (TraceId(1), CommandClass::Execution, "Ingest", false),
            (TraceId(2), CommandClass::Query, "QueryState", true),
            (TraceId(3), CommandClass::System, "GetVersion", true),
            (TraceId(4), CommandClass::Query, "ListNodes", true),
            (TraceId(5), CommandClass::System, "ValidateLog", false),
        ]);
        // Collect multiple times — every result must be byte-for-byte identical.
        let snapshots: Vec<MetricsSnapshot> = (0..10)
            .map(|_| MetricsCollector::from_audit_log(&log))
            .collect();
        for pair in snapshots.windows(2) {
            assert_eq!(pair[0], pair[1]);
        }
    }

    #[test]
    fn last_sequence_on_empty_log() {
        let log = AuditLog::new();
        let m = MetricsCollector::from_audit_log(&log);
        assert_eq!(m.last_sequence, 0);
    }

    #[test]
    fn last_sequence_matches_highest_sequence() {
        let log = log_with(vec![
            (TraceId(1), CommandClass::Query, "Q", true),
            (TraceId(2), CommandClass::Query, "Q", true),
            (TraceId(3), CommandClass::Query, "Q", true),
        ]);
        let m = MetricsCollector::from_audit_log(&log);
        assert_eq!(m.last_sequence, 3);
    }

    #[test]
    fn all_failures() {
        let log = log_with(vec![
            (TraceId(1), CommandClass::Execution, "I", false),
            (TraceId(2), CommandClass::Query, "Q", false),
            (TraceId(3), CommandClass::System, "V", false),
        ]);
        let m = MetricsCollector::from_audit_log(&log);
        assert_eq!(m.total_commands, 3);
        assert_eq!(m.failed_commands, 3);
        assert_eq!(m.successful_commands, 0);
        assert_eq!(m.execution_commands, 1);
        assert_eq!(m.query_commands, 1);
        assert_eq!(m.system_commands, 1);
    }
}
