// ── ABI v0.3 Compatibility Tests ────────────────────────────────────────────
//
// Verifies:
// - ABI version constant correct and stable
// - Version enforcement at all external boundaries
// - Cross-interface consistency under ABI lock
// - Classification table matches runtime exactly

use strata::api::envelope::ENVELOPE_VERSION;
use strata::api::result::CommandResultV1;
use strata::api::ABI_VERSION;
use strata::api::dispatcher::ApiDispatcher;
use strata::bootstrap::Bootstrap;
use strata::cli::CliCommand;

// ── 1. ABI Version Constant ─────────────────────────────────────────────────

#[test]
fn abi_version_is_0_3() {
    assert_eq!(ABI_VERSION, "0.3");
}

#[test]
fn envelope_version_is_1_for_abi_0_3() {
    // ABI v0.3 maps to envelope schema version 1.
    assert_eq!(ENVELOPE_VERSION, 1);
}

#[test]
fn command_result_v1_version_matches_envelope_version() {
    use strata::api::envelope::TraceId;
    use strata::api::command::CommandClass;
    use strata::api::result::ResultPayload;

    let rv1 = CommandResultV1::new(TraceId(0), CommandClass::Query, ResultPayload::Valid);
    assert_eq!(rv1.version, ENVELOPE_VERSION,
        "CommandResultV1.version must equal ENVELOPE_VERSION");
}

// ── 2. Version Enforcement at All External Boundaries ───────────────────────

#[test]
fn api_dispatcher_rejects_missing_version() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"trace_id":0,"command":"QueryState"}"#).unwrap_err();
    assert_eq!(err.code, "INVALID_FIELD",
        "missing version must be rejected with INVALID_FIELD");
}

#[test]
fn api_dispatcher_rejects_wrong_version() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"version":99,"trace_id":0,"command":"QueryState"}"#).unwrap_err();
    assert_eq!(err.code, "SCHEMA_MISMATCH",
        "wrong version must be rejected with SCHEMA_MISMATCH");
}

#[test]
fn api_dispatcher_rejects_version_0() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"version":0,"trace_id":0,"command":"QueryState"}"#).unwrap_err();
    assert_eq!(err.code, "SCHEMA_MISMATCH",
        "version 0 is not a valid ABI v0.3 envelope");
}

#[test]
fn api_dispatcher_rejects_string_version() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"version":"1","trace_id":0,"command":"QueryState"}"#).unwrap_err();
    assert_eq!(err.code, "INVALID_FIELD",
        "string version must be rejected with INVALID_FIELD");
}

#[test]
fn api_dispatcher_rejects_float_version() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"version":1.5,"trace_id":0,"command":"QueryState"}"#).unwrap_err();
    assert_eq!(err.code, "INVALID_FIELD",
        "float version must be rejected with INVALID_FIELD");
}

// ── 3. Envelope Validation Under ABI Lock ───────────────────────────────────

#[test]
fn api_dispatcher_accepts_correct_abi_version() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let result = d.dispatch(r#"{"version":1,"trace_id":42,"command":"QueryState"}"#).unwrap();
    assert_eq!(result.version, 1,
        "accepted envelope must produce CommandResultV1 with version 1");
    assert_eq!(result.trace_id.0, 42,
        "trace_id must be preserved through dispatch");
}

#[test]
fn api_dispatcher_rejects_missing_trace_id() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"version":1,"command":"QueryState"}"#).unwrap_err();
    assert_eq!(err.code, "MISSING_FIELD",
        "missing trace_id must be rejected");
}

#[test]
fn api_dispatcher_rejects_missing_command() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"version":1,"trace_id":0}"#).unwrap_err();
    assert_eq!(err.code, "MISSING_FIELD",
        "missing command must be rejected");
}

// ── 4. Cross-Interface Consistency Under ABI Lock ───────────────────────────
//
// All three interfaces (API, batch CLI, REPL) must produce identical
// ResultPayload for the same command under ABI v0.3.

#[test]
fn batch_and_api_produce_identical_result_payload() {
    // Batch mode.
    let mut bs = Bootstrap::from_events(vec![]);
    let batch = bs.run(CliCommand::Version);

    // API mode.
    let mut d = ApiDispatcher::from_events(vec![]);
    let api = d.dispatch(r#"{"version":1,"trace_id":0,"command":"GetVersion"}"#).unwrap();

    assert_eq!(batch.result, api.result,
        "batch and API must produce identical ResultPayload for Version/GetVersion");
    // class must also match.
    assert_eq!(batch.class, api.class,
        "batch and API must produce identical CommandClass");
}

#[test]
fn batch_list_commands_equals_api_list_commands() {
    let mut bs = Bootstrap::from_events(vec![]);
    let batch = bs.run(CliCommand::ListCommands);

    let mut d = ApiDispatcher::from_events(vec![]);
    let api = d.dispatch(r#"{"version":1,"trace_id":0,"command":"ListCommands"}"#).unwrap();

    assert_eq!(batch.result, api.result,
        "batch and API must produce identical ResultPayload for ListCommands");
}

// ── 5. Serialization Consistency Under ABI Lock ─────────────────────────────

#[test]
fn command_result_v1_serialization_with_abi_version() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let result = d.dispatch(r#"{"version":1,"trace_id":7,"command":"QueryState"}"#).unwrap();

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"version\":1"),
        "serialized output must contain version:1 for ABI v0.3");
    assert!(json.contains("\"trace_id\":7"),
        "serialized output must preserve trace_id");
}

#[test]
fn command_result_v1_serialization_has_no_internal_types() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let result = d.dispatch(r#"{"version":1,"trace_id":1,"command":"QueryState"}"#).unwrap();

    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("HashMap"), "no HashMap leak in output");
    assert!(!json.contains("BTreeMap"), "no BTreeMap leak in output");
    assert!(!json.contains("Kernel"), "no Kernel leak in output");
}

// ── 6. ABI Error Response Consistency ───────────────────────────────────────

#[test]
fn schema_mismatch_error_includes_abi_version_reference() {
    let mut d = ApiDispatcher::from_events(vec![]);
    let err = d.dispatch(r#"{"version":9,"trace_id":0,"command":"QueryState"}"#).unwrap_err();

    // The error message must reference the current ABI version.
    assert!(
        err.message.contains(ABI_VERSION),
        "SCHEMA_MISMATCH error must reference '{}', got: {}",
        ABI_VERSION, err.message
    );
}

// ── 7. Bootstrap Pipeline Consistency ───────────────────────────────────────
//
// Verify that Bootstrap::run and Bootstrap::execute produce identical
// CommandResultV1 structure under ABI v0.3 (modulo trace_id).

#[test]
fn bootstrap_execute_and_run_have_same_structure() {
    use strata::api::envelope::CommandEnvelope;
    use strata::api::command::Command;

    let mut bs = Bootstrap::from_events(vec![]);

    // Via run() — generates its own envelope.
    let run_result = bs.run(CliCommand::ShowState);

    // Via execute() with manual envelope.
    let env = CommandEnvelope::with_id(strata::api::envelope::TraceId(99), Command::QueryState);
    let exec_result = bs.execute(env);

    // Both produce CommandResultV1 with the same structure.
    assert_eq!(run_result.version, exec_result.version,
        "version must match across execution paths");
    assert_eq!(run_result.class, exec_result.class,
        "class must match across execution paths");
    assert_eq!(run_result.result, exec_result.result,
        "result must match across execution paths");
}
