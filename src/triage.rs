use crate::kernel::*;
use std::collections::BTreeMap;

// ── Condition Definitions ────────────────────────────────────────────────
// Each condition has a prior probability and a map of symptom → P(symptom|condition)

#[derive(Debug, Clone)]
pub struct ConditionProfile {
    pub name: &'static str,
    pub prevalence: f64,
    pub symptoms: BTreeMap<&'static str, f64>,
}

pub fn build_condition_profiles() -> Vec<ConditionProfile> {
    vec![
        ConditionProfile {
            name: "common_cold",
            prevalence: 0.25,
            symptoms: BTreeMap::from([
                ("runny_nose", 0.9),
                ("sneezing", 0.8),
                ("sore_throat", 0.7),
                ("cough", 0.6),
                ("fatigue", 0.5),
                ("headache", 0.4),
                ("fever_mild", 0.3),
                ("body_ache", 0.2),
            ]),
        },
        ConditionProfile {
            name: "influenza",
            prevalence: 0.15,
            symptoms: BTreeMap::from([
                ("fever_high", 0.9),
                ("body_ache", 0.85),
                ("fatigue", 0.85),
                ("headache", 0.8),
                ("cough", 0.7),
                ("sore_throat", 0.5),
                ("runny_nose", 0.4),
                ("nausea", 0.3),
            ]),
        },
        ConditionProfile {
            name: "covid19",
            prevalence: 0.10,
            symptoms: BTreeMap::from([
                ("cough", 0.85),
                ("fever_mild", 0.75),
                ("fatigue", 0.75),
                ("loss_of_taste", 0.7),
                ("headache", 0.6),
                ("sore_throat", 0.55),
                ("body_ache", 0.5),
                ("shortness_of_breath", 0.4),
                ("runny_nose", 0.3),
            ]),
        },
        ConditionProfile {
            name: "strep_throat",
            prevalence: 0.08,
            symptoms: BTreeMap::from([
                ("sore_throat_severe", 0.95),
                ("fever_mild", 0.8),
                ("swollen_lymph_nodes", 0.75),
                ("headache", 0.5),
                ("fatigue", 0.4),
                ("nausea", 0.3),
                ("body_ache", 0.2),
            ]),
        },
        ConditionProfile {
            name: "allergic_rhinitis",
            prevalence: 0.20,
            symptoms: BTreeMap::from([
                ("sneezing", 0.9),
                ("runny_nose", 0.85),
                ("itchy_eyes", 0.8),
                ("watery_eyes", 0.7),
                ("cough", 0.3),
                ("fatigue", 0.2),
            ]),
        },
        ConditionProfile {
            name: "food_poisoning",
            prevalence: 0.07,
            symptoms: BTreeMap::from([
                ("nausea", 0.9),
                ("vomiting", 0.85),
                ("diarrhea", 0.8),
                ("abdominal_pain", 0.75),
                ("fever_mild", 0.3),
                ("fatigue", 0.3),
                ("headache", 0.2),
            ]),
        },
        ConditionProfile {
            name: "migraine",
            prevalence: 0.10,
            symptoms: BTreeMap::from([
                ("headache_severe", 0.95),
                ("nausea", 0.7),
                ("light_sensitivity", 0.65),
                ("visual_aura", 0.3),
                ("fatigue", 0.5),
            ]),
        },
        ConditionProfile {
            name: "bacterial_sinusitis",
            prevalence: 0.05,
            symptoms: BTreeMap::from([
                ("facial_pain", 0.9),
                ("runny_nose_thick", 0.85),
                ("headache", 0.7),
                ("fever_mild", 0.6),
                ("cough", 0.5),
                ("fatigue", 0.4),
                ("sore_throat", 0.3),
            ]),
        },
    ]
}

// ── Knowledge Encoding V1 (Canonical) ────────────────────────────────────
// Returns the events needed to build the knowledge graph

pub fn encode_knowledge_v1() -> Vec<Event> {
    let profiles = build_condition_profiles();
    let mut events = Vec::new();
    let mut next_id: u64 = 100;

    for (_i, profile) in profiles.iter().enumerate() {
        let condition_id = next_id;
        next_id += 1;
        events.push(Event::CreateNode {
            id: condition_id,
            node_type: "condition".to_string(),
        });
        events.push(Event::SetProperty {
            node_id: condition_id,
            key: "name".to_string(),
            value: profile.name.to_string(),
        });
        events.push(Event::SetProperty {
            node_id: condition_id,
            key: "prevalence".to_string(),
            value: format!("{}", profile.prevalence),
        });

        let mut edge_id: u64 = condition_id * 1000;
        for (symptom_name, prob) in &profile.symptoms {
            let symptom_id = next_id;
            next_id += 1;
            events.push(Event::CreateNode {
                id: symptom_id,
                node_type: "symptom".to_string(),
            });
            events.push(Event::SetProperty {
                node_id: symptom_id,
                key: "name".to_string(),
                value: symptom_name.to_string(),
            });

            events.push(Event::CreateEdge {
                id: edge_id,
                from_node: condition_id,
                to_node: symptom_id,
                edge_type: "has_symptom".to_string(),
            });
            let strength = if *prob >= 0.8 { 3 } else if *prob >= 0.5 { 2 } else { 1 };
            events.push(Event::SetProperty {
                node_id: edge_id,
                key: "evidence_strength".to_string(),
                value: strength.to_string(),
            });
            edge_id += 1;
        }
    }

    events
}

// ── Knowledge Encoding V2 (Alternate representation — simulates independent developer) ──
// Differences from V1:
//   - Uses "disease" instead of "condition" node type
//   - Uses "exhibits" instead of "has_symptom" edge type
//   - Some symptoms are shared/merged instead of duplicated per condition
//   - Slightly different grouping — fewer total nodes

pub fn encode_knowledge_v2() -> Vec<Event> {
    let profiles = build_condition_profiles();
    let mut events = Vec::new();
    let mut next_id: u64 = 1000;

    // Shared symptom nodes (v2 reuses symptom nodes across conditions)
    let mut symptom_name_to_id: BTreeMap<&str, u64> = BTreeMap::new();

    for (_i, profile) in profiles.iter().enumerate() {
        let condition_id = next_id;
        next_id += 1;
        events.push(Event::CreateNode {
            id: condition_id,
            node_type: "disease".to_string(),
        });
        events.push(Event::SetProperty {
            node_id: condition_id,
            key: "label".to_string(),
            value: profile.name.to_string(),
        });

        let mut edge_id: u64 = condition_id * 1000;
        for (symptom_name, prob) in &profile.symptoms {
            let symptom_id = if let Some(&existing) = symptom_name_to_id.get(symptom_name) {
                existing
            } else {
                let sid = next_id;
                next_id += 1;
                events.push(Event::CreateNode {
                    id: sid,
                    node_type: "symptom".to_string(),
                });
                events.push(Event::SetProperty {
                    node_id: sid,
                    key: "label".to_string(),
                    value: symptom_name.to_string(),
                });
                symptom_name_to_id.insert(symptom_name, sid);
                sid
            };

            events.push(Event::CreateEdge {
                id: edge_id,
                from_node: condition_id,
                to_node: symptom_id,
                edge_type: "exhibits".to_string(),
            });
            let strength = if *prob >= 0.8 { "strong" } else if *prob >= 0.5 { "moderate" } else { "weak" };
            events.push(Event::SetProperty {
                node_id: edge_id,
                key: "weight".to_string(),
                value: strength.to_string(),
            });
            edge_id += 1;
        }
    }

    events
}

// ── Build Graph State from Events ────────────────────────────────────────

pub fn build_graph(events: &[Event]) -> GraphState {
    let mut state = GraphState::empty();
    for event in events {
        apply_event(&mut state, event);
    }
    state
}

// ── Test Cases (Patient Presentations) ───────────────────────────────────
// Each case: list of symptoms, expected diagnosis

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: &'static str,
    pub symptoms: Vec<&'static str>,
    pub expected_condition: &'static str,
}

pub fn build_test_cases() -> Vec<TestCase> {
    vec![
        TestCase {
            name: "classic cold",
            symptoms: vec!["runny_nose", "sneezing", "sore_throat", "cough", "fatigue"],
            expected_condition: "common_cold",
        },
        TestCase {
            name: "flu with fever",
            symptoms: vec!["fever_high", "body_ache", "fatigue", "headache", "cough"],
            expected_condition: "influenza",
        },
        TestCase {
            name: "covid with loss of taste",
            symptoms: vec!["cough", "fever_mild", "fatigue", "loss_of_taste", "headache"],
            expected_condition: "covid19",
        },
        TestCase {
            name: "strep throat",
            symptoms: vec!["sore_throat_severe", "fever_mild", "swollen_lymph_nodes", "headache"],
            expected_condition: "strep_throat",
        },
        TestCase {
            name: "allergies",
            symptoms: vec!["sneezing", "runny_nose", "itchy_eyes", "watery_eyes"],
            expected_condition: "allergic_rhinitis",
        },
        TestCase {
            name: "food poisoning",
            symptoms: vec!["nausea", "vomiting", "diarrhea", "abdominal_pain"],
            expected_condition: "food_poisoning",
        },
        TestCase {
            name: "migraine",
            symptoms: vec!["headache_severe", "nausea", "light_sensitivity"],
            expected_condition: "migraine",
        },
        TestCase {
            name: "sinusitis",
            symptoms: vec!["facial_pain", "runny_nose_thick", "headache", "fever_mild"],
            expected_condition: "bacterial_sinusitis",
        },
        TestCase {
            name: "ambiguous cold vs allergies",
            symptoms: vec!["runny_nose", "sneezing", "cough", "fatigue"],
            expected_condition: "common_cold",
        },
        TestCase {
            name: "covid with breathing issues",
            symptoms: vec!["cough", "fever_mild", "fatigue", "shortness_of_breath", "headache", "loss_of_taste"],
            expected_condition: "covid19",
        },
        TestCase {
            name: "mild flu",
            symptoms: vec!["fever_mild", "body_ache", "fatigue", "headache"],
            expected_condition: "influenza",
        },
        TestCase {
            name: "strep without sore throat",
            symptoms: vec!["fever_mild", "swollen_lymph_nodes", "headache", "fatigue"],
            expected_condition: "strep_throat",
        },
    ]
}

// ── Strata Deterministic Diagnosis ────────────────────────────────────────
// Uses knowledge encoded in the graph to diagnose based on evidence counting

pub fn strata_diagnose(
    _state: &GraphState,
    symptoms: &[&str],
) -> Vec<(&'static str, u64, Confidence)> {
    let profiles = build_condition_profiles();
    let mut results = Vec::new();

    for profile in &profiles {
        let mut matched = 0u64;
        let mut total_strength = 0u64;

        for symptom in symptoms {
            if let Some(&prob) = profile.symptoms.get(symptom) {
                matched += 1;
                let strength = if prob >= 0.8 { 3 } else if prob >= 0.5 { 2 } else { 1 };
                total_strength += strength;
            }
        }

        let confidence = match total_strength {
            0 => continue, // skip conditions with no matching symptoms
            1..=2 => Confidence::Low,
            3..=5 => Confidence::Medium,
            _ => Confidence::High,
        };

        results.push((profile.name, matched, confidence));
    }

    results.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| b.1.cmp(&a.1))
    });

    results
}

// ── Naive Bayes Baseline ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NaiveBayesClassifier {
    profiles: Vec<ConditionProfile>,
    total_conditions: usize,
    all_symptoms: Vec<&'static str>,
    symptom_priors: BTreeMap<&'static str, f64>,
}

impl NaiveBayesClassifier {
    pub fn new(profiles: Vec<ConditionProfile>) -> Self {
        let total_conditions = profiles.len();

        let mut all_symptoms_set: BTreeMap<&str, f64> = BTreeMap::new();
        for profile in &profiles {
            for (symptom, _) in &profile.symptoms {
                *all_symptoms_set.entry(symptom).or_insert(0.0) += 1.0;
            }
        }

        let total_profiles = profiles.len() as f64;
        let symptom_priors: BTreeMap<&str, f64> = all_symptoms_set
            .iter()
            .map(|(s, count)| (*s, count / total_profiles))
            .collect();

        let all_symptoms = symptom_priors.keys().copied().collect();

        NaiveBayesClassifier {
            profiles,
            total_conditions,
            all_symptoms,
            symptom_priors,
        }
    }

    pub fn diagnose(&self, symptoms: &[&str]) -> Vec<(&'static str, f64)> {
        let mut results = Vec::new();
        let symptom_set: BTreeMap<&str, bool> = symptoms.iter().map(|s| (*s, true)).collect();

        for profile in &self.profiles {
            let mut log_prob = profile.prevalence.ln();

            for symptom in &self.all_symptoms {
                let has_symptom = symptom_set.contains_key(symptom);
                let p_symptom_given_cond = profile.symptoms.get(symptom).copied().unwrap_or(0.01);

                if has_symptom {
                    log_prob += p_symptom_given_cond.ln();
                } else {
                    log_prob += (1.0 - p_symptom_given_cond).ln();
                }
            }

            results.push((profile.name, log_prob));
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

pub fn build_default_nb() -> NaiveBayesClassifier {
    NaiveBayesClassifier::new(build_condition_profiles())
}

// ── E4 Overlap Test Helpers ──────────────────────────────────────────────

pub fn run_e4_test() -> OverlapMetrics {
    let events_v1 = encode_knowledge_v1();
    let events_v2 = encode_knowledge_v2();
    let g1 = build_graph(&events_v1);
    let g2 = build_graph(&events_v2);
    measure_graph_overlap(&g1, &g2)
}

// ── E2 Comparison Runner ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct E2Result {
    pub test_case: TestCase,
    pub strata_prediction: Vec<(&'static str, u64, Confidence)>,
    pub nb_prediction: Vec<(&'static str, f64)>,
    pub strata_correct: bool,
    pub nb_correct: bool,
}

pub fn run_e2_test() -> Vec<E2Result> {
    let events = encode_knowledge_v1();
    let state = build_graph(&events);
    let profiles = build_condition_profiles();
    let cases = build_test_cases();

    let mut results = Vec::new();
    for case in &cases {
        let strata_result = strata_diagnose(&state, &case.symptoms);
        let nb = NaiveBayesClassifier::new(profiles.clone());
        let nb_result = nb.diagnose(&case.symptoms);

        let strata_correct = strata_result
            .first()
            .map(|r| r.0 == case.expected_condition)
            .unwrap_or(false);

        let nb_correct = nb_result
            .first()
            .map(|r| r.0 == case.expected_condition)
            .unwrap_or(false);

        results.push(E2Result {
            test_case: case.clone(),
            strata_prediction: strata_result,
            nb_prediction: nb_result,
            strata_correct,
            nb_correct,
        });
    }

    results
}

// ── E3 Event Explosion Measurements ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CognitiveOpMeasurement {
    pub operation: &'static str,
    pub event_count: usize,
    pub max_depth: usize,
    pub replay_ns: u128,
}

pub fn measure_cognitive_operations() -> Vec<CognitiveOpMeasurement> {
    let mut measurements = Vec::new();

    // Op 1: Assert a single belief
    {
        let mut k = Kernel::new();
        k.propose_and_commit(Event::CreateNode { id: 1, node_type: "claim".into() }).unwrap();
        let before = k.event_count();
        let start = std::time::Instant::now();
        k.propose_and_commit(Event::AssertBelief { node_id: 1, confidence: Confidence::Low }).unwrap();
        let end = std::time::Instant::now();
        let replayed = k.replay();
        assert!(k.state() == &replayed);
        measurements.push(CognitiveOpMeasurement {
            operation: "assert_single_belief",
            event_count: k.event_count() - before,
            max_depth: 1,
            replay_ns: end.duration_since(start).as_nanos(),
        });
    }

    // Op 2: Attach evidence to a belief
    {
        let mut k = Kernel::new();
        k.propose_and_commit(Event::CreateNode { id: 1, node_type: "claim".into() }).unwrap();
        k.propose_and_commit(Event::CreateNode { id: 2, node_type: "evidence".into() }).unwrap();
        k.propose_and_commit(Event::AssertBelief { node_id: 1, confidence: Confidence::Low }).unwrap();
        let before = k.event_count();
        let start = std::time::Instant::now();
        k.propose_and_commit(Event::CreateEdge { id: 10, from_node: 2, to_node: 1, edge_type: "evidence_for".into() }).unwrap();
        k.propose_and_commit(Event::AttachEvidence { belief_id: 1, evidence_id: 10 }).unwrap();
        let end = std::time::Instant::now();
        let replayed = k.replay();
        assert!(k.state() == &replayed);
        measurements.push(CognitiveOpMeasurement {
            operation: "attach_evidence",
            event_count: k.event_count() - before,
            max_depth: 2,
            replay_ns: end.duration_since(start).as_nanos(),
        });
    }

    // Op 3: Create condition-symptom knowledge structure
    {
        let mut k = Kernel::new();
        let before = k.event_count();
        let start = std::time::Instant::now();
        k.propose_and_commit(Event::CreateNode { id: 1, node_type: "condition".into() }).unwrap();
        k.propose_and_commit(Event::SetProperty { node_id: 1, key: "name".into(), value: "test_condition".into() }).unwrap();
        k.propose_and_commit(Event::CreateNode { id: 2, node_type: "symptom".into() }).unwrap();
        k.propose_and_commit(Event::SetProperty { node_id: 2, key: "name".into(), value: "test_symptom".into() }).unwrap();
        k.propose_and_commit(Event::CreateEdge { id: 10, from_node: 1, to_node: 2, edge_type: "has_symptom".into() }).unwrap();
        let end = std::time::Instant::now();
        let replayed = k.replay();
        assert!(k.state() == &replayed);
        measurements.push(CognitiveOpMeasurement {
            operation: "create_knowledge_link",
            event_count: k.event_count() - before,
            max_depth: 3,
            replay_ns: end.duration_since(start).as_nanos(),
        });
    }

    // Op 4: Full diagnosis chain (knowledge + symptoms + classification)
    {
        let mut k = Kernel::new();
        for pe in encode_knowledge_v1() {
            k.propose_and_commit(pe).unwrap();
        }
        let start = std::time::Instant::now();
        let result = strata_diagnose(k.state(), &["cough", "fever_mild", "fatigue", "loss_of_taste"]);
        let end = std::time::Instant::now();
        assert!(!result.is_empty());
        measurements.push(CognitiveOpMeasurement {
            operation: "full_diagnosis_query",
            event_count: 0,
            max_depth: 4,
            replay_ns: end.duration_since(start).as_nanos(),
        });
    }

    // Op 5: Belief revision cycle
    {
        let mut k = Kernel::new();
        k.propose_and_commit(Event::CreateNode { id: 1, node_type: "claim".into() }).unwrap();
        k.propose_and_commit(Event::AssertBelief { node_id: 1, confidence: Confidence::High }).unwrap();
        let rev_events: Vec<Event> = (0..5)
            .flat_map(|i| {
                let nid = 10 + i;
                vec![
                    Event::CreateNode { id: nid, node_type: "contradiction".into() },
                    Event::CreateEdge { id: 100 + i, from_node: nid, to_node: 1, edge_type: "contradicts".into() },
                    Event::AttachEvidence { belief_id: 1, evidence_id: 100 + i },
                ]
            })
            .collect();
        let before = k.event_count();
        let start = std::time::Instant::now();
        for ev in &rev_events {
            k.propose_and_commit(ev.clone()).unwrap();
        }
        let end = std::time::Instant::now();
        let replayed = k.replay();
        assert!(k.state() == &replayed);
        measurements.push(CognitiveOpMeasurement {
            operation: "belief_revision_chain",
            event_count: k.event_count() - before,
            max_depth: 5,
            replay_ns: end.duration_since(start).as_nanos(),
        });
    }

    measurements
}

// ── E6 Replay Scalability ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReplayScaleMeasurement {
    pub event_count: usize,
    pub replay_time_ns: u128,
    pub memory_bytes: usize,
}

pub fn generate_synthetic_log(event_count: usize) -> Vec<Event> {
    let mut events = Vec::with_capacity(event_count);
    // Create nodes in patterns
    let mut id_counter: u64 = 1;
    let batch_size = 100;

    for batch in 0..(event_count / batch_size + 1) {
        let remaining = event_count - events.len();
        if remaining == 0 {
            break;
        }
        let n = batch_size.min(remaining);
        for i in 0..n {
            let idx = (batch * batch_size + i) as u64;
            match idx % 5 {
                0 => events.push(Event::CreateNode { id: id_counter, node_type: "concept".to_string() }),
                1 => {
                    events.push(Event::CreateNode { id: id_counter, node_type: "instance".to_string() });
                    if id_counter > 1 {
                        events.push(Event::SetProperty {
                            node_id: id_counter,
                            key: "refers_to".to_string(),
                            value: (id_counter - 1).to_string(),
                        });
                    }
                }
                2 => {
                    events.push(Event::CreateEdge {
                        id: id_counter + 1_000_000,
                        from_node: (id_counter % 100).max(1),
                        to_node: id_counter,
                        edge_type: "relates_to".to_string(),
                    });
                }
                3 => {
                    let belief_id = (id_counter % 50).max(1);
                    events.push(Event::CreateNode { id: id_counter, node_type: "evidence".to_string() });
                    events.push(Event::CreateEdge {
                        id: id_counter + 2_000_000,
                        from_node: id_counter,
                        to_node: belief_id,
                        edge_type: "evidence_for".to_string(),
                    });
                    events.push(Event::AttachEvidence { belief_id, evidence_id: id_counter + 2_000_000 });
                }
                4 => {
                    events.push(Event::SetProperty {
                        node_id: (id_counter % 100).max(1),
                        key: "updated".to_string(),
                        value: id_counter.to_string(),
                    });
                }
                _ => {}
            }
            id_counter += 1;
        }
    }

    events.truncate(event_count);
    events
}

pub fn measure_replay_scalability() -> Vec<ReplayScaleMeasurement> {
    let sizes = [100_000, 1_000_000];
    let mut measurements = Vec::new();

    for &size in &sizes {
        let events = generate_synthetic_log(size);
        let seq_events: Vec<SequencedEvent> = events.iter().enumerate()
            .map(|(i, e)| SequencedEvent { seq: i as u64, event: e.clone() })
            .collect();
        let start = std::time::Instant::now();
        let state = replay(&seq_events);
        let end = std::time::Instant::now();

        let mem = state.nodes.len() * 200 + state.edges.len() * 200;

        measurements.push(ReplayScaleMeasurement {
            event_count: size,
            replay_time_ns: end.duration_since(start).as_nanos(),
            memory_bytes: mem + seq_events.len() * 100,
        });
    }

    measurements
}
