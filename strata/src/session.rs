use serde::Serialize;

use crate::api::envelope::TraceId;

/// Actions the REPL loop can take after handling a REPL-local command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplAction {
    /// Continue the REPL loop.
    Continue,
    /// Exit the REPL loop cleanly.
    Exit,
}

/// Deterministic summary of a session's execution history.
///
/// Excludes all kernel state — contains only session metadata and trace
/// ordering.  Satisfies I3 (Session is observational only).
#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    /// Unique session identifier (deterministic per seed).
    pub session_id: u64,
    /// Number of commands executed in this session.
    pub command_count: u64,
    /// Ordered list of trace IDs, one per executed command.
    pub trace_ids: Vec<TraceId>,
}

/// Observational session tracker for the REPL layer.
///
/// `SessionManager` records execution metadata only.  It has no authority
/// over kernel state, event validation, command classification, or
/// execution semantics.  It is purely an interaction-layer concern.
///
/// ## Invariants
///
/// - **I3 — Session is observational only**: records metadata, groups
///   traces, tracks ordering.  Never mutates kernel state or influences
///   execution.
/// - **I5 — REPL is a thin orchestration loop**: session state does not
///   leak into kernel, command execution, or result computation.
pub struct SessionManager {
    session_id: u64,
    trace_ids: Vec<TraceId>,
    command_count: u64,
}

impl SessionManager {
    /// Create a new session with `session_id = 0`.
    pub fn new() -> Self {
        Self::new_with_seed(0)
    }

    /// Create a new session with an explicit seed.
    ///
    /// The same seed always produces the same `session_id`, making
    /// sessions deterministic for testing.
    pub fn new_with_seed(seed: u64) -> Self {
        SessionManager {
            session_id: seed,
            trace_ids: Vec::new(),
            command_count: 0,
        }
    }

    /// Return the session identifier.
    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Record a completed command's trace ID.
    ///
    /// Called after execution so the trace_id is known.  The trace_id is
    /// appended to the ordered history.
    pub fn record_trace(&mut self, trace_id: TraceId) {
        self.trace_ids.push(trace_id);
        self.command_count += 1;
    }

    /// Return the ordered list of trace IDs recorded so far.
    pub fn trace_ids(&self) -> &[TraceId] {
        &self.trace_ids
    }

    /// Return the number of commands executed in this session.
    pub fn command_count(&self) -> u64 {
        self.command_count
    }

    /// Produce a deterministic summary of the session.
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            session_id: self.session_id,
            command_count: self.command_count,
            trace_ids: self.trace_ids.clone(),
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handles a REPL-local command (starting with `:`).
///
/// Returns `(ReplAction, Option<String>)` where the string is the output
/// the caller should print (if any).  This makes the function testable
/// without performing I/O.
///
/// ## REPL-local commands
///
/// | Command      | Behaviour                                    |
/// |--------------|----------------------------------------------|
/// | `:exit`      | Terminate the session cleanly                |
/// | `:session`   | Print session summary as JSON                |
/// | `:trace`     | Print collected trace IDs                    |
/// | `:help`      | Show available commands                      |
pub fn handle_repl_command(input: &str, session: &SessionManager) -> Result<(ReplAction, Option<String>), String> {
    let trimmed = input.trim();
    match trimmed {
        ":exit" => Ok((ReplAction::Exit, None)),
        ":session" => {
            let summary = session.summary();
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|e| format!("serialization error: {}", e))?;
            Ok((ReplAction::Continue, Some(json)))
        }
        ":trace" => {
            let output = format!("trace_ids: {:?}", session.trace_ids());
            Ok((ReplAction::Continue, Some(output)))
        }
        ":help" => {
            let help = concat!(
                "REPL commands:\n",
                "  :exit     Exit the REPL\n",
                "  :session  Print session summary (JSON)\n",
                "  :trace    Print collected trace IDs\n",
                "  :help     Show this help message\n",
                "\n",
                "Any standard Strata CLI subcommand may be entered directly.\n",
                "Example: create-node my-node\n",
            );
            Ok((ReplAction::Continue, Some(help.to_string())))
        }
        other => Err(format!("unknown REPL command: '{}'. Type :help for available commands.", other)),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_manager_new_has_session_id_zero() {
        let sm = SessionManager::new();
        assert_eq!(sm.session_id(), 0);
        assert_eq!(sm.command_count(), 0);
        assert!(sm.trace_ids().is_empty());
    }

    #[test]
    fn session_manager_new_with_seed_uses_seed() {
        let sm = SessionManager::new_with_seed(42);
        assert_eq!(sm.session_id(), 42);
    }

    #[test]
    fn record_trace_appends_to_history() {
        let mut sm = SessionManager::new();
        sm.record_trace(TraceId(10));
        sm.record_trace(TraceId(20));
        sm.record_trace(TraceId(30));

        assert_eq!(sm.command_count(), 3);
        assert_eq!(sm.trace_ids(), &[TraceId(10), TraceId(20), TraceId(30)]);
    }

    #[test]
    fn trace_ordering_preserved() {
        let mut sm = SessionManager::new();
        for i in 0..5 {
            sm.record_trace(TraceId(i));
        }
        let ids = sm.trace_ids();
        for i in 0..5 {
            assert_eq!(ids[i], TraceId(i as u64), "trace order must be preserved at position {}", i);
        }
    }

    #[test]
    fn summary_contains_all_metadata() {
        let mut sm = SessionManager::new_with_seed(99);
        sm.record_trace(TraceId(1));
        sm.record_trace(TraceId(2));

        let summary = sm.summary();
        assert_eq!(summary.session_id, 99);
        assert_eq!(summary.command_count, 2);
        assert_eq!(summary.trace_ids, vec![TraceId(1), TraceId(2)]);
    }

    #[test]
    fn session_summary_serializes_deterministically() {
        let mut sm = SessionManager::new_with_seed(7);
        sm.record_trace(TraceId(100));
        sm.record_trace(TraceId(200));

        let a = serde_json::to_string(&sm.summary()).unwrap();
        let b = serde_json::to_string(&sm.summary()).unwrap();
        assert_eq!(a, b, "summary serialization must be deterministic");
    }

    #[test]
    fn handle_repl_exit_returns_exit_action() {
        let sm = SessionManager::new();
        let (action, output) = handle_repl_command(":exit", &sm).unwrap();
        assert_eq!(action, ReplAction::Exit);
        assert!(output.is_none());
    }

    #[test]
    fn handle_repl_help_returns_help_text() {
        let sm = SessionManager::new();
        let (action, output) = handle_repl_command(":help", &sm).unwrap();
        assert_eq!(action, ReplAction::Continue);
        let text = output.expect(":help should produce output");
        assert!(text.contains(":exit"), "help must mention :exit");
        assert!(text.contains("create-node"), "help must mention create-node");
    }

    #[test]
    fn handle_repl_session_returns_json_summary() {
        let mut sm = SessionManager::new();
        sm.record_trace(TraceId(42));
        let (action, output) = handle_repl_command(":session", &sm).unwrap();
        assert_eq!(action, ReplAction::Continue);
        let json = output.expect(":session should produce JSON");
        assert!(json.contains("\"session_id\": 0"), "JSON should contain session_id");
        assert!(json.contains("\"command_count\": 1"), "JSON should contain command_count");
        assert!(json.contains("\"trace_ids\":"), "JSON should contain trace_ids");
    }

    #[test]
    fn handle_repl_trace_returns_trace_list() {
        let mut sm = SessionManager::new();
        sm.record_trace(TraceId(7));
        sm.record_trace(TraceId(8));
        let (action, output) = handle_repl_command(":trace", &sm).unwrap();
        assert_eq!(action, ReplAction::Continue);
        let text = output.expect(":trace should produce output");
        assert!(text.contains("7"), "trace output must contain trace_id 7");
        assert!(text.contains("8"), "trace output must contain trace_id 8");
    }

    #[test]
    fn handle_repl_unknown_command_returns_error() {
        let sm = SessionManager::new();
        let result = handle_repl_command(":unknown", &sm);
        assert!(result.is_err(), "unknown REPL command must return error");
        assert!(result.unwrap_err().contains(":unknown"));
    }

    #[test]
    fn handle_repl_non_colon_input_is_error() {
        let sm = SessionManager::new();
        // Input without leading colon is not a REPL command — it should
        // not be routed here at all.  But if it is, it should be an error.
        let result = handle_repl_command("show-state", &sm);
        assert!(result.is_err());
    }

    #[test]
    fn repeated_sessions_produce_consistent_traces() {
        let mut sm1 = SessionManager::new_with_seed(0);
        let mut sm2 = SessionManager::new_with_seed(0);

        for i in 0..3 {
            sm1.record_trace(TraceId(i));
        }
        for i in 0..3 {
            sm2.record_trace(TraceId(i));
        }

        assert_eq!(sm1.trace_ids(), sm2.trace_ids(),
            "same commands in same order must produce identical traces");
    }
}
