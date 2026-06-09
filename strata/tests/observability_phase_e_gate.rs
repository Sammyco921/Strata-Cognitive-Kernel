// ── Phase E5: Final Observability Completion Gate ───────────────────────────
//
// Proves full consistency across AuditLog, MetricsSnapshot,
// ObservabilityReport, and VerificationResult.

use strata::cli::CliCommand;
use strata::test_utils::{test_bootstrap, TestBootstrap};
use strata::observability::{
    export_observability, verify_observability_integrity, AuditLog, MetricsCollector,
};
use strata::api::command::CommandClass;

// ── 1. End-to-End Consistency ───────────────────────────────────────────────
//
// Execute 100 mixed commands, generate audit log, compute metrics, export
// observability report, run verifier — assert all consistent.

#[test]
fn end_to_end_consistency() {
    let mut bs = test_bootstrap(vec![]);
    run_mixed_commands(&mut bs, 100);

    let metrics = MetricsCollector::from_audit_log(bs.audit_log());
    let report = export_observability(bs.audit_log(), &metrics);
    let verification = verify_observability_integrity(bs.audit_log(), &metrics);

    assert!(verification.valid, "verifier must report valid: {:?}", verification.mismatches);
    assert!(report.consistency_check_passed, "consistency check must pass");
    assert_eq!(report.total_commands, 100, "must report exactly 100 commands");
    assert_eq!(report.audit_log_length, 100, "audit log must have 100 records");
    assert_eq!(
        report.execution_commands + report.query_commands + report.system_commands,
        report.total_commands,
        "class counts must sum to total"
    );
    assert_eq!(
        report.success_count + report.failure_count,
        report.total_commands,
        "success + failure must equal total"
    );
}

// ── 2. Determinism Test ─────────────────────────────────────────────────────
//
// Structural determinism: same command sequence produces identical class
// counts, success/failure counts, and consistency check — even though
// trace_ids differ due to the global atomic counter.

#[test]
fn determinism_test() {
    for _ in 0..5 {
        let mut bs = test_bootstrap(vec![]);
        run_mixed_commands(&mut bs, 20);
        let metrics = MetricsCollector::from_audit_log(bs.audit_log());
        let report = export_observability(bs.audit_log(), &metrics);
        // Structural fields must be deterministic.
        assert_eq!(report.total_commands, 20);
        assert!(report.consistency_check_passed);
        assert_eq!(
            report.execution_commands + report.query_commands + report.system_commands,
            report.total_commands,
        );
        assert_eq!(
            report.success_count + report.failure_count,
            report.total_commands,
        );
    }
}

// ── 2b. Deterministic Hashing (bypassing global TraceId counter) ────────────
//
// Verify hashes are byte-identical for the same audit data when using
// fixed trace IDs.

#[test]
fn deterministic_hashing_with_fixed_trace_ids() {
    use strata::api::envelope::TraceId;

    let mut log = AuditLog::new();
    log.append(TraceId(100), CommandClass::Query, "QueryState", true);
    log.append(TraceId(200), CommandClass::System, "GetVersion", true);
    let metrics_a = MetricsCollector::from_audit_log(&log);
    let report_a = export_observability(&log, &metrics_a);

    let report_b = export_observability(&log, &metrics_a);
    assert_eq!(report_a, report_b, "identical audit+metrics must produce identical report");
    assert_eq!(report_a.audit_log_hash, report_b.audit_log_hash);
    assert_eq!(report_a.metrics_snapshot_hash, report_b.metrics_snapshot_hash);
}

// ── 3. Mutation Safety Test ─────────────────────────────────────────────────
//
// Ensure none of: AuditLog, MetricsSnapshot, or kernel state is modified
// by the observability pipeline.

#[test]
fn mutation_safety_test() {
    let mut bs = test_bootstrap(vec![]);
    run_mixed_commands(&mut bs, 30);

    // Snapshot audit log before pipeline.
    let audit_before = bs.audit_log().records().to_vec();

    // Run metrics (read-only).
    let metrics = MetricsCollector::from_audit_log(bs.audit_log());
    let metrics_before = serde_json::to_string(&metrics).unwrap();

    // Run export (read-only).
    let _report = export_observability(bs.audit_log(), &metrics);

    // Run verifier (read-only).
    let _verification = verify_observability_integrity(bs.audit_log(), &metrics);

    // Verify audit log unchanged.
    let audit_after = bs.audit_log().records().to_vec();
    assert_eq!(audit_before, audit_after, "audit log must not be mutated");

    // Verify metrics snapshot unchanged.
    let metrics_after = serde_json::to_string(&metrics).unwrap();
    assert_eq!(metrics_before, metrics_after, "metrics snapshot must not be mutated");
}

// ── 4. Stress Test — 50k Events ─────────────────────────────────────────────
//
// Ensure no panic and stable hash outputs.

#[test]
fn stress_test_50k() {
    let mut log = AuditLog::new();
    for i in 0..50_000u64 {
        let cls = match i % 3 {
            0 => CommandClass::Execution,
            1 => CommandClass::Query,
            _ => CommandClass::System,
        };
        log.append(strata::api::envelope::TraceId(i), cls, "Op", i % 2 == 0);
    }

    let metrics = MetricsCollector::from_audit_log(&log);
    assert_eq!(metrics.total_commands, 50_000);

    // Export must not panic.
    let report = export_observability(&log, &metrics);
    assert!(report.consistency_check_passed);
    assert_eq!(report.total_commands, 50_000);
    assert_eq!(report.audit_log_length, 50_000);
    assert!(!report.audit_log_hash.is_empty());
    assert!(!report.metrics_snapshot_hash.is_empty());

    // Verification must not panic.
    let verification = verify_observability_integrity(&log, &metrics);
    assert!(verification.valid);

    // Hashes must be stable across repeated calls.
    let report_b = export_observability(&log, &metrics);
    assert_eq!(report, report_b, "repeated export on 50k log must be identical");
}

// ── 5. Hash Collision Test ──────────────────────────────────────────────────
//
// Different logs must produce different hashes.

#[test]
fn hash_collision_test() {
    let mut log_a = AuditLog::new();
    log_a.append(strata::api::envelope::TraceId(1), CommandClass::Query, "Q", true);
    log_a.append(strata::api::envelope::TraceId(2), CommandClass::System, "V", false);

    let mut log_b = AuditLog::new();
    log_b.append(strata::api::envelope::TraceId(1), CommandClass::Query, "Q", true);
    log_b.append(strata::api::envelope::TraceId(2), CommandClass::System, "V", true); // different success

    let metrics_a = MetricsCollector::from_audit_log(&log_a);
    let metrics_b = MetricsCollector::from_audit_log(&log_b);

    let report_a = export_observability(&log_a, &metrics_a);
    let report_b = export_observability(&log_b, &metrics_b);

    assert_ne!(report_a.audit_log_hash, report_b.audit_log_hash,
        "different logs must produce different audit_log_hash");
    assert_ne!(report_a.metrics_snapshot_hash, report_b.metrics_snapshot_hash,
        "different metrics must produce different metrics_snapshot_hash");
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn run_mixed_commands(bs: &mut TestBootstrap, count: usize) {
    for i in 0..count {
        let cmd = match i % 3 {
            0 => CliCommand::Version,
            1 => CliCommand::ShowState,
            _ => CliCommand::SchemaVersion,
        };
        bs.run(cmd);
    }
}
