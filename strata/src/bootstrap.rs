use std::collections::{BTreeMap, BTreeSet};

use crate::api::command::{Command, CommandClass, CommandExecutor, CommandResult};
use crate::api::envelope::{CommandEnvelope, ResultEnvelope};
use crate::api::StrataEngine;
use crate::cli::CliCommand;
use crate::kernel::event::{Event, EventType};
use crate::kernel::error::KernelError;
use crate::kernel::version::SchemaVersion;
use crate::persistence;
use crate::CURRENT_SCHEMA_VERSION;

/// The single canonical system entrypoint for Strata.
///
/// `Bootstrap` owns the `StrataEngine` and provides the only valid path
/// from raw input to executed result. All execution routes through
/// `CommandEnvelope → dispatch → CommandExecutor → Engine`.
///
/// ## Determinism
///
/// - Initialization is deterministic: `Bootstrap::new()` always loads from
///   persisted state and produces identical state for identical inputs.
/// - Trace logging captures only deterministic events (command received,
///   command type, execution start/end, result type).
/// - No heuristics, inference, or hidden state introspection.
pub struct Bootstrap {
    engine: StrataEngine,
    event_counter: u64,
    trace_enabled: bool,
}

impl Bootstrap {
    /// Create a new bootstrap instance, initialising the engine from
    /// persisted state (snapshot + event log replay).
    pub fn new() -> Self {
        Self::new_with_trace(false)
    }

    /// Create a new bootstrap instance with deterministic trace logging.
    pub fn new_with_trace(trace_enabled: bool) -> Self {
        let engine = StrataEngine::new();
        Bootstrap {
            engine,
            event_counter: 0,
            trace_enabled,
        }
    }

    /// Create a bootstrap from an explicit event list (test helper).
    /// Uses `StrataEngine::from_events` for deterministic test setup.
    pub fn from_events(events: Vec<Event>) -> Self {
        let engine = StrataEngine::from_events(events);
        Bootstrap {
            engine,
            event_counter: 0,
            trace_enabled: false,
        }
    }

    /// Enable or disable trace logging.
    pub fn set_trace(&mut self, enabled: bool) {
        self.trace_enabled = enabled;
    }

    // ── Primary Execution Path (envelope-based) ──────────────────────────

    /// Execute a command envelope through the canonical pipeline.
    ///
    /// ## Pipeline
    ///
    /// ```text
    /// CommandEnvelope → dispatch → (CommandExecutor → Engine | system handler) → ResultEnvelope
    /// ```
    ///
    /// Every request returns a `ResultEnvelope` with the paired `request_id`.
    /// No raw `CommandResult` is ever returned to the caller.
    pub fn execute(&mut self, envelope: CommandEnvelope) -> ResultEnvelope {
        let request_id = envelope.request_id;
        if self.trace_enabled {
            eprintln!("[trace] envelope received: version={}", envelope.version);
        }
        let result = self.dispatch(envelope.command);
        if self.trace_enabled {
            eprintln!("[trace] result type: {:?}", std::mem::discriminant(&result));
        }
        ResultEnvelope::new(request_id, result)
    }

    // ── Backward-Compatible Entry Point ──────────────────────────────────

    /// Execute a CLI command (wraps in envelope internally).
    ///
    /// Prefer `execute()` for new code.  This method exists so existing
    /// integration tests do not require envelope boilerplate.
    pub fn run(&mut self, cli_cmd: CliCommand) -> CommandResult {
        if self.trace_enabled {
            eprintln!("[trace] command received");
            eprintln!("[trace] command type: {:?}", cli_cmd.kind_str());
        }
        let command = self.convert(cli_cmd);
        let result = self.dispatch(command);
        if self.trace_enabled {
            eprintln!("[trace] result type: {:?}", std::mem::discriminant(&result));
        }
        result
    }

    /// Return a reference to the underlying engine (for testing only).
    #[doc(hidden)]
    pub fn engine(&self) -> &StrataEngine {
        &self.engine
    }

    /// Convert a CLI command into a domain Command.
    ///
    /// This is a separate step so that the caller can wrap the resulting
    /// `Command` in a `CommandEnvelope` with its own `request_id` before
    /// calling `execute()`.
    pub fn convert(&mut self, cli: CliCommand) -> Command {
        match cli {
            CliCommand::CreateNode { id } => {
                Command::Ingest(self.make_event(EventType::CreateNode, serde_json::json!({"id": id})))
            }
            CliCommand::CreateEdge { id, from, to, r#type } => {
                Command::Ingest(self.make_event(
                    EventType::CreateEdge,
                    serde_json::json!({"id": id, "from": from, "to": to, "type": r#type}),
                ))
            }
            CliCommand::SetProperty { target, key, value } => {
                let val = Self::parse_value(&value);
                Command::Ingest(self.make_event(
                    EventType::SetProperty,
                    serde_json::json!({"target_id": target, "key": key, "value": val}),
                ))
            }
            CliCommand::DeleteNode { id } => {
                Command::Ingest(self.make_event(EventType::DeleteNode, serde_json::json!({"id": id})))
            }
            CliCommand::DeleteEdge { id } => {
                Command::Ingest(self.make_event(EventType::DeleteEdge, serde_json::json!({"id": id})))
            }
            CliCommand::Replay => {
                let events = persistence::load_all_events().unwrap_or_default();
                Command::Replay(events)
            }
            CliCommand::ShowState => Command::QueryState,
            CliCommand::SaveSnapshot => Command::ExportSnapshot,
            CliCommand::Explain { node_id, property_key } => {
                Command::Explain { node_id, property_key }
            }
            CliCommand::Trace { event_id } => Command::CausalChain(event_id),

            // ── System commands (map to Command variants) ────────────────
            CliCommand::Version => Command::GetVersion,
            CliCommand::SchemaVersion => Command::GetSchemaVersion,
            CliCommand::ValidateLog => Command::ValidateLog,
            CliCommand::ReplayCheck => Command::ReplayCheck,
            CliCommand::WorkflowList => Command::WorkflowList,
            CliCommand::WorkflowRun { name } => Command::WorkflowRun(name),
            CliCommand::WorkflowValidate => Command::WorkflowValidate,
            CliCommand::ListCommands => Command::ListCommands,
            CliCommand::Describe { command } => Command::Describe(command),
        }
    }

    // ── Private: Command dispatch ────────────────────────────────────────

    fn dispatch(&mut self, command: Command) -> CommandResult {
        match command.class() {
            CommandClass::Execution => {
                // Execution commands must go through the engine executor.
                // They are the only path that mutates engine state.
                match command {
                    Command::Ingest(event) => self.exec(Command::Ingest(event)),
                    _ => {
                        // Safety: class() guarantees only Ingest is Execution.
                        unreachable!("unexpected execution command variant")
                    }
                }
            }
            CommandClass::Query => {
                // Query commands go through the engine executor but must
                // never reach a mutation path. The executor takes &mut Engine
                // for interface compatibility but every query method uses &self.
                match command {
                    Command::Validate(event) => self.exec(Command::Validate(event)),
                    Command::Replay(events) => self.exec(Command::Replay(events)),
                    Command::QueryState => self.exec(Command::QueryState),
                    Command::Explain { node_id, property_key } => {
                        self.exec(Command::Explain { node_id, property_key })
                    }
                    Command::CausalChain(id) => self.exec(Command::CausalChain(id)),
                    Command::ExportSnapshot => self.exec(Command::ExportSnapshot),
                    Command::GetNode(id) => self.exec(Command::GetNode(id)),
                    Command::GetEdge(id) => self.exec(Command::GetEdge(id)),
                    Command::ListNodes => self.exec(Command::ListNodes),
                    Command::ListEdges => self.exec(Command::ListEdges),
                    Command::EventById(id) => self.exec(Command::EventById(id)),
                    Command::EventsForNode(id) => self.exec(Command::EventsForNode(id)),
                    Command::EventsBetween { start, end } => {
                        self.exec(Command::EventsBetween { start, end })
                    }
                    Command::LatestEvents(n) => self.exec(Command::LatestEvents(n)),
                    Command::SnapshotMetadata => self.exec(Command::SnapshotMetadata),
                    _ => {
                        unreachable!("unexpected query command variant")
                    }
                }
            }
            CommandClass::System => {
                // System commands bypass the engine entirely.
                // They return metadata, diagnostics, or workflow results.
                match command {
                    Command::GetVersion => CommandResult::Message("1.0".into()),
                    Command::GetSchemaVersion => {
                        CommandResult::Message(CURRENT_SCHEMA_VERSION.to_string())
                    }
                    Command::ValidateLog => self.validate_log(),
                    Command::ReplayCheck => self.replay_check(),
                    Command::WorkflowList => Self::workflow_list(),
                    Command::WorkflowRun(name) => Self::workflow_run(&name),
                    Command::WorkflowValidate => Self::workflow_validate(),
                    Command::ListCommands => Self::list_commands(),
                    Command::Describe(cmd) => Self::describe(&cmd),
                    _ => unreachable!("unexpected system command variant"),
                }
            }
        }
    }

    // ── Private: System command handlers ─────────────────────────────────

    fn validate_log(&self) -> CommandResult {
        use std::collections::BTreeSet;

        let envelopes = match persistence::load_envelopes() {
            Ok(e) => e,
            Err(e) => return CommandResult::Error(e),
        };

        let mut errors: Vec<String> = Vec::new();
        let mut seen_ids: BTreeSet<String> = BTreeSet::new();
        let mut prev_ts: u64 = 0;
        let mut event_map: BTreeMap<String, &Event> = BTreeMap::new();
        let events: Vec<&Event> = envelopes.iter().map(|env| &env.event).collect();

        // Phase 1: Schema version check (all envelopes must match current schema)
        for (i, env) in envelopes.iter().enumerate() {
            if env.schema_version != SchemaVersion::default() {
                errors.push(format!(
                    "envelope[{}]: schema version mismatch (got {}, expected {})",
                    i, env.schema_version, SchemaVersion::default()
                ));
            }
        }
        if !errors.is_empty() {
            return CommandResult::Error(KernelError::ValidationError(
                errors.join("\n"),
            ));
        }

        // Phase 2: Collect events and check for duplicate IDs
        for e in &events {
            if !seen_ids.insert(e.id.clone()) {
                errors.push(format!("duplicate event id: '{}'", e.id));
            }
            event_map.insert(e.id.clone(), e);
        }

        // Phase 3: Timestamp monotonicity (strictly non-decreasing within load order)
        for e in &events {
            if e.timestamp < prev_ts {
                errors.push(format!(
                    "out-of-order timestamp: '{}' has ts={} < prev ts={}",
                    e.id, e.timestamp, prev_ts
                ));
            }
            prev_ts = e.timestamp;
        }

        // Phase 4: Causal reference validation
        for e in &events {
            for cause_id in &e.causes {
                match event_map.get(cause_id) {
                    None => {
                        errors.push(format!(
                            "orphan causal reference: '{}' references nonexistent event '{}'",
                            e.id, cause_id
                        ));
                    }
                    Some(cause) => {
                        // Cause must not have a timestamp after the dependent event
                        if cause.timestamp > e.timestamp {
                            errors.push(format!(
                                "causal ordering violation: cause '{}' (ts={}) after dependent '{}' (ts={})",
                                cause_id, cause.timestamp, e.id, e.timestamp
                            ));
                        }
                    }
                }
            }
        }

        // Phase 5: Payload integrity (non-empty IDs in payload fields)
        for e in &events {
            if e.event_type == EventType::CreateNode || e.event_type == EventType::DeleteNode {
                if e.payload.get("id").and_then(|v| v.as_str()).map_or(true, |s| s.is_empty()) {
                    errors.push(format!(
                        "invalid payload: '{}' has empty or missing node id",
                        e.id
                    ));
                }
            }
            if e.event_type == EventType::CreateEdge {
                if e.payload.get("id").and_then(|v| v.as_str()).map_or(true, |s| s.is_empty()) {
                    errors.push(format!("invalid payload: '{}' has empty or missing edge id", e.id));
                }
                if e.payload.get("from").and_then(|v| v.as_str()).map_or(true, |s| s.is_empty()) {
                    errors.push(format!("invalid payload: '{}' has empty or missing 'from'", e.id));
                }
                if e.payload.get("to").and_then(|v| v.as_str()).map_or(true, |s| s.is_empty()) {
                    errors.push(format!("invalid payload: '{}' has empty or missing 'to'", e.id));
                }
            }
            if e.event_type == EventType::SetProperty {
                if e.payload.get("target_id").and_then(|v| v.as_str()).map_or(true, |s| s.is_empty()) {
                    errors.push(format!(
                        "invalid payload: '{}' has empty or missing target_id",
                        e.id
                    ));
                }
                if e.payload.get("key").and_then(|v| v.as_str()).map_or(true, |s| s.is_empty()) {
                    errors.push(format!("invalid payload: '{}' has empty or missing key", e.id));
                }
            }
        }

        if errors.is_empty() {
            CommandResult::Message(format!(
                "  log valid: {} events loaded, all checks passed",
                events.len()
            ))
        } else {
            CommandResult::Error(KernelError::ValidationError(
                format!(
                    "log integrity check failed ({} error(s)):\n{}",
                    errors.len(),
                    errors.join("\n")
                )
            ))
        }
    }

    fn replay_check(&mut self) -> CommandResult {
        match persistence::load_all_events() {
            Ok(events) => {
                let state = self.exec(Command::Replay(events));
                let replay_line = match &state {
                    CommandResult::Replay(view) => {
                        format!(
                            "  replay: {} events, {} nodes, {} edges",
                            view.node_count() + view.edge_count(),
                            view.node_count(),
                            view.edge_count()
                        )
                    }
                    _ => "  replay: failed".into(),
                };
                let snapshot_line = match persistence::load_snapshot() {
                    Ok(Some((snap, _))) => {
                        let replayed_view = match &state {
                            CommandResult::Replay(v) => v,
                            _ => {
                                return CommandResult::Message(
                                    format!("{}\n  snapshot: (replay failed)", replay_line));
                            }
                        };
                        let snap_nodes: Vec<crate::NodeView> = snap
                            .state
                            .nodes
                            .values()
                            .map(|n| crate::NodeView::from_node(n))
                            .collect();
                        let snap_edges: Vec<crate::EdgeView> = snap
                            .state
                            .edges
                            .values()
                            .map(|e| crate::EdgeView::from_edge(e))
                            .collect();
                        if snap_nodes.len() == replayed_view.nodes.len()
                            && snap_edges.len() == replayed_view.edges.len()
                        {
                            format!("{}\n  snapshot: MATCH", replay_line)
                        } else {
                            format!("{}\n  snapshot: MISMATCH", replay_line)
                        }
                    }
                    Ok(None) => format!("{}\n  snapshot: (none available)", replay_line),
                    Err(e) => format!("{}\n  snapshot error: {}", replay_line, e),
                };
                CommandResult::Message(snapshot_line)
            }
            Err(e) => CommandResult::Error(e),
        }
    }

    fn workflow_list() -> CommandResult {
        let names = crate::workflow::list();
        let mut out = "  available workflows:\n".to_string();
        for w in names {
            out.push_str(&format!("    - {}\n", w));
        }
        CommandResult::Message(out.trim_end().to_string())
    }

    fn workflow_run(name: &str) -> CommandResult {
        let pass = crate::workflow::run(name);
        if pass {
            CommandResult::Message(format!("  workflow '{}': PASS", name))
        } else {
            CommandResult::Message(format!("  workflow '{}': FAIL", name))
        }
    }

    fn workflow_validate() -> CommandResult {
        let mut all_pass = true;
        let mut out = String::new();
        for w in crate::workflow::list() {
            out.push_str(&format!("  {} ... ", w));
            let pass = crate::workflow::run(w);
            out.push_str(if pass { "PASS\n" } else { "FAIL\n" });
            if !pass {
                all_pass = false;
            }
        }
        if all_pass {
            out.push_str("  all workflows: PASS");
        } else {
            out.push_str("  all workflows: FAIL (some failed)");
        }
        CommandResult::Message(out.trim_end().to_string())
    }

    fn list_commands() -> CommandResult {
        let mut out = String::new();
        for cmd in crate::describe::all_commands() {
            out.push_str(&format!(
                "  {:<20} {}  [{}]\n",
                cmd.name, cmd.summary, cmd.category
            ));
        }
        CommandResult::Message(out.trim_end().to_string())
    }

    fn describe(cmd_name: &str) -> CommandResult {
        match crate::describe::find_command(cmd_name) {
            Some(desc) => {
                let mut out = format!("  {} — {}\n", desc.name, desc.summary);
                out.push_str(&format!("  Category: {}\n", desc.category));
                out.push_str("  Inputs:\n");
                if desc.inputs.is_empty() {
                    out.push_str("    (none)\n");
                } else {
                    for input in desc.inputs {
                        let req = if input.optional { "optional" } else { "required" };
                        out.push_str(&format!(
                            "    {} ({}, {}) — {}\n",
                            input.name, input.kind, req, input.description
                        ));
                    }
                }
                out.push_str(&format!("  Output: {}\n", desc.output));
                CommandResult::Message(out.trim_end().to_string())
            }
            None => CommandResult::Message(format!(
                "  unknown command '{}'. Use 'list-commands' to see all available commands.",
                cmd_name
            )),
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn exec(&mut self, command: Command) -> CommandResult {
        if self.trace_enabled {
            eprintln!("[trace] execution start: {:?}", std::mem::discriminant(&command));
        }
        let mut executor = CommandExecutor::new(&mut self.engine);
        let result = executor.execute(command);
        if self.trace_enabled {
            eprintln!("[trace] execution end");
        }
        result
    }

    fn make_event(&mut self, event_type: EventType, payload: serde_json::Value) -> Event {
        self.event_counter += 1;
        let id = format!("evt-{}", self.event_counter);
        Event::new(id, 0, event_type, payload)
    }

    fn parse_value(s: &str) -> serde_json::Value {
        serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.to_string()))
    }
}
