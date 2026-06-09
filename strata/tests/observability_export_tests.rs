// ── Phase E3: Observability Export Integration Tests ───────────────────────

use strata::observability::{export_observability, AuditLog, MetricsSnapshot};
use strata::api::command::CommandClass;

fn sample_log_and_metrics() -> (AuditLog, MetricsSnapshot) {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), CommandClass::Query, "QueryState", true);
    log.append(strata::api::envelope::TraceId(2), CommandClass::System, "GetVersion", true);
    log.append(strata::api::envelope::TraceId(3), CommandClass::Execution, "Ingest", false);
    let metrics = MetricsSnapshot {
        total_commands: 3,
        execution_commands: 1,
        query_commands: 1,
        system_commands: 1,
        successful_commands: 2,
        failed_commands: 1,
        last_sequence: 3,
    };
    (log, metrics)
}

#[test]
fn empty_system_export() {
    let log = AuditLog::new();
    let metrics = MetricsSnapshot {
        total_commands: 0, execution_commands: 0, query_commands: 0,
        system_commands: 0, successful_commands: 0, failed_commands: 0,
        last_sequence: 0,
    };
    let report = export_observability(&log, &metrics);
    assert_eq!(report.total_commands, 0);
    assert_eq!(report.audit_log_length, 0);
    assert!(report.consistency_check_passed);
}

#[test]
fn single_event_consistency() {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), CommandClass::System, "GetVersion", true);
    let metrics = MetricsSnapshot {
        total_commands: 1, execution_commands: 0, query_commands: 0,
        system_commands: 1, successful_commands: 1, failed_commands: 0,
        last_sequence: 1,
    };
    let report = export_observability(&log, &metrics);
    assert!(report.consistency_check_passed);
    assert_eq!(report.total_commands, 1);
    assert_eq!(report.audit_log_length, 1);
}

#[test]
fn mixed_success_failure_logs() {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), CommandClass::Execution, "I", true);
    log.append(strata::api::envelope::TraceId(2), CommandClass::Query, "Q", false);
    log.append(strata::api::envelope::TraceId(3), CommandClass::System, "V", true);
    log.append(strata::api::envelope::TraceId(4), CommandClass::Execution, "I", false);
    let metrics = MetricsSnapshot {
        total_commands: 4, execution_commands: 2, query_commands: 1,
        system_commands: 1, successful_commands: 2, failed_commands: 2,
        last_sequence: 4,
    };
    let report = export_observability(&log, &metrics);
    assert!(report.consistency_check_passed);
    assert_eq!(report.success_count, 2);
    assert_eq!(report.failure_count, 2);
}

#[test]
fn deterministic_hashing_across_repeated_runs() {
    let (log, metrics) = sample_log_and_metrics();
    let a = export_observability(&log, &metrics);
    let b = export_observability(&log, &metrics);
    assert_eq!(a.metrics_snapshot_hash, b.metrics_snapshot_hash);
    assert_eq!(a.audit_log_hash, b.audit_log_hash);
    assert_eq!(a, b);
}

#[test]
fn mismatch_detection() {
    let (log, metrics) = sample_log_and_metrics();
    let bad = MetricsSnapshot { total_commands: 999, ..metrics };
    let report = export_observability(&log, &bad);
    assert!(!report.consistency_check_passed);
}

#[test]
fn large_log_stability() {
    let mut log = AuditLog::new();
    for i in 0..10_000 {
        let cls = match i % 3 { 0 => CommandClass::Execution, 1 => CommandClass::Query, _ => CommandClass::System };
        log.append(strata::api::envelope::TraceId(i), cls, "Op", i % 2 == 0);
    }
    let metrics = MetricsSnapshot {
        total_commands: 10_000, execution_commands: 3334, query_commands: 3333,
        system_commands: 3333, successful_commands: 5000, failed_commands: 5000,
        last_sequence: 10_000,
    };
    let report = export_observability(&log, &metrics);
    assert!(report.consistency_check_passed);
    assert_eq!(report.audit_log_length, 10_000);
}
