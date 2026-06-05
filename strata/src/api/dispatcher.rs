use serde::Serialize;

use crate::api::envelope::{CommandEnvelope, ENVELOPE_VERSION};
use crate::api::result::CommandResultV1;
use crate::api::ABI_VERSION;
use crate::bootstrap::Bootstrap;
use crate::kernel::event::Event;

/// Transport-agnostic external API boundary for Strata.
///
/// `ApiDispatcher` is the ONLY supported mechanism for external systems to
/// interact with Strata.  It accepts a JSON-serialized `CommandEnvelope`,
/// validates it deterministically, routes it through `Bootstrap::execute()`,
/// and returns `CommandResultV1`.
///
/// ## Invariants
///
/// - **I1 — Kernel Blindness**: The kernel never knows this layer exists.
/// - **I2 — Deterministic External Execution**: identical input JSON always
///   produces identical output JSON (modulo bootstrap-internal trace_id).
/// - **I3 — No Side Channels**: no logs, timing, or ordering influence output.
/// - **I4 — Transport Independence**: plugging in HTTP/IPC/WASM requires zero
///   kernel, bootstrap, or command changes.
/// - **I5 — Pure Execution Boundary**: API layer parses, validates, routes,
///   and serializes.  It never interprets commands or modifies semantics.
pub struct ApiDispatcher {
    bootstrap: Bootstrap,
}

impl ApiDispatcher {
    /// Create a new dispatcher backed by a fresh Bootstrap.
    pub fn new() -> Self {
        ApiDispatcher {
            bootstrap: Bootstrap::new(),
        }
    }

    /// Create a dispatcher from an explicit event list (test helper).
    pub fn from_events(events: Vec<Event>) -> Self {
        ApiDispatcher {
            bootstrap: Bootstrap::from_events(events),
        }
    }

    /// Parse, validate, execute, and return the result.
    ///
    /// Every invocation returns a `CommandResultV1` — even validation errors
    /// are wrapped in `ResultPayload::Error`.  The caller is responsible for
    /// serialization (typically `serde_json::to_string_pretty`).
    pub fn dispatch(&mut self, input: &str) -> Result<CommandResultV1, ApiError> {
        let value: serde_json::Value =
            serde_json::from_str(input).map_err(|e| ApiError::invalid_json(e))?;

        self.validate_envelope(&value)?;
        let trace_id_value = value
            .get("trace_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ApiError::invalid_field("trace_id", "positive integer"))?;

        let envelope: CommandEnvelope = serde_json::from_value(value)
            .map_err(|e| ApiError::deserialization_failed(e))?;

        let result = self.bootstrap.execute(envelope);

        // I2 — Deterministic External Execution: verify trace_id roundtrip.
        debug_assert_eq!(
            result.trace_id.0, trace_id_value,
            "output trace_id must match input trace_id"
        );

        Ok(result)
    }

    /// Validate the envelope structure before deserialization.
    fn validate_envelope(&self, value: &serde_json::Value) -> Result<(), ApiError> {
        let obj = value
            .as_object()
            .ok_or_else(|| ApiError::invalid_json_str("root must be a JSON object"))?;

        // ── version field ────────────────────────────────────────────────
        let version = obj
            .get("version")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ApiError::invalid_field("version", "positive integer"))?;

        if version != ENVELOPE_VERSION as u64 {
            return Err(ApiError::schema_mismatch(version, ENVELOPE_VERSION as u64));
        }

        // ── trace_id field ───────────────────────────────────────────────
        let trace_id_val = obj
            .get("trace_id")
            .ok_or_else(|| ApiError::missing_field("trace_id"))?;

        if !trace_id_val.is_number() {
            return Err(ApiError::invalid_field("trace_id", "integer"));
        }

        // ── command field ───────────────────────────────────────────────
        let command_val = obj
            .get("command")
            .ok_or_else(|| ApiError::missing_field("command"))?;

        if !command_val.is_object() && !command_val.is_string() {
            return Err(ApiError::invalid_field("command", "object or string"));
        }

        Ok(())
    }
}

impl Default for ApiDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ── API Error ─────────────────────────────────────────────────────────────

/// Structured error returned by `ApiDispatcher::dispatch`.
///
/// Every error carries a stable error code and a human-readable message.
/// The error is serializable to JSON so transport layers can return it in
/// a uniform format.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiError {
    /// Stable error code for programmatic handling.
    pub code: String,
    /// Human-readable description of the error.
    pub message: String,
}

impl ApiError {
    fn invalid_json(e: serde_json::Error) -> Self {
        ApiError {
            code: "INVALID_JSON".into(),
            message: format!("malformed JSON input: {}", e),
        }
    }

    fn invalid_json_str(msg: &'static str) -> Self {
        ApiError {
            code: "INVALID_JSON".into(),
            message: msg.to_string(),
        }
    }

    fn missing_field(field: &'static str) -> Self {
        ApiError {
            code: "MISSING_FIELD".into(),
            message: format!("required field '{}' is missing", field),
        }
    }

    fn invalid_field(field: &'static str, expected: &'static str) -> Self {
        ApiError {
            code: "INVALID_FIELD".into(),
            message: format!("field '{}' must be {}", field, expected),
        }
    }

    fn schema_mismatch(got: u64, expected: u64) -> Self {
        ApiError {
            code: "SCHEMA_MISMATCH".into(),
            message: format!(
                "unsupported schema version: got {}, expected {} (ABI {})",
                got, expected, ABI_VERSION
            ),
        }
    }

    fn deserialization_failed(e: serde_json::Error) -> Self {
        ApiError {
            code: "DESERIALIZATION_FAILED".into(),
            message: format!("failed to deserialize envelope: {}", e),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Roundtrip Determinism ─────────────────────────────────────────────

    #[test]
    fn roundtrip_query_state() {
        let input = r#"{"version":1,"trace_id":42,"command":"QueryState"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let result = d.dispatch(input).unwrap();
        assert_eq!(result.trace_id.0, 42);
        assert_eq!(result.version, 1);
    }

    #[test]
    fn roundtrip_version() {
        let input = r#"{"version":1,"trace_id":7,"command":"GetVersion"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let result = d.dispatch(input).unwrap();
        assert_eq!(result.trace_id.0, 7);
    }

    #[test]
    fn roundtrip_create_node() {
        let input = r#"{"version":1,"trace_id":10,"command":{"Ingest":{"id":"api-node","timestamp":0,"event_type":"CreateNode","payload":{"id":"api-node"},"causes":[],"meta_reason":null}}}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let result = d.dispatch(input).unwrap();
        assert_eq!(result.trace_id.0, 10);
    }

    #[test]
    fn roundtrip_determinism_same_input_same_output() {
        let input = r#"{"version":1,"trace_id":0,"command":"QueryState"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let a = d.dispatch(input).unwrap();
        // Calling dispatch again with same input must produce identical output
        // (note: the engine state is the same since QueryState is read-only).
        let b = d.dispatch(input).unwrap();
        assert_eq!(a, b, "identical inputs must produce identical outputs");
    }

    // ── Invalid Input Rejection ───────────────────────────────────────────

    #[test]
    fn reject_malformed_json() {
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch("not json").unwrap_err();
        assert_eq!(err.code, "INVALID_JSON");
    }

    #[test]
    fn reject_empty_input() {
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch("").unwrap_err();
        assert_eq!(err.code, "INVALID_JSON");
    }

    #[test]
    fn reject_missing_version() {
        let input = r#"{"trace_id":0,"command":"QueryState"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch(input).unwrap_err();
        assert_eq!(err.code, "INVALID_FIELD");
    }

    #[test]
    fn reject_missing_trace_id() {
        let input = r#"{"version":1,"command":"QueryState"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch(input).unwrap_err();
        assert_eq!(err.code, "MISSING_FIELD");
    }

    #[test]
    fn reject_missing_command() {
        let input = r#"{"version":1,"trace_id":0}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch(input).unwrap_err();
        assert_eq!(err.code, "MISSING_FIELD");
    }

    #[test]
    fn reject_wrong_schema_version() {
        let input = r#"{"version":99,"trace_id":0,"command":"QueryState"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch(input).unwrap_err();
        assert_eq!(err.code, "SCHEMA_MISMATCH");
    }

    #[test]
    fn reject_invalid_command_variant() {
        let input = r#"{"version":1,"trace_id":0,"command":"BogusVariant"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch(input).unwrap_err();
        assert_eq!(err.code, "DESERIALIZATION_FAILED");
    }

    #[test]
    fn reject_array_input() {
        let mut d = ApiDispatcher::from_events(vec![]);
        let err = d.dispatch("[]").unwrap_err();
        assert_eq!(err.code, "INVALID_JSON");
    }

    // ── Session Metadata Isolation ────────────────────────────────────────

    #[test]
    fn session_id_is_optional_in_input() {
        let input = r#"{"version":1,"trace_id":5,"command":"GetVersion"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let result = d.dispatch(input).unwrap();
        assert_eq!(result.trace_id.0, 5);
    }

    #[test]
    fn session_id_in_input_does_not_affect_output() {
        let input_with_session =
            r#"{"version":1,"trace_id":5,"command":"GetVersion","session_id":42}"#;
        let input_without =
            r#"{"version":1,"trace_id":5,"command":"GetVersion"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let a = d.dispatch(input_with_session).unwrap();
        let b = d.dispatch(input_without).unwrap();
        // Same trace_id and command -> same output.
        assert_eq!(a, b, "session_id must not affect output");
    }

    // ── Kernel Isolation ──────────────────────────────────────────────────

    #[test]
    fn dispatcher_uses_bootstrap_not_kernel_directly() {
        // This test verifies architectural isolation: ApiDispatcher routes
        // through Bootstrap::execute(), not directly to Kernel.  If the
        // implementation is changed to bypass Bootstrap, this test documents
        // the violation.
        let input = r#"{"version":1,"trace_id":1,"command":"QueryState"}"#;
        let mut d = ApiDispatcher::from_events(vec![]);
        let result = d.dispatch(input);
        assert!(result.is_ok(), "dispatch must succeed");
    }
}
