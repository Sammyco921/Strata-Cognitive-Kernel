use crate::closure::types::{AuditResult, SubstrateClosureReport};
use crate::closure::dependency_audit::run_dependency_audit;
use crate::closure::surface_audit::run_surface_audit;
use crate::closure::determinism_audit::run_determinism_audit;
use crate::closure::layering_audit::run_layering_audit;

pub fn generate_closure_report() -> SubstrateClosureReport {
    let dep_result = run_dependency_audit();
    let surf_result = run_surface_audit();
    let det_result = run_determinism_audit();
    let layer_result = run_layering_audit();

    let mut results: Vec<AuditResult> = vec![dep_result, surf_result, det_result, layer_result];
    results.sort_by(|a, b| a.audit_name.cmp(&b.audit_name));

    SubstrateClosureReport::new(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_closure_report_generated() {
        let report = generate_closure_report();
        assert_eq!(report.total_audits, 4);
    }

    #[test]
    fn test_closure_report_deterministic() {
        let first = generate_closure_report();
        for _ in 0..50 {
            let next = generate_closure_report();
            assert_eq!(first.total_audits, next.total_audits);
            assert_eq!(first.passed_audits, next.passed_audits);
            assert_eq!(first.failed_audits, next.failed_audits);
            assert_eq!(first.closure_status, next.closure_status);
        }
    }

    #[test]
    fn test_closure_report_never_panics() {
        for _ in 0..100 {
            let _ = generate_closure_report();
        }
    }

    #[test]
    fn test_closure_report_sorted_results() {
        let report = generate_closure_report();
        for i in 1..report.results.len() {
            assert!(
                report.results[i - 1].audit_name <= report.results[i].audit_name,
                "results not sorted: {} > {}",
                report.results[i - 1].audit_name,
                report.results[i].audit_name
            );
        }
    }
}
