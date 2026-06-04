use crate::kernel::*;
use crate::triage::*;
use std::collections::BTreeMap;

// ── Overlapping Condition Profiles ───────────────────────────────────────

pub fn build_entropy_profiles() -> Vec<ConditionProfile> {
    vec![
        ConditionProfile {
            name: "metabolic_syndrome",
            prevalence: 0.15,
            symptoms: BTreeMap::from([
                ("fatigue", 0.7),
                ("weight_gain", 0.8),
                ("high_blood_pressure", 0.9),
                ("insulin_resistance", 0.85),
                ("abdominal_obesity", 0.85),
            ]),
        },
        ConditionProfile {
            name: "thyroid_dysfunction",
            prevalence: 0.10,
            symptoms: BTreeMap::from([
                ("fatigue", 0.8),
                ("weight_change", 0.7),
                ("heart_palpitations", 0.65),
                ("temperature_sensitivity", 0.75),
                ("mood_changes", 0.6),
            ]),
        },
        ConditionProfile {
            name: "chronic_fatigue",
            prevalence: 0.08,
            symptoms: BTreeMap::from([
                ("fatigue", 1.0),
                ("muscle_pain", 0.8),
                ("joint_pain", 0.6),
                ("sleep_disturbance", 0.85),
                ("cognitive_fog", 0.7),
                ("headache", 0.5),
            ]),
        },
        ConditionProfile {
            name: "fibromyalgia",
            prevalence: 0.05,
            symptoms: BTreeMap::from([
                ("fatigue", 0.9),
                ("muscle_pain", 0.9),
                ("joint_pain", 0.7),
                ("sleep_disturbance", 0.8),
                ("cognitive_fog", 0.6),
                ("mood_changes", 0.5),
            ]),
        },
        ConditionProfile {
            name: "autoimmune",
            prevalence: 0.07,
            symptoms: BTreeMap::from([
                ("fatigue", 0.85),
                ("joint_pain", 0.8),
                ("fever_mild", 0.6),
                ("skin_rash", 0.7),
                ("inflammation", 0.65),
                ("weight_change", 0.4),
            ]),
        },
        ConditionProfile {
            name: "anxiety",
            prevalence: 0.15,
            symptoms: BTreeMap::from([
                ("fatigue", 0.6),
                ("heart_palpitations", 0.85),
                ("sleep_disturbance", 0.7),
                ("mood_changes", 0.8),
                ("headache", 0.55),
                ("chest_tightness", 0.75),
            ]),
        },
    ]
}

// ── Entropy Scenario ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EntropyScenario {
    pub name: &'static str,
    pub description: &'static str,
    pub symptoms: Vec<&'static str>,
    pub ambiguity_rating: &'static str,   // "low" / "medium" / "high" / "total"
    pub expected_primary: Option<&'static str>,
    pub has_conflicting: bool,
}

pub fn build_entropy_scenarios() -> Vec<EntropyScenario> {
    vec![
        EntropyScenario {
            name: "metabolic_clear",
            description: "fatigue + weight gain + high BP + insulin resistance",
            symptoms: vec!["fatigue", "weight_gain", "high_blood_pressure", "insulin_resistance"],
            ambiguity_rating: "low",
            expected_primary: Some("metabolic_syndrome"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "anxiety_vs_thyroid",
            description: "fatigue + mood changes + sleep disturbance + heart palpitations",
            symptoms: vec!["fatigue", "mood_changes", "sleep_disturbance", "heart_palpitations"],
            ambiguity_rating: "high",
            expected_primary: Some("anxiety"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "cfs_vs_fibro",
            description: "muscle pain + joint pain + cognitive fog + fatigue",
            symptoms: vec!["muscle_pain", "joint_pain", "cognitive_fog", "fatigue"],
            ambiguity_rating: "high",
            expected_primary: Some("chronic_fatigue"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "autoimmune_moderate",
            description: "fatigue + joint pain + skin rash + mild fever",
            symptoms: vec!["fatigue", "joint_pain", "skin_rash", "fever_mild"],
            ambiguity_rating: "medium",
            expected_primary: Some("autoimmune"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "anxiety_clear",
            description: "fatigue + heart palpitations + chest tightness + sleep disturbance",
            symptoms: vec!["fatigue", "heart_palpitations", "chest_tightness", "sleep_disturbance"],
            ambiguity_rating: "low",
            expected_primary: Some("anxiety"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "metabolic_partial",
            description: "fatigue + insulin resistance + abdominal obesity (no weight gain)",
            symptoms: vec!["fatigue", "insulin_resistance", "abdominal_obesity"],
            ambiguity_rating: "medium",
            expected_primary: Some("metabolic_syndrome"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "thyroid_mood",
            description: "temperature sensitivity + weight change + fatigue + mood changes",
            symptoms: vec!["temperature_sensitivity", "weight_change", "fatigue", "mood_changes"],
            ambiguity_rating: "medium",
            expected_primary: Some("thyroid_dysfunction"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "pain_fatigue_ambiguous",
            description: "muscle pain + sleep disturbance + fatigue (no joint pain, no cognitive fog)",
            symptoms: vec!["muscle_pain", "sleep_disturbance", "fatigue"],
            ambiguity_rating: "high",
            expected_primary: Some("fibromyalgia"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "autoimmune_inflammatory",
            description: "joint pain + inflammation + fatigue + skin rash",
            symptoms: vec!["joint_pain", "inflammation", "fatigue", "skin_rash"],
            ambiguity_rating: "medium",
            expected_primary: Some("autoimmune"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "fatigue_only",
            description: "fatigue only — all conditions share fatigue",
            symptoms: vec!["fatigue"],
            ambiguity_rating: "total",
            expected_primary: None,
            has_conflicting: false,
        },
        EntropyScenario {
            name: "metabolic_no_fatigue",
            description: "high BP + abdominal obesity + insulin resistance (no fatigue)",
            symptoms: vec!["high_blood_pressure", "abdominal_obesity", "insulin_resistance"],
            ambiguity_rating: "low",
            expected_primary: Some("metabolic_syndrome"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "anxiety_no_fatigue",
            description: "heart palpitations + mood changes + chest tightness (no fatigue)",
            symptoms: vec!["heart_palpitations", "mood_changes", "chest_tightness"],
            ambiguity_rating: "low",
            expected_primary: Some("anxiety"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "autoimmune_weight",
            description: "fatigue + joint pain + weight change + skin rash",
            symptoms: vec!["fatigue", "joint_pain", "weight_change", "skin_rash"],
            ambiguity_rating: "medium",
            expected_primary: Some("autoimmune"),
            has_conflicting: false,
        },
        EntropyScenario {
            name: "cfs_cognitive",
            description: "muscle pain + headache + cognitive fog + fatigue",
            symptoms: vec!["muscle_pain", "headache", "cognitive_fog", "fatigue"],
            ambiguity_rating: "high",
            expected_primary: Some("chronic_fatigue"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "thyroid_anxiety_mixed",
            description: "mood changes + sleep disturbance + fatigue + heart palpitations",
            symptoms: vec!["mood_changes", "sleep_disturbance", "fatigue", "heart_palpitations"],
            ambiguity_rating: "high",
            expected_primary: Some("anxiety"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "stress_response",
            description: "fatigue + high BP + heart palpitations + chest tightness",
            symptoms: vec!["fatigue", "high_blood_pressure", "heart_palpitations", "chest_tightness"],
            ambiguity_rating: "high",
            expected_primary: Some("anxiety"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "broad_pain_fatigue",
            description: "muscle pain + joint pain + sleep disturbance + cognitive fog + fatigue",
            symptoms: vec!["muscle_pain", "joint_pain", "sleep_disturbance", "cognitive_fog", "fatigue"],
            ambiguity_rating: "high",
            expected_primary: Some("chronic_fatigue"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "metabolic_thyroid_overlap",
            description: "weight gain + fatigue + temperature sensitivity + mood changes",
            symptoms: vec!["weight_gain", "fatigue", "temperature_sensitivity", "mood_changes"],
            ambiguity_rating: "high",
            expected_primary: Some("metabolic_syndrome"),
            has_conflicting: true,
        },
        EntropyScenario {
            name: "empty_presentation",
            description: "no symptoms at all",
            symptoms: vec![],
            ambiguity_rating: "total",
            expected_primary: None,
            has_conflicting: false,
        },
        EntropyScenario {
            name: "autoimmune_full",
            description: "fatigue + joint pain + inflammation + skin rash + mild fever + muscle pain",
            symptoms: vec!["fatigue", "joint_pain", "inflammation", "skin_rash", "fever_mild", "muscle_pain"],
            ambiguity_rating: "low",
            expected_primary: Some("autoimmune"),
            has_conflicting: false,
        },
    ]
}

// ── Measurement Types ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EntropyResult {
    pub scenario: EntropyScenario,
    pub strata_result: Vec<(&'static str, u64, Confidence)>,
    pub nb_result: Vec<(&'static str, f64)>,
    pub strata_correct: bool,
    pub nb_correct: bool,
    pub event_count: usize,
    pub evidence_strength_range: (u64, u64),  // (min, max) evidence across conditions
    pub belief_spread: usize,  // how many conditions had any evidence
}

#[derive(Debug, Clone)]
pub struct BeliefTrace {
    pub step: usize,
    pub step_label: String,
    pub belief_distribution: Vec<(&'static str, u64, Confidence)>,
}

#[derive(Debug, Clone)]
pub struct StepwiseResult {
    pub scenario: EntropyScenario,
    pub traces: Vec<BeliefTrace>,
    pub final_result: EntropyResult,
    pub oscillation_detected: bool,
    pub final_flip_count: usize,
}

// ── Run a single entropy scenario ────────────────────────────────────────

pub fn diagnose_with_profiles(
    profiles: &[ConditionProfile],
    symptoms: &[&str],
) -> Vec<(&'static str, u64, Confidence)> {
    let mut results = Vec::new();
    for profile in profiles {
        let mut matched = 0u64;
        let mut total_strength = 0u64;
        for symptom in symptoms {
            if let Some(&prob) = profile.symptoms.get(symptom) {
                matched += 1;
                let strength = if prob >= 0.8 { 3 } else if prob >= 0.5 { 2 } else { 1 };
                total_strength += strength;
            }
        }
        if matched == 0 {
            continue;
        }
        let confidence = match total_strength {
            1..=2 => Confidence::Low,
            3..=5 => Confidence::Medium,
            _ => Confidence::High,
        };
        results.push((profile.name, matched, confidence));
    }
    results.sort_by(|a, b| {
        b.2.cmp(&a.2).then_with(|| b.1.cmp(&a.1))
    });
    results
}

// ── E7: Run all entropy scenarios ────────────────────────────────────────

pub fn run_e7_test(profiles: &[ConditionProfile]) -> Vec<EntropyResult> {
    let scenarios = build_entropy_scenarios();
    let nb = NaiveBayesClassifier::new(profiles.to_vec());
    let mut results = Vec::new();

    for scenario in &scenarios {
        let strata = diagnose_with_profiles(profiles, &scenario.symptoms);
        let nb_result = nb.diagnose(&scenario.symptoms);

        let strata_correct = match scenario.expected_primary {
            Some(expected) => strata.first().map(|r| r.0 == expected).unwrap_or(false),
            None => strata.is_empty(),
        };
        let nb_correct = match scenario.expected_primary {
            Some(expected) => nb_result.first().map(|r| r.0 == expected).unwrap_or(false),
            None => false,
        };

        let (min_ev, max_ev) = if strata.is_empty() {
            (0, 0)
        } else {
            let evs: Vec<u64> = strata.iter().map(|r| r.1).collect();
            (*evs.iter().min().unwrap_or(&0), *evs.iter().max().unwrap_or(&0))
        };

        results.push(EntropyResult {
            scenario: scenario.clone(),
            belief_spread: strata.len(),
            strata_result: strata,
            nb_result: nb_result,
            strata_correct,
            nb_correct,
            event_count: scenario.symptoms.len() * 3, // approximate
            evidence_strength_range: (min_ev, max_ev),
        });
    }

    results
}

// ── Stepwise belief tracing (for oscillation detection) ──────────────────

pub fn run_e7_stepwise(profiles: &[ConditionProfile]) -> Vec<StepwiseResult> {
    let scenarios = build_entropy_scenarios();
    let mut step_results = Vec::new();

    for scenario in &scenarios {
        if scenario.symptoms.is_empty() {
            continue;
        }
        let mut traces = Vec::new();
        let mut prev_top: Option<&'static str> = None;
        let mut flips = 0;

        for step in 0..scenario.symptoms.len() {
            let current_symptoms = &scenario.symptoms[0..=step];
            let label = format!("+{}", scenario.symptoms[step]);
            let dist = diagnose_with_profiles(profiles, current_symptoms);
            let top = dist.first().map(|r| r.0);

            if let (Some(p), Some(c)) = (prev_top, top) {
                if p != c {
                    flips += 1;
                }
            }
            prev_top = top;

            traces.push(BeliefTrace {
                step,
                step_label: label,
                belief_distribution: dist,
            });
        }

        let final_result = run_e7_test(profiles).into_iter()
            .find(|r| r.scenario.name == scenario.name)
            .unwrap();

        step_results.push(StepwiseResult {
            scenario: scenario.clone(),
            traces,
            final_result,
            oscillation_detected: flips > 0,
            final_flip_count: flips,
        });
    }

    step_results
}

// ── E8: Order sensitivity test ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OrderSensitivityResult {
    pub scenario_name: &'static str,
    pub permutations: usize,
    pub identical_count: usize,
    pub divergent_count: usize,
    pub variance: f64,
    pub is_order_dependent: bool,
}

pub fn run_e8_test(profiles: &[ConditionProfile], permutations: usize) -> Vec<OrderSensitivityResult> {
    let scenarios = build_entropy_scenarios();
    let mut results = Vec::new();

    for scenario in &scenarios {
        if scenario.symptoms.len() < 2 {
            continue;
        }
        let mut outcomes: Vec<Vec<(&'static str, u64, Confidence)>> = Vec::new();

        for _ in 0..permutations {
            let mut shuffled = scenario.symptoms.clone();
            let mut rng_state = outcomes.len() as u64;
            let len = shuffled.len();
            for i in (1..len).rev() {
                rng_state = fast_deterministic_rng(rng_state);
                let j = (rng_state % (i as u64 + 1)) as usize;
                shuffled.swap(i, j);
            }
            let result = diagnose_with_profiles(profiles, &shuffled);
            outcomes.push(result);
        }

        // Compare all outcomes to the first one
        let reference = &outcomes[0];
        let mut divergent = 0;
        for outcome in &outcomes[1..] {
            let top_ref = reference.first().map(|r| r.0);
            let top_out = outcome.first().map(|r| r.0);
            if top_ref != top_out {
                divergent += 1;
            }
        }

        results.push(OrderSensitivityResult {
            scenario_name: scenario.name,
            permutations,
            identical_count: permutations - divergent,
            divergent_count: divergent,
            variance: divergent as f64 / permutations as f64,
            is_order_dependent: divergent > 0,
        });
    }

    results
}

fn fast_deterministic_rng(seed: u64) -> u64 {
    // Simple xorshift — deterministic, no external deps
    let mut x = seed.wrapping_add(0x9e3779b97f4a7c15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^= x >> 31;
    x
}

// ── E9: Conflicting evidence resolution ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct ConflictResult {
    pub scenario: EntropyScenario,
    pub strata_top: Option<&'static str>,
    pub strata_second: Option<&'static str>,
    pub confidence_gap: &'static str,  // "large" / "small" / "tie"
    pub resolution_strategy: &'static str,
    pub is_stable: bool,
}

pub fn run_e9_test(profiles: &[ConditionProfile]) -> Vec<ConflictResult> {
    let scenarios = build_entropy_scenarios();
    let conflicting: Vec<&EntropyScenario> = scenarios.iter()
        .filter(|s| s.has_conflicting && s.symptoms.len() >= 3)
        .collect();

    let mut results = Vec::new();

    for scenario in conflicting {
        let strata = diagnose_with_profiles(profiles, &scenario.symptoms);
        let top = strata.first().map(|r| r.0);
        let second = strata.get(1).map(|r| r.0);
        let gap = if strata.len() < 2 {
            "large"
        } else {
            let c1 = strata[0].2.clone();
            let c2 = strata[1].2.clone();
            match (c1, c2) {
                (Confidence::High, Confidence::Low) | (Confidence::Medium, Confidence::Low) => "large",
                (Confidence::High, Confidence::Medium) => "small",
                _ => "tie",
            }
        };

        results.push(ConflictResult {
            scenario: (*scenario).clone(),
            strata_top: top,
            strata_second: second,
            confidence_gap: gap,
            resolution_strategy: match (top, second, gap) {
                (Some(_), Some(_), "large") => "decisive",
                (Some(_), Some(_), "small") => "leaning",
                (Some(_), Some(_), "tie") => "arbitrary_tie",
                (Some(_), None, _) => "uncontested",
                (None, _, _) => "indecision",
                _ => "unknown",
            },
            is_stable: gap != "tie",
        });
    }

    results
}

// ── E10: Information loss comparison ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InformationLossResult {
    pub scenario_name: &'static str,
    pub strata_winner: Option<&'static str>,
    pub nb_winner: Option<&'static str>,
    pub strata_confidence: Option<&'static str>,
    pub nb_probability: f64,
    pub decision_diverges: bool,
}

pub fn run_e10_test(profiles: &[ConditionProfile]) -> Vec<InformationLossResult> {
    let scenarios = build_entropy_scenarios();
    let nb = NaiveBayesClassifier::new(profiles.to_vec());
    let mut results = Vec::new();

    for scenario in &scenarios {
        let strata = diagnose_with_profiles(profiles, &scenario.symptoms);
        let nb_out = nb.diagnose(&scenario.symptoms);

        let strata_winner = strata.first().map(|r| r.0);
        let strata_conf = strata.first().map(|r| r.2.as_str());
        let nb_winner = nb_out.first().map(|r| r.0);
        let nb_prob = nb_out.first().map(|r| r.1.exp()).unwrap_or(0.0);

        results.push(InformationLossResult {
            scenario_name: scenario.name,
            strata_winner,
            nb_winner,
            strata_confidence: strata_conf,
            nb_probability: nb_prob,
            decision_diverges: strata_winner != nb_winner,
        });
    }

    results
}
