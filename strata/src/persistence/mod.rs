use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use serde::{Deserialize, Serialize};

use crate::kernel::error::KernelError;
use crate::kernel::event::{Event, EventEnvelope};
use crate::kernel::graph::GraphState;
use crate::kernel::version::{KernelVersion, SchemaVersion, CURRENT_KERNEL_VERSION, CURRENT_SCHEMA_VERSION};

const EVENTS_FILE: &str = "events.jsonl";
const CAUSAL_FILE: &str = "causal.jsonl";
const SNAPSHOT_FILE: &str = "snapshot.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub kernel_version: KernelVersion,
    pub schema_version: SchemaVersion,
    pub last_event_timestamp: u64,
    pub state: GraphState,
}

impl Snapshot {
    pub fn new(state: GraphState, last_event_timestamp: u64) -> Self {
        Snapshot {
            kernel_version: CURRENT_KERNEL_VERSION,
            schema_version: CURRENT_SCHEMA_VERSION,
            last_event_timestamp,
            state,
        }
    }
}

pub fn append_event(event: &Event) -> Result<(), KernelError> {
    let envelope = EventEnvelope::new(event.clone());
    append_envelope(&envelope)
}

fn append_envelope(envelope: &EventEnvelope) -> Result<(), KernelError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(EVENTS_FILE)
        .map_err(|e| KernelError::PersistenceError(format!("cannot open events file: {}", e)))?;

    let line = serde_json::to_string(envelope)
        .map_err(|e| KernelError::PersistenceError(format!("serialize error: {}", e)))?;
    writeln!(file, "{}", line)
        .map_err(|e| KernelError::PersistenceError(format!("write error: {}", e)))?;

    Ok(())
}

pub fn load_all_events() -> Result<Vec<Event>, KernelError> {
    let file = match File::open(EVENTS_FILE) {
        Ok(f) => f,
        Err(_) => return Ok(Vec::new()),
    };

    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| {
            KernelError::PersistenceError(format!("read error at line {}: {}", lineno + 1, e))
        })?;
        if line.trim().is_empty() {
            continue;
        }

        let event = match serde_json::from_str::<EventEnvelope>(&line) {
            Ok(envelope) => envelope.event,
            Err(_) => {
                serde_json::from_str::<Event>(&line).map_err(|e| {
                    KernelError::PersistenceError(format!(
                        "parse error at line {}: {}",
                        lineno + 1,
                        e
                    ))
                })?
            }
        };
        events.push(event);
    }

    Ok(events)
}

pub fn load_envelopes() -> Result<Vec<EventEnvelope>, KernelError> {
    let file = match File::open(EVENTS_FILE) {
        Ok(f) => f,
        Err(_) => return Ok(Vec::new()),
    };

    let reader = BufReader::new(file);
    let mut envelopes = Vec::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| {
            KernelError::PersistenceError(format!("read error at line {}: {}", lineno + 1, e))
        })?;
        if line.trim().is_empty() {
            continue;
        }

        let envelope = match serde_json::from_str::<EventEnvelope>(&line) {
            Ok(env) => env,
            Err(_) => {
                let event: Event = serde_json::from_str(&line).map_err(|e| {
                    KernelError::PersistenceError(format!(
                        "parse error at line {}: {}",
                        lineno + 1,
                        e
                    ))
                })?;
                EventEnvelope::with_version(event, SchemaVersion::new(1, 0))
            }
        };
        envelopes.push(envelope);
    }

    Ok(envelopes)
}

pub fn append_causal_edge(edge: &crate::projection::causal::CausalEdge) -> Result<(), KernelError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(CAUSAL_FILE)
        .map_err(|e| KernelError::PersistenceError(format!("cannot open causal file: {}", e)))?;

    let line = serde_json::to_string(edge)
        .map_err(|e| KernelError::PersistenceError(format!("serialize error: {}", e)))?;
    writeln!(file, "{}", line)
        .map_err(|e| KernelError::PersistenceError(format!("write error: {}", e)))?;

    Ok(())
}

pub fn load_causal_edges() -> Result<Vec<crate::projection::causal::CausalEdge>, KernelError> {
    let file = match File::open(CAUSAL_FILE) {
        Ok(f) => f,
        Err(_) => return Ok(Vec::new()),
    };

    let reader = BufReader::new(file);
    let mut edges = Vec::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| {
            KernelError::PersistenceError(format!("read error at line {}: {}", lineno + 1, e))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let edge: crate::projection::causal::CausalEdge = serde_json::from_str(&line)
            .map_err(|e| KernelError::PersistenceError(format!("parse error at line {}: {}", lineno + 1, e)))?;
        edges.push(edge);
    }

    Ok(edges)
}

pub fn save_snapshot(state: &GraphState, last_event_timestamp: u64) -> Result<(), KernelError> {
    let snapshot = Snapshot::new(state.clone(), last_event_timestamp);
    let json = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| KernelError::PersistenceError(format!("snapshot serialize error: {}", e)))?;
    fs::write(SNAPSHOT_FILE, json)
        .map_err(|e| KernelError::PersistenceError(format!("snapshot write error: {}", e)))
}

pub fn load_snapshot() -> Result<Option<(Snapshot, u64)>, KernelError> {
    let data = match fs::read_to_string(SNAPSHOT_FILE) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    let snapshot: Snapshot =
        serde_json::from_str(&data).map_err(|e| KernelError::PersistenceError(format!("snapshot parse error: {}", e)))?;
    let timestamp = snapshot.last_event_timestamp;
    Ok(Some((snapshot, timestamp)))
}

pub fn last_event_timestamp() -> u64 {
    let events = load_all_events().ok();
    events
        .as_ref()
        .and_then(|v| v.last())
        .map(|e| e.timestamp)
        .unwrap_or(0)
}

pub fn load_causal_graph() -> Result<(Vec<Event>, Vec<crate::projection::causal::CausalEdge>), KernelError> {
    let events = load_all_events()?;
    let edges = load_causal_edges()?;
    Ok((events, edges))
}
