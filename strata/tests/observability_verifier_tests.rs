// ── Phase E4: Observability Integrity Verifier Integration Tests ───────────

use strata::observability::{
    verify_observability_integrity, AuditLog, MetricsSnapshot,
};
use strata::api::command::CommandClass;

fn consistent_log_and_metrics() -> (AuditLog, MetricsSnapshot) {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), CommandClass::Execution, "Ingest", true);
    log.append(strata::api::envelope::TraceId(2), CommandClass::Query, "QueryState", true);
    log.append(strata::api::envelope::TraceId(3), CommandClass::System, "GetVersion", false);
    let metrics = MetricsSnapshot {
        total_commands: 3, execution_commands: 1, query_commands: 1,
        system_commands: 1, successful_commands: 2, failed_commands: 1,
        last_sequence: 3,
    };
    (log, metrics)
}

#[test]
fn perfect_match() {
    let (log, metrics) = consistent_log_and_metrics();
    let result = verify_observability_integrity(&log, &metrics);
    assert!(result.valid);
    assert!(result.mismatches.is_empty());
}

#[test]
fn injected_count_mismatch() {
    let (log, metrics) = consistent_log_and_metrics();
    let bad = MetricsSnapshot { total_commands: 999, ..metrics };
    let result = verify_observability_integrity(&log, &bad);
    assert!(!result.valid);
    assert!(result.mismatches.iter().any(|v| v.violation_type == "AuditMetricsCountMismatch"));
}

#[test]
fn injected_success_mismatch() {
    let (log, metrics) = consistent_log_and_metrics();
    let bad = MetricsSnapshot { successful_commands: 0, ..metrics };
    let result = verify_observability_integrity(&log, &bad);
    assert!(!result.valid);
    assert!(result.mismatches.iter().any(|v| v.violation_type == "SuccessCountMismatch"));
}

#[test]
fn injected_failure_mismatch() {
    let (log, metrics) = consistent_log_and_metrics();
    let bad = MetricsSnapshot { failed_commands: 0, ..metrics };
    let result = verify_observability_integrity(&log, &bad);
    assert!(!result.valid);
    assert!(result.mismatches.iter().any(|v| v.violation_type == "FailureCountMismatch"));
}

#[test]
fn injected_sequence_drift() {
    let (log, metrics) = consistent_log_and_metrics();
    let bad = MetricsSnapshot { last_sequence: 999, ..metrics };
    let result = verify_observability_integrity(&log, &bad);
    assert!(!result.valid);
    assert!(result.mismatches.iter().any(|v| v.violation_type == "SequenceDriftDetected"));
}

#[test]
fn partial_corruption_simulation() {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), CommandClass::Query, "Q", true);
    log.append(strata::api::envelope::TraceId(2), CommandClass::Query, "Q", false);
    let metrics = MetricsSnapshot {
        total_commands: 1, execution_commands: 0, query_commands: 1,
        system_commands: 0, successful_commands: 2, failed_commands: 0,
        last_sequence: 1,
    };
    let result = verify_observability_integrity(&log, &metrics);
    assert!(!result.valid);
    assert!(result.mismatches.iter().any(|v| v.violation_type == "AuditMetricsCountMismatch"));
    assert!(result.mismatches.iter().any(|v| v.violation_type == "SuccessCountMismatch"));
    assert!(result.mismatches.iter().any(|v| v.violation_type == "FailureCountMismatch"));
    assert!(result.mismatches.iter().any(|v| v.violation_type == "SequenceDriftDetected"));
}

#[test]
fn empty_system_validation() {
    let log = AuditLog::new();
    let metrics = MetricsSnapshot {
        total_commands: 0, execution_commands: 0, query_commands: 0,
        system_commands: 0, successful_commands: 0, failed_commands: 0,
        last_sequence: 0,
    };
    let result = verify_observability_integrity(&log, &metrics);
    assert!(result.valid);
}

#[test]
fn large_log_stress_validation() {
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
    let result = verify_observability_integrity(&log, &metrics);
    assert!(result.valid, "large log must pass verification");
}

#[test]
fn deterministic_mismatch_ordering() {
    let log = AuditLog::new();
    let metrics = MetricsSnapshot {
        total_commands: 5, execution_commands: 0, query_commands: 0,
        system_commands: 0, successful_commands: 3, failed_commands: 2,
        last_sequence: 5,
    };
    let a = verify_observability_integrity(&log, &metrics);
    let b = verify_observability_integrity(&log, &metrics);
    assert_eq!(a.mismatches, b.mismatches);
}
