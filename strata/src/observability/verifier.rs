use crate::observability::{AuditLog, MetricsSnapshot};

/// A description of a single consistency violation found by the verifier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IntegrityViolation {
    /// A stable string identifying the violation type.
    pub violation_type: &'static str,
    /// Human-readable description of the mismatch.
    pub description: String,
}

/// The result of running `verify_observability_integrity`.
///
/// `valid` is `true` iff no violations were found.
/// `mismatches` is always sorted in a deterministic order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerificationResult {
    pub valid: bool,
    pub mismatches: Vec<IntegrityViolation>,
}

/// Verify consistency between an `AuditLog` and its `MetricsSnapshot`.
///
/// ## Rules
///
/// - Pure function: never mutates inputs.
/// - No kernel, replay, or engine access.
/// - Only compares derived observability structures.
/// - Mismatches are reported in a deterministic order (by violation type name).
///
/// ## Invariants
///
/// - I1: Observability is derivable only from the audit log.
/// - I2: Metrics are a projection — not a source of truth.
/// - I3: Verification never mutates system state.
/// - I4: Mismatch ordering is deterministic.
pub fn verify_observability_integrity(
    audit: &AuditLog,
    metrics: &MetricsSnapshot,
) -> VerificationResult {
    let mut mismatches: Vec<IntegrityViolation> = Vec::new();

    // ── 1. Count match ──────────────────────────────────────────────────────
    if metrics.total_commands != audit.count() as u64 {
        mismatches.push(IntegrityViolation {
            violation_type: "AuditMetricsCountMismatch",
            description: format!(
                "total_commands mismatch: metrics={}, audit_log_length={}",
                metrics.total_commands,
                audit.count(),
            ),
        });
    }

    // ── 2. Success/failure counts ───────────────────────────────────────────
    let (audit_success, audit_failure) = {
        let mut s = 0u64;
        let mut f = 0u64;
        for rec in audit.records() {
            if rec.success { s += 1; } else { f += 1; }
        }
        (s, f)
    };

    if metrics.successful_commands != audit_success {
        mismatches.push(IntegrityViolation {
            violation_type: "SuccessCountMismatch",
            description: format!(
                "successful_commands mismatch: metrics={}, audit_derived={}",
                metrics.successful_commands,
                audit_success,
            ),
        });
    }

    if metrics.failed_commands != audit_failure {
        mismatches.push(IntegrityViolation {
            violation_type: "FailureCountMismatch",
            description: format!(
                "failed_commands mismatch: metrics={}, audit_derived={}",
                metrics.failed_commands,
                audit_failure,
            ),
        });
    }

    // ── 3. Sequence drift ───────────────────────────────────────────────────
    let audit_last_seq = audit.records().last().map(|r| r.sequence).unwrap_or(0);
    if metrics.last_sequence != audit_last_seq {
        mismatches.push(IntegrityViolation {
            violation_type: "SequenceDriftDetected",
            description: format!(
                "last_sequence mismatch: metrics={}, audit_last_sequence={}",
                metrics.last_sequence,
                audit_last_seq,
            ),
        });
    }

    // ── 4. Missing audit records (sequence gaps) ────────────────────────────
    for (i, rec) in audit.records().iter().enumerate() {
        let expected_seq = (i + 1) as u64;
        if rec.sequence != expected_seq {
            mismatches.push(IntegrityViolation {
                violation_type: "MissingAuditRecord",
                description: format!(
                    "sequence gap at index {}: expected sequence {}, got {}",
                    i, expected_seq, rec.sequence,
                ),
            });
        }
    }

    // ── Sort deterministically by violation_type ────────────────────────────
    mismatches.sort_by(|a, b| a.violation_type.cmp(b.violation_type));

    VerificationResult {
        valid: mismatches.is_empty(),
        mismatches,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::command::CommandClass;
    use crate::api::envelope::TraceId;

    fn consistent_log_and_metrics() -> (AuditLog, MetricsSnapshot) {
        let mut log = AuditLog::new();
        log.append(TraceId(1), CommandClass::Execution, "Ingest", true);
        log.append(TraceId(2), CommandClass::Query, "QueryState", true);
        log.append(TraceId(3), CommandClass::System, "GetVersion", false);
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
        log.append(TraceId(1), CommandClass::Query, "Q", true);
        log.append(TraceId(2), CommandClass::Query, "Q", false);
        let metrics = MetricsSnapshot {
            total_commands: 1,    // wrong (audit has 2)
            execution_commands: 0,
            query_commands: 1,
            system_commands: 0,
            successful_commands: 2, // wrong (audit has 1)
            failed_commands: 0,     // wrong (audit has 1)
            last_sequence: 1,       // wrong (audit last is 2)
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
            total_commands: 0,
            execution_commands: 0,
            query_commands: 0,
            system_commands: 0,
            successful_commands: 0,
            failed_commands: 0,
            last_sequence: 0,
        };
        let result = verify_observability_integrity(&log, &metrics);
        assert!(result.valid);
    }

    #[test]
    fn large_log_stress_validation() {
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
        let result = verify_observability_integrity(&log, &metrics);
        assert!(result.valid, "large log must pass verification");
    }

    #[test]
    fn deterministic_mismatch_ordering() {
        // Multiple mismatches must always be reported in the same order.
        let log = AuditLog::new();  // empty audit
        let metrics = MetricsSnapshot {
            total_commands: 5,
            execution_commands: 0,
            query_commands: 0,
            system_commands: 0,
            successful_commands: 3,
            failed_commands: 2,
            last_sequence: 5,
        };
        let a = verify_observability_integrity(&log, &metrics);
        let b = verify_observability_integrity(&log, &metrics);
        assert_eq!(a.mismatches, b.mismatches);
        // Verify ordering: violation types are sorted alphabetically.
        let types: Vec<&str> = a.mismatches.iter().map(|v| v.violation_type).collect();
        let mut sorted = types.clone();
        sorted.sort();
        assert_eq!(types, sorted, "mismatches must be in deterministic order");
    }
}
