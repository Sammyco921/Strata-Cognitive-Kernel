use clap::{Parser, Subcommand};

/// Strata CLI — pure adapter layer.
///
/// This module is responsible ONLY for parsing command-line arguments into
/// `CliCommand` values. It has zero knowledge of the Engine, Kernel,
/// persistence, or any execution internals.
///
/// Every `CliCommand` is forwarded to `Bootstrap::run()` which maps it to
/// an API `Command` and executes it via `CommandExecutor`.
#[derive(Parser)]
#[command(name = "strata", version = "0.2.0", about = "Deterministic causal event-sourced graph kernel")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

/// All valid CLI commands.
///
/// Each variant maps to either an API `Command` (engine operation) or a
/// system command (version, schema, workflow, etc.). Mapping is done in
/// `Bootstrap::run()` — never in this module.
#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Create a new node
    CreateNode { id: String },
    /// Create a new edge between two nodes
    CreateEdge {
        id: String,
        from: String,
        to: String,
        r#type: String,
    },
    /// Set a property on a node or edge
    SetProperty {
        target: String,
        key: String,
        value: String,
    },
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
    /// Explain a belief on a node (trace causal chain)
    Explain {
        node_id: String,
        property_key: Option<String>,
    },
    /// Trace the causal chain of an event
    Trace { event_id: String },
    /// Display kernel version
    Version,
    /// Display schema version
    SchemaVersion,
    /// Validate event log integrity
    ValidateLog,
    /// Replay event log and verify consistency with snapshot
    ReplayCheck,
    /// List available workflows
    WorkflowList,
    /// Run a named workflow
    WorkflowRun { name: String },
    /// Run all workflows and report results
    WorkflowValidate,
    /// List all available CLI commands
    ListCommands,
    /// Show detailed info about a specific command
    Describe { command: String },
}

impl CliCommand {
    /// Return a short string identifying the command category, for trace logging.
    pub fn kind_str(&self) -> &'static str {
        match self {
            CliCommand::CreateNode { .. } => "CreateNode",
            CliCommand::CreateEdge { .. } => "CreateEdge",
            CliCommand::SetProperty { .. } => "SetProperty",
            CliCommand::DeleteNode { .. } => "DeleteNode",
            CliCommand::DeleteEdge { .. } => "DeleteEdge",
            CliCommand::Replay => "Replay",
            CliCommand::ShowState => "ShowState",
            CliCommand::SaveSnapshot => "SaveSnapshot",
            CliCommand::Explain { .. } => "Explain",
            CliCommand::Trace { .. } => "Trace",
            CliCommand::Version => "Version",
            CliCommand::SchemaVersion => "SchemaVersion",
            CliCommand::ValidateLog => "ValidateLog",
            CliCommand::ReplayCheck => "ReplayCheck",
            CliCommand::WorkflowList => "WorkflowList",
            CliCommand::WorkflowRun { .. } => "WorkflowRun",
            CliCommand::WorkflowValidate => "WorkflowValidate",
            CliCommand::ListCommands => "ListCommands",
            CliCommand::Describe { .. } => "Describe",
        }
    }
}
