use sha2::{Digest, Sha256};

use crate::observability::{AuditLog, MetricsSnapshot};

/// A deterministic, self-consistent observability report.
///
/// Combines metrics from `MetricsSnapshot` with consistency validation
/// against `AuditLog`.  All fields are derived purely from observability
/// data — no kernel, replay, or persistence access.
///
/// ## Invariants
///
/// - I1: No hidden state — every field has an explicit definition.
/// - I2: Deterministic — same inputs produce byte-identical reports.
/// - I3: Replay independence — export never touches kernel or engine.
/// - I4: Audit ↔ Metrics consistency — enforced by `consistency_check_passed`.
/// - I5: No mutation — inputs are never modified.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ObservabilityReport {
    // ── Metrics from MetricsSnapshot ────────────────────────────────────────
    pub total_commands: u64,
    pub execution_commands: u64,
    pub query_commands: u64,
    pub system_commands: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_sequence: u64,

    // ── Audit metadata ─────────────────────────────────────────────────────
    pub audit_log_length: usize,

    // ── Deterministic hashes (SHA-256 of canonical JSON) ───────────────────
    pub metrics_snapshot_hash: String,
    pub audit_log_hash: String,

    // ── Consistency check ──────────────────────────────────────────────────
    pub consistency_check_passed: bool,
}

fn sha256_hex_from_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in &result {
        use std::fmt::Write;
        write!(&mut hex, "{:02x}", byte).unwrap();
    }
    hex
}

/// Export a complete observability report from an `AuditLog` and its
/// pre-computed `MetricsSnapshot`.
///
/// ## Determinism
///
/// Identical inputs always produce byte-identical output.  The report
/// contains SHA-256 hashes of both the audit log and metrics snapshot,
/// allowing external consumers to verify integrity without re-exporting.
pub fn export_observability(audit: &AuditLog, metrics: &MetricsSnapshot) -> ObservabilityReport {
    // ── Consistency validation (pure, no side effects) ─────────────────
    let mut consistent = true;

    // Check total matches audit length.
    if metrics.total_commands != audit.count() as u64 {
        consistent = false;
    }

    // Check success/failure counts match audit records.
    let (audit_success, audit_failure) = {
        let mut s = 0u64;
        let mut f = 0u64;
        for rec in audit.records() {
            if rec.success { s += 1; } else { f += 1; }
        }
        (s, f)
    };
    if metrics.successful_commands != audit_success {
        consistent = false;
    }
    if metrics.failed_commands != audit_failure {
        consistent = false;
    }

    // Check last_sequence matches the highest audit sequence.
    let audit_last_seq = audit.records().last().map(|r| r.sequence).unwrap_or(0);
    if metrics.last_sequence != audit_last_seq {
        consistent = false;
    }

    // ── Deterministic hashes ───────────────────────────────────────────
    let metrics_canonical =
        serde_json::to_vec(metrics).expect("MetricsSnapshot serialization must not fail");
    let audit_canonical = serde_json::to_vec(audit.records())
        .expect("AuditLog records serialization must not fail");
    let metrics_hash = sha256_hex_from_bytes(&metrics_canonical);
    let audit_hash = sha256_hex_from_bytes(&audit_canonical);

    ObservabilityReport {
        total_commands: metrics.total_commands,
        execution_commands: metrics.execution_commands,
        query_commands: metrics.query_commands,
        system_commands: metrics.system_commands,
        success_count: metrics.successful_commands,
        failure_count: metrics.failed_commands,
        last_sequence: metrics.last_sequence,
        audit_log_length: audit.count(),
        metrics_snapshot_hash: metrics_hash,
        audit_log_hash: audit_hash,
        consistency_check_passed: consistent,
    }
}

/// Deterministic JSON exporter for the audit log.
pub struct AuditExporter;

impl AuditExporter {
    /// Export the full audit log as a compact JSON array.
    pub fn export_audit_json(audit: &AuditLog) -> String {
        serde_json::to_string(audit.records())
            .expect("audit records are always serializable")
    }
}

/// Deterministic JSON exporter for metrics snapshots.
pub struct MetricsExporter;

impl MetricsExporter {
    /// Export the metrics snapshot as a compact JSON object.
    pub fn export_metrics_json(metrics: &MetricsSnapshot) -> String {
        serde_json::to_string(metrics)
            .expect("MetricsSnapshot is always serializable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::command::CommandClass;
    use crate::api::envelope::TraceId;

    fn sample_log() -> AuditLog {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::Query, "QueryState", true);
        log.append(TraceId(2), CommandClass::System, "GetVersion", true);
        log.append(TraceId(3), CommandClass::Execution, "Ingest", false);
        log
    }

    fn sample_metrics() -> MetricsSnapshot {
        MetricsSnapshot {
            total_commands: 3,
            execution_commands: 1,
            query_commands: 1,
            system_commands: 1,
            successful_commands: 2,
            failed_commands: 1,
            last_sequence: 3,
        }
    }

    #[test]
    fn empty_system_export() {
        let log = AuditLog::new();
        let metrics = MetricsSnapshot {
            total_commands: 0,
            execution_commands: 0,
            query_commands: 0,
            system_commands: 0,
            successful_commands: 0,
            failed_commands: 0,
            last_sequence: 0,
        };
        let report = export_observability(&log, &metrics);
        assert_eq!(report.total_commands, 0);
        assert_eq!(report.audit_log_length, 0);
        assert!(report.consistency_check_passed);
        assert!(!report.metrics_snapshot_hash.is_empty());
        assert!(!report.audit_log_hash.is_empty());
    }

    #[test]
    fn single_event_consistency() {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::System, "GetVersion", true);
        let metrics = MetricsSnapshot {
            total_commands: 1,
            execution_commands: 0,
            query_commands: 0,
            system_commands: 1,
            successful_commands: 1,
            failed_commands: 0,
            last_sequence: 1,
        };
        let report = export_observability(&log, &metrics);
        assert!(report.consistency_check_passed);
        assert_eq!(report.total_commands, 1);
        assert_eq!(report.audit_log_length, 1);
    }

    #[test]
    fn mixed_success_failure_export() {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::Execution, "Ingest", true);
        log.append(TraceId(2), CommandClass::Query, "QueryState", true);
        log.append(TraceId(3), CommandClass::System, "GetVersion", false);
        log.append(TraceId(4), CommandClass::Query, "ListNodes", false);
        let metrics = MetricsSnapshot {
            total_commands: 4,
            execution_commands: 1,
            query_commands: 2,
            system_commands: 1,
            successful_commands: 2,
            failed_commands: 2,
            last_sequence: 4,
        };
        let report = export_observability(&log, &metrics);
        assert!(report.consistency_check_passed);
        assert_eq!(report.success_count, 2);
        assert_eq!(report.failure_count, 2);
    }

    #[test]
    fn deterministic_hashing_across_repeated_runs() {
        let log = sample_log();
        let metrics = sample_metrics();
        let a = export_observability(&log, &metrics);
        let b = export_observability(&log, &metrics);
        assert_eq!(a.metrics_snapshot_hash, b.metrics_snapshot_hash);
        assert_eq!(a.audit_log_hash, b.audit_log_hash);
        assert_eq!(a, b, "full report must be byte-identical");
    }

    #[test]
    fn mismatch_detection() {
        let log = sample_log();
        // Inject a metrics mismatch: wrong total.
        let bad_metrics = MetricsSnapshot {
            total_commands: 999,
            ..sample_metrics()
        };
        let report = export_observability(&log, &bad_metrics);
        assert!(!report.consistency_check_passed);
    }

    #[test]
    fn large_log_stability() {
        let mut log = AuditLog::new();
        for i in 0..10_000 {
            let cls = match i % 3 {
                0 => CommandClass::Execution,
                1 => CommandClass::Query,
                _ => CommandClass::System,
            };
            log.append(TraceId(i as u64), cls, "Op", i % 2 == 0);
        }
        let metrics = MetricsSnapshot {
            total_commands: 10_000,
            execution_commands: 3334,
            query_commands: 3333,
            system_commands: 3333,
            successful_commands: 5000,
            failed_commands: 5000,
            last_sequence: 10_000,
        };
        // Must not panic.
        let report = export_observability(&log, &metrics);
        assert!(report.consistency_check_passed);
        assert_eq!(report.audit_log_length, 10_000);
        assert_eq!(report.total_commands, 10_000);
    }

    #[test]
    fn export_does_not_mutate_source() {
        let log = sample_log();
        let metrics = sample_metrics();
        let before_log = serde_json::to_string(log.records()).unwrap();
        let _report = export_observability(&log, &metrics);
        let after_log = serde_json::to_string(log.records()).unwrap();
        assert_eq!(before_log, after_log, "export must not mutate audit log");
    }

    #[test]
    fn audit_exporter_empty_export() {
        let log = AuditLog::new();
        assert_eq!(AuditExporter::export_audit_json(&log), "[]");
    }

    #[test]
    fn metrics_exporter_deterministic() {
        let m = sample_metrics();
        let a = MetricsExporter::export_metrics_json(&m);
        let b = MetricsExporter::export_metrics_json(&m);
        assert_eq!(a, b);
    }
}
