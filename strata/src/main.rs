use clap::{Parser, Subcommand};

use strata::event::{Event, EventType};
use strata::kernel::{replay, Kernel};
use strata::persistence;
use strata::version::CURRENT_SCHEMA_VERSION;

#[derive(Parser)]
#[command(name = "strata", version = "0.1.0", about = "Deterministic causal event-sourced graph kernel")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new node
    CreateNode { id: String },
    /// Create a new edge between two nodes
    CreateEdge { id: String, from: String, to: String, r#type: String },
    /// Set a property on a node or edge
    SetProperty { target: String, key: String, value: String },
    /// Delete a node and its incident edges
    DeleteNode { id: String },
    /// Delete an edge
    DeleteEdge { id: String },
    /// Replay event log from scratch and show state
    Replay,
    /// Show current graph state
    ShowState,
    /// Save a snapshot of current state
    SaveSnapshot,
    /// Run adversarial stress experiments (E-S1 through E-S4)
    StressTest,
    /// Explain a belief on a node (trace causal chain)
    Explain { node_id: String, property_key: Option<String> },
    /// Trace the causal chain of an event
    Trace { event_id: String },
    /// Display kernel version
    Version,
    /// Display schema version
    SchemaVersion,
    /// Validate event log integrity
    ValidateLog,
    /// Replay event log and verify consistency
    ReplayCheck,
}

fn parse_value(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.to_string()))
}

fn main() {
    let cli = Cli::parse();
    let mut kernel = Kernel::new();

    match cli.command {
        Command::CreateNode { id } => {
            let event = Event::new(
                format!("evt-{}", kernel.clock + 1),
                0,
                EventType::CreateNode,
                serde_json::json!({"id": id}),
            );
            match kernel.commit(event) {
                Ok(()) => println!("  node '{}' created", id),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Command::CreateEdge { id, from, to, r#type } => {
            let event = Event::new(
                format!("evt-{}", kernel.clock + 1),
                0,
                EventType::CreateEdge,
                serde_json::json!({"id": id, "from": from, "to": to, "type": r#type}),
            );
            match kernel.commit(event) {
                Ok(()) => println!("  edge '{}' created ({} --{}--> {})", id, from, r#type, to),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Command::SetProperty { target, key, value } => {
            let val = parse_value(&value);
            let event = Event::new(
                format!("evt-{}", kernel.clock + 1),
                0,
                EventType::SetProperty,
                serde_json::json!({"target_id": target, "key": key, "value": val}),
            );
            match kernel.commit(event) {
                Ok(()) => println!("  {} {} = {} set", target, key, val),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Command::DeleteNode { id } => {
            let event = Event::new(
                format!("evt-{}", kernel.clock + 1),
                0,
                EventType::DeleteNode,
                serde_json::json!({"id": id}),
            );
            match kernel.commit(event) {
                Ok(()) => println!("  node '{}' deleted (edges cascaded)", id),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Command::DeleteEdge { id } => {
            let event = Event::new(
                format!("evt-{}", kernel.clock + 1),
                0,
                EventType::DeleteEdge,
                serde_json::json!({"id": id}),
            );
            match kernel.commit(event) {
                Ok(()) => println!("  edge '{}' deleted", id),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Command::Replay => {
            let events = strata::persistence::load_all_events().unwrap_or_default();
            println!("[replay] loading {} events from log...", events.len());
            let state = strata::kernel::replay(&events);
            let cg = strata::causal::replay_causal(&events);
            println!(
                "[replay] result: {} nodes, {} edges, {} causal edges",
                state.node_count(),
                state.edge_count(),
                cg.edges.len()
            );
            for (id, node) in &state.nodes {
                println!("  NODE {} | props={}", id, node.properties.len());
            }
            for (id, edge) in &state.edges {
                println!(
                    "  EDGE {} ({} --{}--> {}) | props={}",
                    id,
                    edge.from,
                    edge.edge_type,
                    edge.to,
                    edge.properties.len()
                );
            }
        }
        Command::ShowState => {
            let state = kernel.get_state();
            let cg = kernel.get_causal_graph();
            if state.node_count() == 0 && state.edge_count() == 0 {
                println!("  (empty graph)");
            }
            for (id, node) in &state.nodes {
                let props: Vec<String> = node
                    .properties
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                println!("  NODE {}", id);
                if !props.is_empty() {
                    println!("    props: {}", props.join(", "));
                }
            }
            for (id, edge) in &state.edges {
                let props: Vec<String> = edge
                    .properties
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                println!("  EDGE {} ({} --{}--> {})", id, edge.from, edge.edge_type, edge.to);
                if !props.is_empty() {
                    println!("    props: {}", props.join(", "));
                }
            }
            println!(
                "  --- {} nodes, {} edges, {} events, {} causal edges ---",
                state.node_count(),
                state.edge_count(),
                kernel.event_count,
                cg.edges.len()
            );
        }
        Command::SaveSnapshot => {
            match kernel.save_snapshot() {
                Ok(()) => println!("  snapshot saved"),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Command::StressTest => {
            let report = strata::harness::run_all_experiments();
            println!("{}", report);
        }
        Command::Explain { node_id, property_key } => {
            let explanation = kernel.explain_belief(&node_id, property_key.as_deref());
            println!("  Explanation for {}:{}", node_id, property_key.as_deref().unwrap_or("*"));
            match explanation.current_value {
                Some(ref v) => println!("  Current value: {}", v),
                None => println!("  (no value or node not found)"),
            }
            if explanation.chain.is_empty() {
                println!("  (no causal chain found)");
            } else {
                println!("  Causal chain ({} hops):", explanation.hops);
                for (i, link) in explanation.chain.iter().enumerate() {
                    println!(
                        "    [{}/{}] {} ({:?}) ts={}{}",
                        i + 1,
                        explanation.hops,
                        link.event_id,
                        link.event_type,
                        link.timestamp,
                        link.meta_reason
                            .as_ref()
                            .map(|r| format!(" reason=\"{}\"", r))
                            .unwrap_or_default()
                    );
                }
            }
        }
        Command::Trace { event_id } => {
            let chain = kernel.trace_causal_chain(&event_id);
            if chain.is_empty() {
                println!("  event '{}' not found in causal graph", event_id);
            } else {
                println!("  Causal chain for {} ({} hops):", event_id, chain.len());
                for (i, link) in chain.iter().enumerate() {
                    println!(
                        "    [{}/{}] {} ({:?}) ts={}{}",
                        i + 1,
                        chain.len(),
                        link.event_id,
                        link.event_type,
                        link.timestamp,
                        link.meta_reason
                            .as_ref()
                            .map(|r| format!(" reason=\"{}\"", r))
                            .unwrap_or_default()
                    );
                }
            }
        }
        Command::Version => {
            let kv = kernel.kernel_version();
            println!("{}", kv);
        }
        Command::SchemaVersion => {
            println!("{}", CURRENT_SCHEMA_VERSION);
        }
        Command::ValidateLog => {
            match persistence::load_all_events() {
                Ok(events) => {
                    println!("  log valid: {} events loaded", events.len());
                    let mut prev_ts = 0u64;
                    let mut errors = 0;
                    for e in &events {
                        if e.timestamp < prev_ts {
                            eprintln!("  error: {} has out-of-order timestamp {} < {}", e.id, e.timestamp, prev_ts);
                            errors += 1;
                        }
                        prev_ts = e.timestamp;
                    }
                    if errors == 0 {
                        println!("  timestamps: monotonically non-decreasing");
                    } else {
                        eprintln!("  {} timestamp error(s) found", errors);
                    }
                }
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Command::ReplayCheck => {
            match persistence::load_all_events() {
                Ok(events) => {
                    let state = replay(&events);
                    println!(
                        "  replay: {} events -> {} nodes, {} edges",
                        events.len(),
                        state.node_count(),
                        state.edge_count()
                    );
                    match persistence::load_snapshot() {
                        Ok(Some((snap, _))) => {
                            if snap.state == state {
                                println!("  snapshot: MATCH (state identical)");
                            } else {
                                println!("  snapshot: MISMATCH (state differs)");
                            }
                        }
                        Ok(None) => println!("  snapshot: (none available)"),
                        Err(e) => eprintln!("  snapshot error: {}", e),
                    }
                }
                Err(e) => eprintln!("error: {}", e),
            }
        }
    }
}
