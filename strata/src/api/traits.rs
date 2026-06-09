use crate::kernel::error::KernelError;
use crate::kernel::event::Event;
use crate::kernel::graph::GraphState;
use crate::projection::causal::CausalChainLink;

use crate::api::query::{EdgeView, EventView, ExplanationView, NodeView, SnapshotView};

/// The Engine trait defines the stable public contract for Strata.
///
/// Every implementation must satisfy:
/// - Determinism: identical event sequences produce identical state
/// - Side-effect isolation: mutations only occur through ingest_event
/// - Projection purity: derived structures never feed back into kernel
pub trait Engine {
    // ── Core Operations ─────────────────────────────────────────────────────

    /// Validate an event against current kernel state without committing it.
    /// Returns Ok(()) if the event would be accepted, or a KernelError
    /// describing the violation (duplicate node, missing reference, etc.).
    fn validate(&self, event: &Event) -> Result<(), KernelError>;

    /// Validate and persist an event to the G₀ log, then update kernel state.
    fn ingest_event(&mut self, event: Event) -> Result<(), KernelError>;

    /// Replay a sequence of events and return the resulting state.
    /// This is a pure read operation — no side-effects, no persistence.
    fn replay(&self, events: &[Event]) -> GraphState;

    /// Return a read-only reference to the current G₀ state.
    fn query_state(&self) -> &GraphState;

    /// Generate an explanation for a node's property value and return a stable DTO.
    /// Traces the causal chain (G₁) anchoring every link to a G₀ event.
    fn get_explanation(&self, node_id: &str, property_key: Option<&str>) -> ExplanationView;

    /// Export a JSON snapshot of the current kernel state.
    fn export_snapshot(&self) -> Result<String, KernelError>;

    // ── State Queries ───────────────────────────────────────────────────────

    /// Look up a single node by ID.
    /// Returns `None` if the node does not exist.
    fn get_node(&self, id: &str) -> Option<NodeView>;

    /// Look up a single edge by ID.
    /// Returns `None` if the edge does not exist.
    fn get_edge(&self, id: &str) -> Option<EdgeView>;

    /// Return all nodes in the current graph state.
    fn list_nodes(&self) -> Vec<NodeView>;

    /// Return all edges in the current graph state.
    fn list_edges(&self) -> Vec<EdgeView>;

    // ── History Queries ──────────────────────────────────────────────────────

    /// Look up a single event by its ID.
    /// Returns `None` if the event does not exist in the log.
    fn event_by_id(&self, id: &str) -> Option<EventView>;

    /// Return all events whose payload references the given node ID.
    /// Matches CreateNode, DeleteNode, SetProperty (target_id), and
    /// CreateEdge (from/to). DeleteEdge events cannot be mapped to a
    /// node from event data alone and are excluded.
    fn events_for_node(&self, node_id: &str) -> Vec<EventView>;

    /// Return events whose timestamp falls in the inclusive range [start, end].
    fn events_between(&self, start: u64, end: u64) -> Vec<EventView>;

    /// Return the most recent `n` events from the event log.
    /// If fewer than `n` events exist, returns all events.
    fn latest_events(&self, n: usize) -> Vec<EventView>;

    // ── Explanation Queries ──────────────────────────────────────────────────

    /// Trace the causal chain for a specific event ID.
    /// Returns the chain of events leading to the given event.
    fn causal_chain(&self, event_id: &str) -> Vec<CausalChainLink>;

    // ── Snapshot Queries ─────────────────────────────────────────────────────

    /// Return metadata about the current kernel state (version info + counts).
    /// Always returns data — there is no failure path.
    fn get_snapshot_metadata(&self) -> SnapshotView;
}
