// ── Phase E2: Metrics Projection Integration Tests ──────────────────────────

use strata::observability::{AuditLog, MetricsCollector};

// ── 1. Empty Log ────────────────────────────────────────────────────────────

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

// ── 2. Single Command ───────────────────────────────────────────────────────

#[test]
fn single_command() {
    let mut log = AuditLog::new();
    log.append(
        strata::api::envelope::TraceId(1),
        strata::api::command::CommandClass::Query,
        "QueryState",
        true,
    );
    let m = MetricsCollector::from_audit_log(&log);
    assert_eq!(m.total_commands, 1);
    assert_eq!(m.query_commands, 1);
    assert_eq!(m.execution_commands, 0);
    assert_eq!(m.system_commands, 0);
    assert_eq!(m.successful_commands, 1);
    assert_eq!(m.failed_commands, 0);
    assert_eq!(m.last_sequence, 1);
}

// ── 3. Mixed Command Classes ────────────────────────────────────────────────

#[test]
fn mixed_command_classes() {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), strata::api::command::CommandClass::Execution, "Ingest", true);
    log.append(strata::api::envelope::TraceId(2), strata::api::command::CommandClass::Query, "QueryState", true);
    log.append(strata::api::envelope::TraceId(3), strata::api::command::CommandClass::System, "GetVersion", true);
    log.append(strata::api::envelope::TraceId(4), strata::api::command::CommandClass::Execution, "Ingest", false);
    log.append(strata::api::envelope::TraceId(5), strata::api::command::CommandClass::Query, "ListNodes", true);

    let m = MetricsCollector::from_audit_log(&log);
    assert_eq!(m.total_commands, 5);
    assert_eq!(m.execution_commands, 2);
    assert_eq!(m.query_commands, 2);
    assert_eq!(m.system_commands, 1);
    assert_eq!(m.successful_commands, 4);
    assert_eq!(m.failed_commands, 1);
    assert_eq!(m.last_sequence, 5);
}

// ── 4. Success/Failure Counting ─────────────────────────────────────────────

#[test]
fn success_failure_counting() {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), strata::api::command::CommandClass::Query, "Q", true);
    log.append(strata::api::envelope::TraceId(2), strata::api::command::CommandClass::System, "V", false);
    log.append(strata::api::envelope::TraceId(3), strata::api::command::CommandClass::Execution, "I", true);
    log.append(strata::api::envelope::TraceId(4), strata::api::command::CommandClass::Query, "L", false);
    log.append(strata::api::envelope::TraceId(5), strata::api::command::CommandClass::System, "S", true);

    let m = MetricsCollector::from_audit_log(&log);
    assert_eq!(m.total_commands, 5);
    assert_eq!(m.successful_commands, 3);
    assert_eq!(m.failed_commands, 2);
    assert_eq!(m.successful_commands + m.failed_commands, m.total_commands);
}

// ── 5. Determinism ──────────────────────────────────────────────────────────

#[test]
fn determinism() {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), strata::api::command::CommandClass::Execution, "Ingest", true);
    log.append(strata::api::envelope::TraceId(2), strata::api::command::CommandClass::Query, "QueryState", true);

    let a = MetricsCollector::from_audit_log(&log);
    let b = MetricsCollector::from_audit_log(&log);
    assert_eq!(a, b);
}

// ── 6. Append-Only Growth ───────────────────────────────────────────────────

#[test]
fn append_only_growth() {
    let mut log = AuditLog::new();

    let m0 = MetricsCollector::from_audit_log(&log);
    assert_eq!(m0.total_commands, 0);

    log.append(strata::api::envelope::TraceId(1), strata::api::command::CommandClass::Query, "Q", true);
    let m1 = MetricsCollector::from_audit_log(&log);
    assert_eq!(m1.total_commands, 1);

    log.append(strata::api::envelope::TraceId(2), strata::api::command::CommandClass::System, "V", false);
    let m2 = MetricsCollector::from_audit_log(&log);
    assert_eq!(m2.total_commands, 2);
    assert_eq!(m2.query_commands, 1);
    assert_eq!(m2.system_commands, 1);
    assert_eq!(m1.total_commands, 1, "prior snapshot must remain valid");
}

// ── 7. Snapshot Reproducibility ─────────────────────────────────────────────

#[test]
fn snapshot_reproducibility() {
    let mut log = AuditLog::new();
    log.append(strata::api::envelope::TraceId(1), strata::api::command::CommandClass::Execution, "Ingest", false);
    log.append(strata::api::envelope::TraceId(2), strata::api::command::CommandClass::Query, "QueryState", true);
    log.append(strata::api::envelope::TraceId(3), strata::api::command::CommandClass::System, "GetVersion", true);
    log.append(strata::api::envelope::TraceId(4), strata::api::command::CommandClass::Query, "ListNodes", true);
    log.append(strata::api::envelope::TraceId(5), strata::api::command::CommandClass::System, "ValidateLog", false);

    let snapshots: Vec<_> = (0..10)
        .map(|_| MetricsCollector::from_audit_log(&log))
        .collect();
    for pair in snapshots.windows(2) {
        assert_eq!(pair[0], pair[1]);
    }
}

// ── 8. Bootstrap Integration (real execution path) ──────────────────────────

#[test]
fn metrics_from_bootstrap_audit_log() {
    let mut bs = strata::test_utils::test_bootstrap(vec![]);
    bs.run(strata::cli::CliCommand::Version);
    bs.run(strata::cli::CliCommand::ShowState);
    bs.run(strata::cli::CliCommand::CreateNode { id: "n1".into() });

    let m = MetricsCollector::from_audit_log(bs.audit_log());
    assert_eq!(m.total_commands, 3);
    // Version → System, ShowState → Query, CreateNode → Execution
    assert_eq!(m.system_commands, 1);
    assert_eq!(m.query_commands, 1);
    assert_eq!(m.execution_commands, 1);
    assert_eq!(m.last_sequence, 3);
}
