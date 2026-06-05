use crate::kernel::engine::Kernel;
use crate::kernel::error::KernelError;
use crate::kernel::event::Event;
use crate::kernel::graph::GraphState;
use crate::kernel::replay::replay;
use crate::kernel::version::{CURRENT_KERNEL_VERSION, CURRENT_SCHEMA_VERSION};
use crate::persistence;

use crate::projection::causal::CausalChainLink;

pub use crate::api::traits::Engine;

pub mod command;
pub mod dispatcher;
pub mod envelope;
mod query;
pub mod result;
mod traits;

pub use command::{Command, CommandClass, CommandExecutor, CommandResult, DiffView, StateView};
pub use query::{EdgeView, EventView, ExplanationView, NodeView, SnapshotView};

/// The current ABI version string.
///
/// This is the single authoritative version identifier for all external
/// contracts.  Any change to output schema, command classification,
/// execution order, or error codes requires a version bump.
pub const ABI_VERSION: &str = "0.3";

/// The single supported external entrypoint for the Strata engine.
///
/// `StrataEngine` wraps the kernel and exposes only the five authorised
/// operations. No external code may access `Kernel`, `CausalGraph`, or
/// any internal structure directly.
///
/// ## Usage
///
/// ```ignore
/// let mut eng = StrataEngine::new();
/// eng.ingest_event(event).unwrap();
/// let state = eng.query_state();
/// let exp = eng.get_explanation("node_x", Some("color"));
/// ```
pub struct StrataEngine {
    kernel: Kernel,
}

impl StrataEngine {
    /// Initialise the engine from persisted state (event log + snapshot).
    pub fn new() -> Self {
        StrataEngine { kernel: Kernel::new() }
    }

    /// Initialise the engine from an explicit event list (test helper).
    pub fn from_events(events: Vec<Event>) -> Self {
        StrataEngine { kernel: Kernel::new_test(events) }
    }
}

impl Engine for StrataEngine {
    // ── Core Operations ─────────────────────────────────────────────────────

    fn validate(&self, event: &Event) -> Result<(), KernelError> {
        self.kernel.propose(event)
    }

    fn ingest_event(&mut self, event: Event) -> Result<(), KernelError> {
        self.kernel.commit(event)
    }

    fn replay(&self, events: &[Event]) -> GraphState {
        replay(events)
    }

    fn query_state(&self) -> &GraphState {
        self.kernel.get_state()
    }

    fn get_explanation(&self, node_id: &str, property_key: Option<&str>) -> ExplanationView {
        let ex = self.kernel.explain_belief(node_id, property_key);
        ExplanationView::from_explanation(&ex)
    }

    fn export_snapshot(&self) -> Result<String, KernelError> {
        self.kernel.save_snapshot()?;
        persistence::load_snapshot()
            .map(|opt| match opt {
                Some((snap, _)) => serde_json::to_string_pretty(&snap)
                    .unwrap_or_else(|_| "{}".to_string()),
                None => "{}".to_string(),
            })
    }

    // ── State Queries ───────────────────────────────────────────────────────

    fn get_node(&self, id: &str) -> Option<NodeView> {
        self.kernel.state.nodes.get(id).map(|n| NodeView::from_node(n))
    }

    fn get_edge(&self, id: &str) -> Option<EdgeView> {
        self.kernel.state.edges.get(id).map(|e| EdgeView::from_edge(e))
    }

    fn list_nodes(&self) -> Vec<NodeView> {
        self.kernel.state.nodes.values().map(|n| NodeView::from_node(n)).collect()
    }

    fn list_edges(&self) -> Vec<EdgeView> {
        self.kernel.state.edges.values().map(|e| EdgeView::from_edge(e)).collect()
    }

    // ── History Queries ─────────────────────────────────────────────────────

    fn event_by_id(&self, id: &str) -> Option<EventView> {
        self.kernel.prior_events.iter().find(|e| e.id == id).map(|e| EventView::from_event(e))
    }

    fn events_for_node(&self, node_id: &str) -> Vec<EventView> {
        self.kernel.prior_events
            .iter()
            .filter(|e| query::event_targets_node(e, node_id))
            .map(|e| EventView::from_event(e))
            .collect()
    }

    fn events_between(&self, start: u64, end: u64) -> Vec<EventView> {
        self.kernel.prior_events
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .map(|e| EventView::from_event(e))
            .collect()
    }

    fn latest_events(&self, n: usize) -> Vec<EventView> {
        self.kernel.prior_events
            .iter()
            .rev()
            .take(n)
            .map(|e| EventView::from_event(e))
            .collect()
    }

    // ── Explanation Queries ─────────────────────────────────────────────────

    fn causal_chain(&self, event_id: &str) -> Vec<CausalChainLink> {
        self.kernel.trace_causal_chain(event_id)
    }

    // ── Snapshot Queries ─────────────────────────────────────────────────────

    fn get_snapshot_metadata(&self) -> SnapshotView {
        SnapshotView {
            kernel_version: CURRENT_KERNEL_VERSION.to_string(),
            schema_version: CURRENT_SCHEMA_VERSION.to_string(),
            last_event_timestamp: self.kernel.event_count,
            node_count: self.kernel.state.node_count(),
            edge_count: self.kernel.state.edge_count(),
        }
    }
}
