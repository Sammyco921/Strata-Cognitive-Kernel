use strata_kill_test::kernel::*;
use strata_kill_test::triage::{generate_synthetic_log, measure_replay_scalability};

#[test]
fn e6_replay_scalability_100k() {
    let events = generate_synthetic_log(100_000);
    let seq_events: Vec<SequencedEvent> = events.iter().enumerate()
        .map(|(i, e)| SequencedEvent { seq: i as u64, event: e.clone() })
        .collect();

    let start = std::time::Instant::now();
    let state = replay(&seq_events);
    let elapsed = start.elapsed();

    println!("=== E6: Replay Scalability (100K events) ===");
    println!("  Replayed {} events in {:.4}s", 100_000, elapsed.as_secs_f64());
    println!("  Result: {} nodes, {} edges", state.node_count(), state.edge_count());
    println!("  Rate: {:.0} events/sec", 100_000.0 / elapsed.as_secs_f64());

    assert!(elapsed.as_secs_f64() < 60.0,
        "100K events replayed in {:.2}s (need < 60s)", elapsed.as_secs_f64());
}

#[test]
fn e6_replay_scalability_1m() {
    let events = generate_synthetic_log(1_000_000);
    let seq_events: Vec<SequencedEvent> = events.iter().enumerate()
        .map(|(i, e)| SequencedEvent { seq: i as u64, event: e.clone() })
        .collect();

    let start = std::time::Instant::now();
    let state = replay(&seq_events);
    let elapsed = start.elapsed();

    println!("=== E6: Replay Scalability (1M events) ===");
    println!("  Replayed {} events in {:.4}s", 1_000_000, elapsed.as_secs_f64());
    println!("  Result: {} nodes, {} edges", state.node_count(), state.edge_count());
    println!("  Rate: {:.0} events/sec", 1_000_000.0 / elapsed.as_secs_f64());

    assert!(elapsed.as_secs_f64() < 120.0,
        "1M events replayed in {:.2}s (need < 120s)", elapsed.as_secs_f64());
}

#[test]
fn e6_scaling_curve() {
    let measurements = measure_replay_scalability();
    println!("=== E6: Replay Scaling Curve ===");
    for m in &measurements {
        let secs = m.replay_time_ns as f64 / 1_000_000_000.0;
        let events_per_sec = if secs > 0.0 { m.event_count as f64 / secs } else { 0.0 };
        println!("  {} events: {:.4}s ({:.0} events/sec)", m.event_count, secs, events_per_sec);
    }

    if measurements.len() >= 2 {
        let t1 = measurements[0].replay_time_ns as f64 / 1_000_000_000.0;
        let t2 = measurements[1].replay_time_ns as f64 / 1_000_000_000.0;
        let e1 = measurements[0].event_count as f64;
        let e2 = measurements[1].event_count as f64;
        let ratio_time = t2 / t1;
        let ratio_events = e2 / e1;
        println!("  Time ratio: {:.2}x for {:.0}x events (ideal linear = {:.2}x)",
            ratio_time, ratio_events, ratio_events);
        if ratio_time > ratio_events * 1.5 {
            println!("  WARNING: Superlinear scaling detected (ratio={:.2}x, expected<={:.2}x)",
                ratio_time, ratio_events * 1.5);
        } else {
            println!("  PASS: Near-linear scaling");
        }
    }
}
