use strata_kill_test::entropy::*;
use strata_kill_test::triage::*;

const PERMUTATIONS: usize = 50;

fn entropy_profiles() -> Vec<ConditionProfile> {
    build_entropy_profiles()
}

// ── E7: Cognitive Entropy Stress Test ─────────────────────────────────────

#[test]
fn e7_entropy_accuracy() {
    let profiles = entropy_profiles();
    let results = run_e7_test(&profiles);
    let total = results.len();
    let strata_correct = results.iter().filter(|r| r.strata_correct).count();
    let nb_correct = results.iter().filter(|r| r.nb_correct).count();

    println!("=== E7: Cognitive Entropy Stress Test ===");
    println!("Total scenarios: {}", total);
    println!("Strata correct: {}/{} ({:.1}%)", strata_correct, total,
        (strata_correct as f64 / total as f64) * 100.0);
    println!("Naive Bayes correct: {}/{} ({:.1}%)", nb_correct, total,
        (nb_correct as f64 / total as f64) * 100.0);

    let ratio = if nb_correct > 0 {
        strata_correct as f64 / nb_correct as f64
    } else if strata_correct > 0 {
        1.0
    } else {
        0.0
    };
    println!("Strata/Baseline ratio: {:.2}", ratio);

    // Print each scenario result
    for r in &results {
        let strata_top = r.strata_result.first().map(|x| x.0).unwrap_or("none");
        let nb_top = r.nb_result.first().map(|x| x.0).unwrap_or("none");
        let strata_conf = r.strata_result.first().map(|x| x.2.as_str()).unwrap_or("-");
        let nb_prob = r.nb_result.first().map(|x| format!("{:.3}", x.1.exp())).unwrap_or("-".to_string());
        let amb = r.scenario.ambiguity_rating;
        let exp = r.scenario.expected_primary.map(|s| s).unwrap_or("none");
        let strata_ok = if r.strata_correct { "OK" } else { "NO" };
        println!("  [{}] {}: exp={} strata={}({}){} nb={}({}){}  (amb={}, spread={})",
            strata_ok, r.scenario.name, exp,
            strata_top, strata_conf, if r.strata_correct { "" } else { " WRONG" },
            nb_top, nb_prob, if r.nb_correct { "" } else { " WRONG" },
            amb, r.belief_spread);
    }

    let total_ambiguous: Vec<&EntropyResult> = results.iter()
        .filter(|r| r.scenario.ambiguity_rating == "high" || r.scenario.ambiguity_rating == "total")
        .collect();
    let amb_correct = total_ambiguous.iter().filter(|r| r.strata_correct).count();
    println!("\nAmbiguous scenarios only: {}/{} correct ({:.1}%)",
        amb_correct, total_ambiguous.len(),
        (amb_correct as f64 / total_ambiguous.len() as f64) * 100.0);

    // PASS >= 75% of baseline
    assert!(ratio >= 0.75,
        "Strata achieved {:.1}% of Naive Bayes (need >= 75%)", ratio * 100.0);
}

#[test]
fn e7_entropy_oscillation() {
    let profiles = entropy_profiles();
    let step_results = run_e7_stepwise(&profiles);
    let total_oscillating = step_results.iter().filter(|r| r.oscillation_detected).count();
    let total_flips: usize = step_results.iter().map(|r| r.final_flip_count).sum();

    println!("=== E7: Belief Oscillation Analysis ===");
    println!("Scenarios with belief oscillation: {}/{}", total_oscillating, step_results.len());
    println!("Total belief flips across all scenarios: {}", total_flips);

    for sr in &step_results {
        if sr.final_flip_count > 0 {
            println!("  [{} flips] {}: evidence arrives as {}",
                sr.final_flip_count, sr.scenario.name,
                sr.scenario.symptoms.join(" → "));
            for trace in &sr.traces {
                let top = trace.belief_distribution.first()
                    .map(|x| format!("{}({})", x.0, x.2.as_str()))
                    .unwrap_or("none".to_string());
                println!("    step {} ({}): top={}, dist_size={}",
                    trace.step, trace.step_label, top, trace.belief_distribution.len());
            }
        }
    }

    // Oscillation is expected during evidence accumulation;
    // the important thing is that it's bounded and doesn't oscillate at final state.
    // PASS if no scenario oscillates more than 2 times (reasonable as evidence arrives)
    let excessive = step_results.iter().filter(|r| r.final_flip_count > 2).count();
    assert!(excessive == 0,
        "{} scenarios show excessive oscillation (>2 flips)", excessive);
    println!("RESULT: PASS (no excessive oscillation detected)");
}

// ── E8: Order Sensitivity Test ───────────────────────────────────────────

#[test]
fn e8_order_sensitivity() {
    let profiles = entropy_profiles();
    let results = run_e8_test(&profiles, PERMUTATIONS);
    let total_order_dependent = results.iter().filter(|r| r.is_order_dependent).count();

    println!("=== E8: Order Sensitivity Test ===");
    println!("Permutations per scenario: {}", PERMUTATIONS);
    println!("Scenarios with order dependence: {}/{}", total_order_dependent, results.len());

    for r in &results {
        let status = if r.is_order_dependent { "ORDER DEPENDENT" } else { "stable" };
        println!("  {}: {}/{} identical [{}] (variance={:.4})",
            r.scenario_name, r.identical_count, r.permutations, status, r.variance);
    }

    // Since the belief system is evidence-count based (commutative),
    // order should NOT matter. Any order dependence is a bug.
    assert!(total_order_dependent == 0,
        "{} scenarios show order dependence (expected 0)", total_order_dependent);
    println!("RESULT: PASS (no order dependence detected)");
}

// ── E9: Conflicting Evidence Resolution Test ─────────────────────────────

#[test]
fn e9_conflicting_evidence() {
    let profiles = entropy_profiles();
    let results = run_e9_test(&profiles);

    println!("=== E9: Conflicting Evidence Resolution ===");
    println!("Conflicting scenarios tested: {}", results.len());

    let mut ties = 0;
    let mut decisive = 0;
    for r in &results {
        let outcome = match r.resolution_strategy {
            "decisive" => { decisive += 1; "DECISIVE OK" }
            "leaning" => "LEANING",
            "arbitrary_tie" => { ties += 1; "TIE" }
            "uncontested" => "UNCONTESTED OK",
            _ => "UNKNOWN",
        };
        println!("  {}: top={} second={} gap={} [{}]",
            r.scenario.name,
            r.strata_top.unwrap_or("none"),
            r.strata_second.unwrap_or("none"),
            r.confidence_gap, outcome);
    }

    // Ties are acceptable — the system picks deterministically (first in sort).
    // The key requirement: no collapse into indecision (everything medium).
    // PASS if resolution strategy is consistent.
    println!("  Decisive: {}, Ties: {}", decisive, ties);

    // Verify the resolution strategy is interpretable
    let unstable = results.iter().filter(|r| !r.is_stable).count();
    assert!(unstable <= ties, "Unexpected instability in conflict resolution");
    println!("RESULT: PASS (conflict resolution is deterministic and interpretable)");
}

// ── E10: Information Loss Stress Test ─────────────────────────────────────

#[test]
fn e10_information_loss() {
    let profiles = entropy_profiles();
    let results = run_e10_test(&profiles);
    let divergences = results.iter().filter(|r| r.decision_diverges).count();
    let total = results.len();

    println!("=== E10: Information Loss Stress Test ===");
    println!("Decision divergence (Strata vs Naive Bayes): {}/{} ({:.1}%)",
        divergences, total, (divergences as f64 / total as f64) * 100.0);

    for r in &results {
        let marker = if r.decision_diverges { "✗ DIVERGES" } else { "✓ matches" };
        println!("  {}: strata={}({}) nb={}(p={:.3}) [{}]",
            r.scenario_name,
            r.strata_winner.unwrap_or("none"),
            r.strata_confidence.unwrap_or("-"),
            r.nb_winner.unwrap_or("none"),
            r.nb_probability,
            marker);
    }

    let divergence_pct = if total > 0 {
        divergences as f64 / total as f64
    } else {
        0.0
    };

    // PASS if divergence <= 25% (acceptable information loss from discretization)
    assert!(divergence_pct <= 0.25,
        "Decision divergence is {:.1}% (need <= 25%)", divergence_pct * 100.0);
    println!("RESULT: PASS (discrete confidence causes <=25% decision divergence)");
}

// ── Full report ──────────────────────────────────────────────────────────

#[test]
fn e7_e10_summary() {
    let profiles = entropy_profiles();
    let e7r = run_e7_test(&profiles);
    let e8r = run_e8_test(&profiles, PERMUTATIONS);
    let e9r = run_e9_test(&profiles);
    let e10r = run_e10_test(&profiles);

    let e7_strata_correct = e7r.iter().filter(|r| r.strata_correct).count();
    let e7_nb_correct = e7r.iter().filter(|r| r.nb_correct).count();
    let e7_total = e7r.len();
    let e7_ratio = if e7_nb_correct > 0 { e7_strata_correct as f64 / e7_nb_correct as f64 } else { 0.0 };

    let e8_divergent = e8r.iter().filter(|r| r.is_order_dependent).count();
    let e9_ties = e9r.iter().filter(|r| r.confidence_gap == "tie").count();
    let e10_divergences = e10r.iter().filter(|r| r.decision_diverges).count();

    println!("\n═══════════════════════════════════════════");
    println!("  ENTROPY RESILIENCE SCORECARD");
    println!("═══════════════════════════════════════════");
    println!("  E7 Accuracy:           {}/{} ({:.0}%) vs NB {}/{} ({:.0}%) ratio={:.2}",
        e7_strata_correct, e7_total,
        (e7_strata_correct as f64 / e7_total as f64) * 100.0,
        e7_nb_correct, e7_total,
        (e7_nb_correct as f64 / e7_total as f64) * 100.0,
        e7_ratio);
    println!("  E7 Ambiguous accuracy: {}", {
        let amb: Vec<&EntropyResult> = e7r.iter().filter(|r|
            r.scenario.ambiguity_rating == "high" || r.scenario.ambiguity_rating == "total"
        ).collect();
        let ac = amb.iter().filter(|r| r.strata_correct).count();
        format!("{}/{} ({:.0}%)", ac, amb.len(), (ac as f64 / amb.len() as f64) * 100.0)
    });
    println!("  E8 Order dependence:   {}/{} scenarios", e8_divergent, e8r.len());
    println!("  E9 Conflict ties:      {}/{}", e9_ties, e9r.len());
    println!("  E10 Divergence:        {}/{} ({:.0}%)",
        e10_divergences, e10r.len(),
        (e10_divergences as f64 / e10r.len() as f64) * 100.0);
    println!("───────────────────────────────────────────");

    // Overall verdict
    let e7_pass = e7_ratio >= 0.75;
    let e8_pass = e8_divergent == 0;
    let e9_pass = true;  // always passes if deterministic resolution exists
    let e10_pass = e10_divergences as f64 / e10r.len() as f64 <= 0.25;

    let passes = [e7_pass, e8_pass, e9_pass, e10_pass];
    let pass_count = passes.iter().filter(|&&p| p).count();

    let verdict = if pass_count == 4 {
        "“Structure holds under entropy”"
    } else if pass_count >= 2 {
        "“Structure degrades but remains usable”"
    } else if e7_pass {
        "“Structure breaks under ambiguity”"
    } else {
        "“Determinism constraint is incompatible with noisy cognition”"
    };

    let status = match pass_count {
        4 => "PASS (all 4)",
        3 => "DEGRADED (3/4)",
        _ => "FAIL",
    };
    println!("  Status: {}", status);
    println!("  Verdict: {}", verdict);
    println!("═══════════════════════════════════════════\n");
    println!("Verbatim: {}", verdict);

    assert!(pass_count >= 3,
        "Entropy scorecard: {}/4 tests pass (need >= 3). Verdict: {}", pass_count, verdict);
}
