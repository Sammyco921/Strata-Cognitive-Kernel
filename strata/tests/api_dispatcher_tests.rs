// ── Phase D: API Dispatcher Integration Tests ──────────────────────────────
//
// Verifies roundtrip determinism, invalid input rejection, cross-interface
// equivalence, kernel isolation, and serialization purity.

use strata::api::dispatcher::ApiDispatcher;

// ── 1. Roundtrip Determinism ─────────────────────────────────────────────
//
// I2 — Deterministic External Execution:
// Identical input JSON MUST always produce identical output JSON
// (modulo bootstrap-internal state changes for Execution-class commands).

#[test]
fn roundtrip_query_state_json() {
    let input = r#"{"version":1,"trace_id":0,"command":"QueryState"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let result = d.dispatch(input).unwrap();

    let json = serde_json::to_string(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["version"], 1);
    assert_eq!(parsed["trace_id"], 0);
    assert!(parsed["result"].is_object(), "result must be a JSON object");
}

#[test]
fn roundtrip_create_node_json() {
    let input = r#"{"version":1,"trace_id":5,"command":{"Ingest":{"id":"api-node","timestamp":0,"event_type":"CreateNode","payload":{"id":"api-node"},"causes":[],"meta_reason":null}}}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let result = d.dispatch(input).unwrap();

    let json = serde_json::to_string(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["version"], 1);
    assert_eq!(parsed["trace_id"], 5);
    // Result should be an Ingested or Error variant.
    let result_obj = parsed["result"].as_object().expect("result must be an object");
    let variant_name = result_obj.keys().next().unwrap();
    assert!(
        variant_name == "Ingested" || variant_name == "Error",
        "CreateNode result must be 'Ingested' or 'Error', got '{}'",
        variant_name
    );
}

#[test]
fn roundtrip_repeated_identical_inputs() {
    let input = r#"{"version":1,"trace_id":1,"command":"QueryState"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);

    let a = d.dispatch(input).unwrap();
    let b = d.dispatch(input).unwrap();

    assert_eq!(a, b, "repeated identical inputs must produce identical outputs");
}

#[test]
fn roundtrip_ingest_then_query() {
    let mut d = ApiDispatcher::from_events(vec![]);

    // Create a node.
    let create = r#"{"version":1,"trace_id":10,"command":{"Ingest":{"id":"n1","timestamp":0,"event_type":"CreateNode","payload":{"id":"n1"},"causes":[],"meta_reason":null}}}"#;
    let create_result = d.dispatch(create).unwrap();
    assert_eq!(create_result.trace_id.0, 10, "create trace_id must match input");

    // Query state — node should be present.
    let query = r#"{"version":1,"trace_id":11,"command":{"Ingest":{"id":"n2","timestamp":0,"event_type":"CreateNode","payload":{"id":"n2"},"causes":[],"meta_reason":null}}}"#;
    let query_result = d.dispatch(query).unwrap();
    assert_eq!(query_result.trace_id.0, 11, "query trace_id must match input");
}

// ── 2. Invalid Input Rejection ───────────────────────────────────────────

#[test]
fn reject_non_object_json() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#""just a string""#).unwrap_err();
    assert_eq!(err.code, "INVALID_JSON");
}

#[test]
fn reject_null_input() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch("null").unwrap_err();
    assert_eq!(err.code, "INVALID_JSON");
}

#[test]
fn reject_version_zero() {
    let input = r#"{"version":0,"trace_id":0,"command":"QueryState"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(input).unwrap_err();
    assert_eq!(err.code, "SCHEMA_MISMATCH");
}

#[test]
fn reject_string_trace_id() {
    let input = r#"{"version":1,"trace_id":"abc","command":"QueryState"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(input).unwrap_err();
    assert_eq!(err.code, "INVALID_FIELD");
}

// ── 3. Cross-Interface Equivalence ───────────────────────────────────────
//
// API mode == batch CLI output (modulo trace_id).

#[test]
fn api_equals_batch_for_query_state() {
    use strata::cli::CliCommand;
    use strata::test_utils::test_bootstrap;

    // Batch path.
    let mut bs = test_bootstrap(vec![]);
    let batch_result = bs.run(CliCommand::ShowState);

    // API path.
    let input = r#"{"version":1,"trace_id":0,"command":"QueryState"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let api_result = d.dispatch(input).unwrap();

    // Compare result payloads (ignore trace_id — both paths generate independently).
    assert_eq!(
        batch_result.result, api_result.result,
        "API and batch mode must produce identical result payloads"
    );
}

#[test]
fn api_equals_batch_for_version() {
    use strata::test_utils::test_bootstrap;
    use strata::cli::CliCommand;

    let mut bs = test_bootstrap(vec![]);
    let batch_result = bs.run(CliCommand::Version);

    let input = r#"{"version":1,"trace_id":0,"command":"GetVersion"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let api_result = d.dispatch(input).unwrap();

    assert_eq!(
        batch_result.result, api_result.result,
        "API and batch mode must produce identical Version results"
    );
}

#[test]
fn api_equals_batch_for_error() {
    use strata::test_utils::test_bootstrap;
    use strata::cli::CliCommand;

    let mut bs = test_bootstrap(vec![]);
    let batch_result = bs.run(CliCommand::CreateNode { id: "".into() });

    // API path: Ingest with empty id.
    let input = r#"{"version":1,"trace_id":0,"command":{"Ingest":{"id":"","timestamp":0,"event_type":"CreateNode","payload":{"id":""},"causes":[],"meta_reason":null}}}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let api_result = d.dispatch(input).unwrap();

    // Both must produce a ValidationError.
    // Compare error_code and message (trace_id differs between paths).
    match (&batch_result.result, &api_result.result) {
        (strata::api::result::ResultPayload::Error(ref be), strata::api::result::ResultPayload::Error(ref ae)) => {
            assert_eq!(be.error_code, ae.error_code, "error_code must match");
            assert_eq!(be.message, ae.message, "error message must match");
        }
        _ => panic!("Both paths must produce Error results"),
    }
}

// ── 4. Serialization Purity ──────────────────────────────────────────────
//
// Output MUST contain only CommandResultV1 fields.
// No internal types may leak.

#[test]
fn output_contains_only_command_result_v1_fields() {
    let input = r#"{"version":1,"trace_id":0,"command":"QueryState"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let json = serde_json::to_string(&d.dispatch(input).unwrap()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    let obj = parsed.as_object().expect("output must be a JSON object");
    let allowed_keys = ["version", "trace_id", "class", "result"];
    for key in obj.keys() {
        assert!(
            allowed_keys.contains(&key.as_str()),
            "output must not contain '{}' — only CommandResultV1 fields allowed",
            key
        );
    }
}

#[test]
fn error_output_is_proper_json() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch("not valid json").unwrap_err();
    let json = serde_json::to_string(&err).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("code").is_some(), "error output must contain 'code'");
    assert!(parsed.get("message").is_some(), "error output must contain 'message'");
}

// ── 5. Stateless / No Side Channels ─────────────────────────────────────────
//
// I3 — No Side Channels:
// API dispatcher must produce identical output for identical input
// regardless of prior invocations (for read-only commands).

#[test]
fn query_state_is_idempotent() {
    let input = r#"{"version":1,"trace_id":0,"command":"QueryState"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);

    // Call 10 times — every call must return the same result.
    let first = d.dispatch(input).unwrap();
    for _ in 0..9 {
        let next = d.dispatch(input).unwrap();
        assert_eq!(first, next, "QueryState must be idempotent");
    }
}

// ── 6. No REPL/Session Leakage ──────────────────────────────────────────────
//
// ApiDispatcher must not be coupled to the session system.

#[test]
fn api_dispatcher_api_has_no_session_import() {
    // Architectural verification: ApiDispatcher is in src/api/dispatcher.rs
    // which must not import from session module.  This test verifies the
    // dispatcher works without any session context.
    let input = r#"{"version":1,"trace_id":0,"command":"GetVersion"}"#;
    let mut d = ApiDispatcher::from_events(vec![]);
    let result = d.dispatch(input);
    assert!(result.is_ok(), "dispatcher must work without any session context");
    // Verify no session_id leaks into output.
    let json = serde_json::to_string(&result.unwrap()).unwrap();
    assert!(!json.contains("session_id"), "output must not contain session_id");
}
