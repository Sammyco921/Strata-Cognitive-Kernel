use strata_kill_test::triage::*;

#[test]
fn e2_strata_vs_naive_bayes() {
    let results = run_e2_test();
    let total = results.len();
    let strata_correct = results.iter().filter(|r| r.strata_correct).count();
    let nb_correct = results.iter().filter(|r| r.nb_correct).count();

    println!("=== E2: Deterministic vs Naive Bayes ===");
    println!("Total test cases: {}", total);
    println!("Strata correct: {}/{} ({:.1}%)", strata_correct, total,
        (strata_correct as f64 / total as f64) * 100.0);
    println!("Naive Bayes correct: {}/{} ({:.1}%)", nb_correct, total,
        (nb_correct as f64 / total as f64) * 100.0);

    // PASS >= 80% of baseline accuracy
    let ratio = if nb_correct > 0 {
        (strata_correct as f64) / (nb_correct as f64)
    } else {
        // Both zero or NB baseline has no correct: use absolute comparison
        if strata_correct >= 1 { 1.0 } else { 0.0 }
    };
    println!("Strata/Baseline ratio: {:.2}", ratio);

    // Print detailed results
    for r in &results {
        let status = if r.strata_correct == r.nb_correct {
            if r.strata_correct { "BOTH OK" } else { "BOTH WRONG" }
        } else if r.strata_correct {
            "STRATA WINS"
        } else {
            "NB WINS"
        };
        println!("  {}: exp={} strata={:?} nb={:?} [{}]",
            r.test_case.name,
            r.test_case.expected_condition,
            r.strata_prediction.first().map(|x| x.0),
            r.nb_prediction.first().map(|x| x.0),
            status);
    }

    assert!(strata_correct as f64 >= nb_correct as f64 * 0.8,
        "Strata ({}/{}) achieved < 80% of Naive Bayes ({}/{})",
        strata_correct, total, nb_correct, total);
}
