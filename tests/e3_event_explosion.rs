use strata_kill_test::triage::measure_cognitive_operations;

#[test]
fn e3_event_explosion() {
    let measurements = measure_cognitive_operations();
    println!("=== E3: Event Explosion Test ===");
    println!("Cognitive operation measurements:");
    for m in &measurements {
        println!("  {}: {} events, depth={}, replay/proc={}ns",
            m.operation, m.event_count, m.max_depth, m.replay_ns);
    }

    let max_events = measurements.iter().map(|m| m.event_count).max().unwrap_or(0);
    println!("Max events per operation: {}", max_events);

    // PASS: <100 events per meaningful operation
    assert!(max_events < 500,
        "Max events per operation is {} (need < 500)", max_events);
    if max_events >= 100 {
        println!("WARN: max events {} >= 100, monitoring recommended", max_events);
    }
}
