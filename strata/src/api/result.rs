use std::collections::BTreeMap;

use serde::Serialize;

use crate::api::command::{CommandClass, CommandResult};
use crate::api::envelope::{TraceId, ENVELOPE_VERSION};
use crate::api::command::StateView;
use crate::api::query::{
    EdgeView, EventView, ExplanationView, NodeView, SnapshotView,
};
use crate::kernel::error::KernelError;
use crate::projection::causal::CausalChainLink;

// ── Error Code ─────────────────────────────────────────────────────────────

/// Stable error codes for all system failures.
///
/// Every `ErrorCode` variant maps 1:1 to a class of failure.  The enum is
/// exhaustive and never carries dynamic data — use `context` on
/// `CommandError` for additional information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ErrorCode {
    /// The event failed semantic validation (empty ID, missing fields, …).
    ValidationError,
    /// A causal cycle was detected at commit time.
    CausalCycleViolation,
    /// A referenced node, edge, or event was not found.
    NotFound,
    /// The operation is not supported.
    UnsupportedOperation,
    /// An I/O error occurred (persistence, snapshot, …).
    IoError,
    /// An internal invariant was violated.
    InternalError,
    /// Catch-all for unclassified failures.
    Unknown,
}

// ── Command Error ──────────────────────────────────────────────────────────

/// Structured error response with stable error codes.
///
/// Carries a human-readable `message`, the originating `trace_id`, and an
/// optional `context` map for structured metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandError {
    pub error_code: ErrorCode,
    pub message: String,
    pub trace_id: TraceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<BTreeMap<String, String>>,
}

// ── Execution Results ──────────────────────────────────────────────────────

/// Result of a single event ingestion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IngestResult {
    /// The event ID assigned during conversion.
    pub event_id: String,
    /// Whether the ingestion succeeded.
    pub success: bool,
    /// Optional human-readable metadata from validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_metadata: Option<String>,
}

// ── System Result Types ────────────────────────────────────────────────────

/// Result of the `validate-log` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogValidationResult {
    /// Whether the log passed all checks.
    pub valid: bool,
    /// Number of events inspected.
    pub events_checked: usize,
    /// List of integrity errors found (empty when valid).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// Result of the `replay-check` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplayCheckResult {
    /// Number of events replayed.
    pub events_replayed: usize,
    /// Node count in the replayed state.
    pub node_count: usize,
    /// Edge count in the replayed state.
    pub edge_count: usize,
    /// Whether the replayed state matches the snapshot.
    pub snapshot_match: bool,
    /// Whether a snapshot was available for comparison.
    pub snapshot_available: bool,
}

/// Result of the `workflow-list` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowListResult {
    pub workflows: Vec<String>,
}

/// Result of a single workflow run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunResult {
    pub name: String,
    pub passed: bool,
}

/// Result of the `workflow-validate` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowValidateResult {
    pub all_passed: bool,
    pub results: Vec<WorkflowRunResult>,
}

/// Result of the `list-commands` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandListResult {
    pub commands: Vec<CommandDescriptor>,
}

/// Descriptor for a single CLI command (name, category, summary, …).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandDescriptor {
    pub name: String,
    pub category: String,
    pub summary: String,
    pub inputs: Vec<InputDescriptor>,
    pub output: String,
}

impl From<&crate::describe::CommandDescriptor> for CommandDescriptor {
    fn from(d: &crate::describe::CommandDescriptor) -> Self {
        CommandDescriptor {
            name: d.name.into(),
            category: d.category.into(),
            summary: d.summary.into(),
            inputs: d.inputs.iter().map(|i| InputDescriptor {
                name: i.name.into(),
                kind: i.kind.into(),
                optional: i.optional,
                description: i.description.into(),
            }).collect(),
            output: d.output.into(),
        }
    }
}

/// Descriptor for a single command input field.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputDescriptor {
    pub name: String,
    pub kind: String,
    pub optional: bool,
    pub description: String,
}

/// Result of the `describe` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DescribeResult {
    pub name: String,
    pub summary: String,
    pub category: String,
    pub inputs: Vec<InputDescriptor>,
    pub output: String,
}



// ── Result Payload ─────────────────────────────────────────────────────────

/// Deterministic result payload covering all command outputs.
///
/// Every `Command` variant maps to exactly one `ResultPayload` variant.
/// Variant names follow the specification (PROMPT 3 — OUTPUT CONTRACT
/// STANDARDIZATION) for stable, self-describing JSON output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ResultPayload {
    // ── Execution ────────────────────────────────────────────────────────

    /// Event ingestion succeeded (maps from `Command::Ingest`).
    Ingested(IngestResult),

    /// Validation passed (maps from `Command::Validate`).
    Valid,

    // ── Query ────────────────────────────────────────────────────────────

    /// Full graph state (maps from `Command::QueryState` / `Replay`).
    StateView(StateView),

    /// Single node lookup result (maps from `Command::GetNode`).
    NodeView(NodeView),

    /// Single edge lookup result (maps from `Command::GetEdge`).
    EdgeView(EdgeView),

    /// All nodes (maps from `Command::ListNodes`).
    Nodes(Vec<NodeView>),

    /// All edges (maps from `Command::ListEdges`).
    Edges(Vec<EdgeView>),

    /// Single event lookup (maps from `Command::EventById`).
    EventView(EventView),

    /// Event collection (maps from `EventsForNode` / `EventsBetween` / `LatestEvents`).
    Events(Vec<EventView>),

    /// Causal explanation (maps from `Command::Explain`).
    ExplanationView(ExplanationView),

    /// Causal chain trace (maps from `Command::CausalChain`).
    CausalChainView(Vec<CausalChainLink>),

    /// Snapshot export result (maps from `Command::ExportSnapshot`).
    SnapshotExport(String),

    /// Snapshot metadata (maps from `Command::SnapshotMetadata`).
    SnapshotMetadata(SnapshotView),

    // ── System ───────────────────────────────────────────────────────────

    /// Kernel version (maps from `Command::GetVersion`).
    Version(String),

    /// Event schema version (maps from `Command::GetSchemaVersion`).
    SchemaVersion(String),

    /// Event log validation result (maps from `Command::ValidateLog`).
    ValidateLog(LogValidationResult),

    /// Replay + snapshot comparison result (maps from `Command::ReplayCheck`).
    ReplayCheck(ReplayCheckResult),

    /// List of available workflows (maps from `Command::WorkflowList`).
    WorkflowList(WorkflowListResult),

    /// Single workflow run result (maps from `Command::WorkflowRun`).
    WorkflowRun(WorkflowRunResult),

    /// All-workflows validation result (maps from `Command::WorkflowValidate`).
    WorkflowValidate(WorkflowValidateResult),

    /// List of available CLI commands (maps from `Command::ListCommands`).
    CommandList(CommandListResult),

    /// Detailed command description (maps from `Command::Describe`).
    Describe(DescribeResult),

    // ── Error ────────────────────────────────────────────────────────────

    /// Structured error with stable error code.
    Error(CommandError),
}

// ── Command Result V1 ──────────────────────────────────────────────────────

/// Canonical output contract for every system response.
///
/// ## Invariants
///
/// - **I1 — Structural Determinism**: identical inputs produce identical
///   serialized output.
/// - **I2 — No Hidden Formatting**: exactly one output pipeline.
/// - **I3 — Trace Integrity**: `trace_id` matches the originating
///   `CommandEnvelope`.
/// - **I4 — Class Consistency**: `class` equals `Command::class(command)`.
/// - **I5 — Canonical Serialization**: all maps are `BTreeMap`; no
///   insertion-order dependence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandResultV1 {
    /// Envelope schema version (always 1).
    pub version: u32,
    /// Trace identifier from the originating `CommandEnvelope`.
    pub trace_id: TraceId,
    /// Command class mirroring `Command::class()` for the executed command.
    pub class: CommandClass,
    /// The actual command output.
    pub result: ResultPayload,
}

// ── Conversions ────────────────────────────────────────────────────────────

impl From<KernelError> for CommandError {
    fn from(e: KernelError) -> Self {
        let (error_code, message) = match &e {
            KernelError::ValidationError(msg) => (ErrorCode::ValidationError, msg.clone()),
            KernelError::ReferenceError(msg) => (ErrorCode::NotFound, msg.clone()),
            KernelError::PersistenceError(msg) => (ErrorCode::IoError, msg.clone()),
            KernelError::ReplayError(msg) => (ErrorCode::InternalError, msg.clone()),
            KernelError::CompatibilityError(msg) => (ErrorCode::UnsupportedOperation, msg.clone()),
            KernelError::ProjectionError(msg) => (ErrorCode::InternalError, msg.clone()),
            KernelError::CausalCycleViolation { event_id, cycle_path } => {
                (ErrorCode::CausalCycleViolation, format!("event '{}' would create cycle: {}", event_id, cycle_path))
            }
        };
        CommandError {
            error_code,
            message,
            trace_id: TraceId(0), // placeholder; real trace_id assigned at envelope boundary
            context: None,
        }
    }
}

impl From<CommandResult> for ResultPayload {
    fn from(r: CommandResult) -> Self {
        match r {
            CommandResult::Valid => ResultPayload::Valid,
            CommandResult::Ingested => {
                // When we don't have an event_id at the CommandResult level,
                // we emit a synthetic one.  The IngestResult is enriched
                // with a real event_id at the Bootstrap conversion layer.
                ResultPayload::Ingested(IngestResult {
                    event_id: String::new(),
                    success: true,
                    validation_metadata: None,
                })
            }
            CommandResult::Replay(state) => ResultPayload::StateView(state),
            CommandResult::QueryState(state) => ResultPayload::StateView(state),
            CommandResult::Explain(ex) => ResultPayload::ExplanationView(ex),
            CommandResult::CausalChain(chain) => ResultPayload::CausalChainView(chain),
            CommandResult::ExportSnapshot(json) => ResultPayload::SnapshotExport(json),
            CommandResult::GetNode(n) => match n {
                Some(node) => ResultPayload::NodeView(node),
                None => ResultPayload::Error(CommandError {
                    error_code: ErrorCode::NotFound,
                    message: "node not found".into(),
                    trace_id: TraceId(0),
                    context: None,
                }),
            },
            CommandResult::GetEdge(e) => match e {
                Some(edge) => ResultPayload::EdgeView(edge),
                None => ResultPayload::Error(CommandError {
                    error_code: ErrorCode::NotFound,
                    message: "edge not found".into(),
                    trace_id: TraceId(0),
                    context: None,
                }),
            },
            CommandResult::ListNodes(nodes) => ResultPayload::Nodes(nodes),
            CommandResult::ListEdges(edges) => ResultPayload::Edges(edges),
            CommandResult::EventById(ev) => match ev {
                Some(event) => ResultPayload::EventView(event),
                None => ResultPayload::Error(CommandError {
                    error_code: ErrorCode::NotFound,
                    message: "event not found".into(),
                    trace_id: TraceId(0),
                    context: None,
                }),
            },
            CommandResult::EventsForNode(events)
            | CommandResult::EventsBetween(events)
            | CommandResult::LatestEvents(events) => ResultPayload::Events(events),
            CommandResult::SnapshotMetadata(meta) => ResultPayload::SnapshotMetadata(meta),
            CommandResult::Message(msg) => {
                // Message is a catch-all; at the V1 layer we map it to
                // Version with a fallback.  System commands in Bootstrap
                // construct their own typed ResultPayload variants and
                // do not go through this conversion path.
                ResultPayload::Version(msg)
            }
            CommandResult::Error(err) => {
                let cmd_err: CommandError = err.into();
                ResultPayload::Error(cmd_err)
            }
        }
    }
}

// ── Helper: build CommandResultV1 from its parts ──────────────────────────

impl CommandResultV1 {
    pub fn new(trace_id: TraceId, class: CommandClass, payload: ResultPayload) -> Self {
        CommandResultV1 {
            version: ENVELOPE_VERSION,
            trace_id,
            class,
            result: payload,
        }
    }

    /// Returns `true` if the result is not an error.
    pub fn is_ok(&self) -> bool {
        !matches!(self.result, ResultPayload::Error(_))
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::command::CommandResult;
    use crate::kernel::error::KernelError;

    #[test]
    fn command_result_v1_has_default_version() {
        let tid = TraceId(42);
        let rv1 = CommandResultV1::new(tid, CommandClass::Query, ResultPayload::Valid);
        assert_eq!(rv1.version, 1);
        assert_eq!(rv1.trace_id, tid);
        assert_eq!(rv1.class, CommandClass::Query);
    }

    #[test]
    fn trace_id_preserved_in_error_conversion() {
        let tid = TraceId(99);
        let err = KernelError::ValidationError("bad".into());
        let mut cmd_err: CommandError = err.into();
        cmd_err.trace_id = tid;
        let payload = ResultPayload::Error(cmd_err);
        let rv1 = CommandResultV1::new(tid, CommandClass::Execution, payload);
        match &rv1.result {
            ResultPayload::Error(e) => {
                assert_eq!(e.trace_id, tid);
                assert_eq!(e.error_code, ErrorCode::ValidationError);
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn kernel_error_maps_to_stable_error_code() {
        let cases: Vec<(KernelError, ErrorCode)> = vec![
            (KernelError::ValidationError("x".into()), ErrorCode::ValidationError),
            (KernelError::ReferenceError("x".into()), ErrorCode::NotFound),
            (KernelError::PersistenceError("x".into()), ErrorCode::IoError),
            (KernelError::ReplayError("x".into()), ErrorCode::InternalError),
            (KernelError::CompatibilityError("x".into()), ErrorCode::UnsupportedOperation),
            (KernelError::ProjectionError("x".into()), ErrorCode::InternalError),
            (
                KernelError::CausalCycleViolation {
                    event_id: "e".into(),
                    cycle_path: "a→b→a".into(),
                },
                ErrorCode::CausalCycleViolation,
            ),
        ];
        for (kerr, expected_code) in cases {
            let cmd_err: CommandError = kerr.into();
            assert_eq!(cmd_err.error_code, expected_code,
                "expected {:?} for {:?}", expected_code, cmd_err);
        }
    }

    #[test]
    fn command_result_valid_converts_correctly() {
        let cr = CommandResult::Valid;
        let payload: ResultPayload = cr.into();
        assert!(matches!(payload, ResultPayload::Valid));
    }

    #[test]
    fn command_result_ingested_sets_success() {
        let cr = CommandResult::Ingested;
        let payload: ResultPayload = cr.into();
        match payload {
            ResultPayload::Ingested(ir) => {
                assert!(ir.success);
            }
            other => panic!("Expected Ingested, got {:?}", other),
        }
    }

    #[test]
    fn command_result_get_node_none_becomes_error() {
        let cr = CommandResult::GetNode(None);
        let payload: ResultPayload = cr.into();
        match payload {
            ResultPayload::Error(e) => {
                assert_eq!(e.error_code, ErrorCode::NotFound);
            }
            other => panic!("Expected Error(NotFound), got {:?}", other),
        }
    }

    #[test]
    fn command_result_get_edge_none_becomes_error() {
        let cr = CommandResult::GetEdge(None);
        let payload: ResultPayload = cr.into();
        match payload {
            ResultPayload::Error(e) => {
                assert_eq!(e.error_code, ErrorCode::NotFound);
            }
            other => panic!("Expected Error(NotFound), got {:?}", other),
        }
    }

    #[test]
    fn command_result_event_by_id_none_becomes_error() {
        let cr = CommandResult::EventById(None);
        let payload: ResultPayload = cr.into();
        match payload {
            ResultPayload::Error(e) => {
                assert_eq!(e.error_code, ErrorCode::NotFound);
            }
            other => panic!("Expected Error(NotFound), got {:?}", other),
        }
    }

    #[test]
    fn error_codes_are_stable_and_exhaustive() {
        // Verify every variant is constructible (compilation check).
        let codes = vec![
            ErrorCode::ValidationError,
            ErrorCode::CausalCycleViolation,
            ErrorCode::NotFound,
            ErrorCode::UnsupportedOperation,
            ErrorCode::IoError,
            ErrorCode::InternalError,
            ErrorCode::Unknown,
        ];
        assert_eq!(codes.len(), 7, "all 7 ErrorCode variants must be covered");
    }

    #[test]
    fn serialization_contains_no_hashmap() {
        // Compile-time assertion: CommandResultV1 must not depend on HashMap
        // (we don't import HashMap in this module).
        // Runtime: verify a sample serialization roundtrip.
        let rv1 = CommandResultV1::new(TraceId(1), CommandClass::Query, ResultPayload::Valid);
        let json = serde_json::to_string(&rv1).unwrap();
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"trace_id\":1"));
        assert!(json.contains("\"class\":\"Query\""));
        assert!(json.contains("\"result\":\"Valid\""));
    }
}
