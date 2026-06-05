use serde::{Deserialize, Serialize};

use crate::api::{EdgeView, Engine, EventView, ExplanationView, NodeView, SnapshotView};
use crate::kernel::error::KernelError;
use crate::kernel::event::Event;
use crate::projection::causal::CausalChainLink;

// ── Output DTOs ──────────────────────────────────────────────────────────────

/// DTO-safe representation of full graph state.
/// Does NOT contain internal kernel types.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StateView {
    pub nodes: Vec<NodeView>,
    pub edges: Vec<EdgeView>,
}

impl StateView {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }
}

/// Structural comparison between two replay results.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiffView {
    pub label_a: String,
    pub label_b: String,
    pub nodes_only_in_a: Vec<String>,
    pub nodes_only_in_b: Vec<String>,
    pub edges_only_in_a: Vec<String>,
    pub edges_only_in_b: Vec<String>,
    pub nodes_with_different_properties: Vec<String>,
    pub edges_with_different_properties: Vec<String>,
    pub states_equal: bool,
}

// ── Commands (1:1 with Engine methods) ──────────────────────────────────────

/// A canonical execution request.
///
/// Every variant maps to exactly one Engine trait method. No variant
/// duplicates Engine behaviour. No variant is an alias for another.
///
/// Commands are immutable, deterministic, and interface-independent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    // ── Core (6 methods) ─────────────────────────────────────────────────

    /// Dry-run validation — calls Engine::validate.
    Validate(Event),

    /// Commit an event — calls Engine::ingest_event.
    Ingest(Event),

    /// Pure replay — calls Engine::replay.
    Replay(Vec<Event>),

    /// Full current state — calls Engine::query_state.
    QueryState,

    /// Trace causal chain behind a property — calls Engine::get_explanation.
    Explain { node_id: String, property_key: Option<String> },

    /// Trace causal predecessors of an event — calls Engine::causal_chain.
    CausalChain(String),

    /// Export snapshot to disk + return JSON — calls Engine::export_snapshot.
    ExportSnapshot,

    // ── State Queries (4 methods) ───────────────────────────────────────

    /// Single node by ID — calls Engine::get_node.
    GetNode(String),

    /// Single edge by ID — calls Engine::get_edge.
    GetEdge(String),

    /// All nodes — calls Engine::list_nodes.
    ListNodes,

    /// All edges — calls Engine::list_edges.
    ListEdges,

    // ── History Queries (4 methods) ─────────────────────────────────────

    /// Single event by ID — calls Engine::event_by_id.
    EventById(String),

    /// All events referencing a node — calls Engine::events_for_node.
    EventsForNode(String),

    /// Events in a timestamp range — calls Engine::events_between.
    EventsBetween { start: u64, end: u64 },

    /// Most recent N events — calls Engine::latest_events.
    LatestEvents(usize),

    // ── Snapshot (1 method) ─────────────────────────────────────────────

    /// Snapshot metadata — calls Engine::get_snapshot_metadata.
    SnapshotMetadata,

    // ── System Commands (meta-operations, no engine dispatch) ───────────

    /// Display kernel version.
    GetVersion,
    /// Display schema version.
    GetSchemaVersion,
    /// Validate event log integrity.
    ValidateLog,
    /// Replay and compare against snapshot.
    ReplayCheck,
    /// List available workflows.
    WorkflowList,
    /// Run a named workflow.
    WorkflowRun(String),
    /// Run all workflows and report results.
    WorkflowValidate,
    /// List all available CLI commands.
    ListCommands,
    /// Show detailed info about a specific command.
    Describe(String),
}

/// Formal classification of every `Command` variant.
///
/// Each variant belongs to exactly one category:
///
/// | Class | Semantics | Engine access |
/// |---|---|---|
/// | `Execution` | Mutates the event log | `&mut dyn Engine` |
/// | `Query` | Reads state only | `&dyn Engine` |
/// | `System` | Metadata / diagnostics | None (no engine) |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandClass {
    /// Commands that mutate the event log.
    Execution,
    /// Commands that read state without mutation.
    Query,
    /// Commands that return metadata or diagnostics without touching the engine.
    System,
}

impl Command {
    /// Return the formal class of this command.
    ///
    /// The classification is immutable — every `Command` variant returns
    /// exactly one class, and that class never changes.
    pub fn class(&self) -> CommandClass {
        match self {
            // ── Execution (mutates event log) ────────────────────────────
            Command::Ingest(_) => CommandClass::Execution,

            // ── Query (reads state, no mutation) ─────────────────────────
            Command::Validate(_)
            | Command::Replay(_)
            | Command::QueryState
            | Command::Explain { .. }
            | Command::CausalChain(_)
            | Command::ExportSnapshot
            | Command::GetNode(_)
            | Command::GetEdge(_)
            | Command::ListNodes
            | Command::ListEdges
            | Command::EventById(_)
            | Command::EventsForNode(_)
            | Command::EventsBetween { .. }
            | Command::LatestEvents(_)
            | Command::SnapshotMetadata => CommandClass::Query,

            // ── System ───────────────────────────────────────────────────
            Command::GetVersion
            | Command::GetSchemaVersion
            | Command::ValidateLog
            | Command::ReplayCheck
            | Command::WorkflowList
            | Command::WorkflowRun(_)
            | Command::WorkflowValidate
            | Command::ListCommands
            | Command::Describe(_) => CommandClass::System,
        }
    }
}

// ── Command Results (1:1 with Command variants) ─────────────────────────────

/// The result of executing a single `Command`.
///
/// Every variant encodes exactly one semantic intent. No polymorphic
/// reuse of result types. Shared inner types (e.g. `StateView` appearing
/// in both `QueryState` and `Replay`) are distinguished by variant name.
///
/// Errors from Engine or Command validation are always carried via the
/// `Error` variant — this is the sole failure channel to external callers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum CommandResult {
    // ── Core results ────────────────────────────────────────────────────

    /// Validate succeeded.
    Valid,
    /// Ingest succeeded.
    Ingested,
    /// Replayed state.
    Replay(StateView),
    /// Full graph state.
    QueryState(StateView),
    /// Explanation of a node's property.
    Explain(ExplanationView),
    /// Causal chain trace.
    CausalChain(Vec<CausalChainLink>),
    /// Exported snapshot JSON string.
    ExportSnapshot(String),

    // ── State query results ─────────────────────────────────────────────

    /// Single node lookup.
    GetNode(Option<NodeView>),
    /// Single edge lookup.
    GetEdge(Option<EdgeView>),
    /// All nodes.
    ListNodes(Vec<NodeView>),
    /// All edges.
    ListEdges(Vec<EdgeView>),

    // ── History query results ───────────────────────────────────────────

    /// Single event lookup.
    EventById(Option<EventView>),
    /// Events referencing a node.
    EventsForNode(Vec<EventView>),
    /// Events in a timestamp range.
    EventsBetween(Vec<EventView>),
    /// Most recent events.
    LatestEvents(Vec<EventView>),

    // ── Snapshot result ─────────────────────────────────────────────────

    /// Snapshot metadata.
    SnapshotMetadata(SnapshotView),

    // ── Special carriers ────────────────────────────────────────────────

    /// Human-readable output from system commands (version, schema, etc.).
    Message(String),
    /// The sole failure carrier to external callers.
    Error(KernelError),
}

impl CommandResult {
    /// Format this result as human-readable output for CLI display.
    pub fn format_output(&self) -> String {
        match self {
            CommandResult::Valid => "  valid".into(),
            CommandResult::Ingested => "  committed".into(),

            CommandResult::Replay(view) | CommandResult::QueryState(view) => {
                let mut out = String::new();
                if view.node_count() == 0 && view.edge_count() == 0 {
                    out.push_str("  (empty graph)\n");
                }
                for node in &view.nodes {
                    out.push_str(&format!("  NODE {}\n", node.id));
                    if !node.properties.is_empty() {
                        let props: Vec<String> = node
                            .properties
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect();
                        out.push_str(&format!("    props: {}\n", props.join(", ")));
                    }
                }
                for edge in &view.edges {
                    let props: Vec<String> = edge
                        .properties
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect();
                    out.push_str(&format!(
                        "  EDGE {} ({} --{}--> {})\n",
                        edge.id, edge.from, edge.edge_type, edge.to
                    ));
                    if !props.is_empty() {
                        out.push_str(&format!("    props: {}\n", props.join(", ")));
                    }
                }
                out.push_str(&format!(
                    "  --- {} nodes, {} edges ---\n",
                    view.node_count(),
                    view.edge_count(),
                ));
                out.trim_end().to_string()
            }

            CommandResult::GetNode(Some(node)) => {
                format!("  NODE {}\n    props: {}", node.id, node.properties.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", "))
            }
            CommandResult::GetNode(None) => "  (not found)".into(),

            CommandResult::GetEdge(Some(edge)) => {
                format!("  EDGE {} ({} --{}--> {})", edge.id, edge.from, edge.edge_type, edge.to)
            }
            CommandResult::GetEdge(None) => "  (not found)".into(),

            CommandResult::ListNodes(nodes) => {
                if nodes.is_empty() {
                    "  (none)".into()
                } else {
                    let mut out = String::new();
                    for node in nodes {
                        out.push_str(&format!("  NODE {}\n", node.id));
                    }
                    out.trim_end().to_string()
                }
            }
            CommandResult::ListEdges(edges) => {
                if edges.is_empty() {
                    "  (none)".into()
                } else {
                    let mut out = String::new();
                    for edge in edges {
                        out.push_str(&format!("  EDGE {} ({} --{}--> {})\n", edge.id, edge.from, edge.edge_type, edge.to));
                    }
                    out.trim_end().to_string()
                }
            }

            CommandResult::EventById(Some(ev)) => {
                format!("  EVENT {} ({}) ts={}", ev.id, ev.event_type, ev.timestamp)
            }
            CommandResult::EventById(None) => "  (not found)".into(),

            CommandResult::EventsForNode(events)
            | CommandResult::EventsBetween(events)
            | CommandResult::LatestEvents(events) => {
                if events.is_empty() {
                    "  (none)".into()
                } else {
                    let mut out = String::new();
                    for ev in events {
                        out.push_str(&format!("  EVENT {} ({}) ts={}\n", ev.id, ev.event_type, ev.timestamp));
                    }
                    out.trim_end().to_string()
                }
            }

            CommandResult::Explain(ex) => {
                let mut out = format!("  Explanation for {}:{}", ex.target_node_id, ex.property_key.as_deref().unwrap_or("*"));
                match &ex.current_value {
                    Some(ref v) => out.push_str(&format!("\n  Current value: {}", v)),
                    None => out.push_str("\n  (no value or node not found)"),
                }
                if ex.chain.is_empty() {
                    out.push_str("\n  (no causal chain found)");
                } else {
                    out.push_str(&format!("\n  Causal chain ({} hops):", ex.hops));
                    for (i, link) in ex.chain.iter().enumerate() {
                        out.push_str(&format!(
                            "\n    [{}/{}] {} ({}) ts={}{}",
                            i + 1,
                            ex.hops,
                            link.event_id,
                            link.event_type,
                            link.timestamp,
                            link.meta_reason.as_ref().map(|r| format!(" reason=\"{}\"", r)).unwrap_or_default()
                        ));
                    }
                }
                out
            }

            CommandResult::CausalChain(chain) => {
                if chain.is_empty() {
                    "  (no causal chain found)".into()
                } else {
                    let mut out = String::new();
                    for link in chain {
                        out.push_str(&format!("  {} ({}) ts={}\n", link.event_id, link.event_type, link.timestamp));
                    }
                    out.trim_end().to_string()
                }
            }

            CommandResult::ExportSnapshot(json) => {
                format!("  snapshot: {} bytes", json.len())
            }

            CommandResult::SnapshotMetadata(meta) => {
                format!(
                    "  kernel: {} | schema: {} | events: {} | nodes: {} | edges: {}",
                    meta.kernel_version, meta.schema_version, meta.last_event_timestamp, meta.node_count, meta.edge_count
                )
            }

            CommandResult::Message(msg) => msg.clone(),
            CommandResult::Error(err) => format!("  error: {}", err),
        }
    }

    /// Returns true if this result represents a success.
    pub fn is_ok(&self) -> bool {
        !matches!(self, CommandResult::Error(_))
    }
}

// ── Command Executor ─────────────────────────────────────────────────────────

/// Executes `Command` values against an `Engine`.
///
/// The executor is the only bridge between the command system and the engine.
/// It depends **only** on the `Engine` trait — no direct kernel, projection,
/// or persistence access.
///
/// ## Pipeline
///
/// ```text
/// Command → Engine call → CommandResult
/// ```
pub struct CommandExecutor<'a> {
    engine: &'a mut dyn Engine,
}

impl<'a> CommandExecutor<'a> {
    /// Create a new executor bound to the given engine.
    pub fn new(engine: &'a mut dyn Engine) -> Self {
        CommandExecutor { engine }
    }

    /// Execute a command and return its result.
    ///
    /// Every command follows the same pipeline:
    /// 1. Delegate to the appropriate Engine trait method
    /// 2. Wrap the result in a `CommandResult`
    pub fn execute(&mut self, command: Command) -> CommandResult {
        match command {
            // ── Core ─────────────────────────────────────────────────────
            Command::Validate(event) => match self.engine.validate(&event) {
                Ok(()) => CommandResult::Valid,
                Err(e) => CommandResult::Error(e),
            },
            Command::Ingest(event) => match self.engine.ingest_event(event) {
                Ok(()) => CommandResult::Ingested,
                Err(e) => CommandResult::Error(e),
            },
            Command::Replay(events) => {
                let state = self.engine.replay(&events);
                let nodes: Vec<NodeView> = state.nodes.values().map(|n| NodeView::from_node(n)).collect();
                let edges: Vec<EdgeView> = state.edges.values().map(|e| EdgeView::from_edge(e)).collect();
                CommandResult::Replay(StateView { nodes, edges })
            }
            Command::QueryState => {
                let state = self.engine.query_state();
                let nodes: Vec<NodeView> = state.nodes.values().map(|n| NodeView::from_node(n)).collect();
                let edges: Vec<EdgeView> = state.edges.values().map(|e| EdgeView::from_edge(e)).collect();
                CommandResult::QueryState(StateView { nodes, edges })
            }
            Command::Explain { node_id, property_key } => {
                CommandResult::Explain(
                    self.engine.get_explanation(&node_id, property_key.as_deref()),
                )
            }
            Command::CausalChain(event_id) => {
                CommandResult::CausalChain(self.engine.causal_chain(&event_id))
            }
            Command::ExportSnapshot => match self.engine.export_snapshot() {
                Ok(json) => CommandResult::ExportSnapshot(json),
                Err(e) => CommandResult::Error(e),
            },

            // ── State Queries ────────────────────────────────────────────
            Command::GetNode(id) => CommandResult::GetNode(self.engine.get_node(&id)),
            Command::GetEdge(id) => CommandResult::GetEdge(self.engine.get_edge(&id)),
            Command::ListNodes => CommandResult::ListNodes(self.engine.list_nodes()),
            Command::ListEdges => CommandResult::ListEdges(self.engine.list_edges()),

            // ── History Queries ──────────────────────────────────────────
            Command::EventById(id) => CommandResult::EventById(self.engine.event_by_id(&id)),
            Command::EventsForNode(node_id) => {
                CommandResult::EventsForNode(self.engine.events_for_node(&node_id))
            }
            Command::EventsBetween { start, end } => {
                CommandResult::EventsBetween(self.engine.events_between(start, end))
            }
            Command::LatestEvents(n) => CommandResult::LatestEvents(self.engine.latest_events(n)),

            // ── Snapshot ─────────────────────────────────────────────────
            Command::SnapshotMetadata => {
                CommandResult::SnapshotMetadata(self.engine.get_snapshot_metadata())
            }

            // ── System commands (should never reach executor) ───────────
            _ => CommandResult::Error(KernelError::ValidationError(
                "system command dispatched to engine executor".into(),
            )),
        }
    }
}

// ── Bootstrap-level helpers (not part of the Command → Engine 1:1) ─────────

/// Compute a structural diff between two event sequences by replaying both.
///
/// This is a free function, not a Command variant, because both sides
/// call the same Engine::replay method. Building it at the bootstrap
/// layer preserves the 1:1 Command → Engine bijection.
pub fn diff(
    engine: &mut dyn Engine,
    label_a: &str,
    events_a: Vec<Event>,
    label_b: &str,
    events_b: Vec<Event>,
) -> DiffView {
    let state_a = engine.replay(&events_a);
    let state_b = engine.replay(&events_b);

    let mut nodes_only_in_a = Vec::new();
    let mut nodes_only_in_b = Vec::new();
    let mut nodes_with_different_properties = Vec::new();

    for (id, _) in &state_a.nodes {
        if !state_b.nodes.contains_key(id) {
            nodes_only_in_a.push(id.clone());
        } else if state_a.nodes[id].properties != state_b.nodes[id].properties {
            nodes_with_different_properties.push(id.clone());
        }
    }
    for (id, _) in &state_b.nodes {
        if !state_a.nodes.contains_key(id) {
            nodes_only_in_b.push(id.clone());
        }
    }

    let mut edges_only_in_a = Vec::new();
    let mut edges_only_in_b = Vec::new();
    let mut edges_with_different_properties = Vec::new();

    for (id, _) in &state_a.edges {
        if !state_b.edges.contains_key(id) {
            edges_only_in_a.push(id.clone());
        } else if state_a.edges[id].properties != state_b.edges[id].properties {
            edges_with_different_properties.push(id.clone());
        }
    }
    for (id, _) in &state_b.edges {
        if !state_a.edges.contains_key(id) {
            edges_only_in_b.push(id.clone());
        }
    }

    let states_equal = state_a == state_b;
    DiffView {
        label_a: label_a.into(),
        label_b: label_b.into(),
        nodes_only_in_a,
        nodes_only_in_b,
        edges_only_in_a,
        edges_only_in_b,
        nodes_with_different_properties,
        edges_with_different_properties,
        states_equal,
    }
}
