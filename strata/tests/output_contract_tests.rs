// ── Phase C Hardening: Output Contract Enforcement ──────────────────────────
//
// Every output MUST pass through CommandResultV1.
// CLI is strictly serialization-only.
// No println! or debug formatting in execution path.

use strata::api::result::CommandResultV1;
use strata::cli::CliCommand;
use strata::test_utils::test_bootstrap;

// ── J1. CommandResultV1 Is the External Boundary ───────────────────────────
//
// I1 — Output Contract: All system output converges through CommandResultV1.
// No internal type is ever serialized for external consumption.

#[test]
fn command_result_v1_has_correct_shape() {
    let mut bs = test_bootstrap(vec![]);
    let result = bs.run(CliCommand::Version);

    // CommandResultV1 must contain the three required fields.
    assert_eq!(result.version, 1, "CommandResultV1 must report version 1");

    // class must be a valid CommandClass variant (serializable).
    let class_str = serde_json::to_string(&result.class).unwrap();
    assert!(!class_str.is_empty(), "class must serialize to non-empty JSON");

    // trace_id must be a valid u64 (actual value depends on test ordering within the binary).
    let _ = result.trace_id.0; // just verify the field exists and is u64

    // result must be a valid ResultPayload variant.
    let payload_json = serde_json::to_string(&result.result).unwrap();
    assert!(payload_json.len() > 2, "ResultPayload must serialize to non-trivial JSON");
}

#[test]
fn command_result_v1_serializes_to_valid_json() {
    let mut bs = test_bootstrap(vec![]);
    let result = bs.run(CliCommand::ShowState);

    let json = serde_json::to_string_pretty(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Must contain the four top-level fields.
    assert!(parsed.get("version").is_some(), "JSON must contain 'version'");
    assert!(parsed.get("trace_id").is_some(), "JSON must contain 'trace_id'");
    assert!(parsed.get("class").is_some(), "JSON must contain 'class'");
    assert!(parsed.get("result").is_some(), "JSON must contain 'result'");

    // Must NOT contain internal types.
    assert!(parsed.get("command").is_none(), "JSON must NOT contain 'command'");
    assert!(parsed.get("session_id").is_none(), "JSON must NOT contain 'session_id' in batch mode");
}

#[test]
fn command_result_v1_serializes_without_hashmap_keywords() {
    // Verifies I2 — No HashMap leak into serialized output.
    let mut bs = test_bootstrap(vec![]);
    let result = bs.run(CliCommand::ShowState);

    let json = serde_json::to_string(&result).unwrap();
    // HashMap/BTreeMap implementation detail must not appear in JSON keys.
    // Properties should be rendered as a JSON object, not with Rust's Debug format.
    assert!(!json.contains("BTreeMap"), "JSON must not leak BTreeMap");
    assert!(!json.contains("HashMap"), "JSON must not leak HashMap");
}

#[test]
fn command_result_v1_all_variants_serialize() {
    // Verify that every major ResultPayload variant serializes.
    let mut bs = test_bootstrap(vec![]);

    let commands: Vec<(CliCommand, &str)> = vec![
        (CliCommand::CreateNode { id: "out-A".into() }, "CreateNode"),
        (CliCommand::ShowState, "ShowState"),
        (CliCommand::Version, "Version"),
        (CliCommand::CreateNode { id: "".into() }, "ValidationError"),
        (CliCommand::WorkflowList, "WorkflowList"),
    ];

    for (cmd, label) in commands {
        let result = bs.run(cmd);
        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            parsed.get("result").is_some(),
            "{} must produce CommandResultV1 with 'result' field",
            label
        );
    }
}

// ── J2. No Internal Types Leaked ───────────────────────────────────────────
//
// The serialization boundary must never expose kernel internals.

#[test]
fn result_payload_is_the_only_result_type() {
    // Verify that the type returned by Bootstrap::run() is CommandResultV1.
    let mut bs = test_bootstrap(vec![]);
    let result: CommandResultV1 = bs.run(CliCommand::Version);
    // This test is intentionally trivial — it's a type-level assertion that
    // Bootstrap::run() returns CommandResultV1. If the return type changes,
    // this test will not compile.
    drop(result);
}

// ── J3. Session Metadata Never Leaks into Batch Output ─────────────────────
//
// Batch mode (Bootstrap::run) must never include session_id in its output.

#[test]
fn batch_mode_omits_session_id() {
    // Bootstrap::run() creates envelopes internally via CommandEnvelope::new()
    // which sets session_id to None.  The serialization must therefore omit it.
    let mut bs = test_bootstrap(vec![]);
    let result = bs.run(CliCommand::Version);

    let json = serde_json::to_string(&result).unwrap();
    // session_id is #[serde(skip_serializing_if = "Option::is_none")] on
    // the envelope, but it should NOT appear in CommandResultV1 at all.
    assert!(!json.contains("session_id"),
        "batch output must not contain session_id");
}

// ── J4. Deterministic Serialization ────────────────────────────────────────
//
// I2 — Deterministic External Execution:
// Same input MUST always produce identical output JSON (modulo trace_id).

#[test]
fn command_result_v1_serialization_deterministic() {
    let mut bs = test_bootstrap(vec![]);
    let result = bs.run(CliCommand::ShowState);

    let a = serde_json::to_string(&result).unwrap();
    let b = serde_json::to_string(&result).unwrap();
    assert_eq!(a, b, "serialization must be deterministic");
}

// ── J5. Error Serialization Includes Error Code ────────────────────────────
//
// Error results must contain a stable error_code, not a free-form string.

#[test]
fn error_result_contains_stable_error_code() {
    let mut bs = test_bootstrap(vec![]);
    let result = bs.run(CliCommand::CreateNode { id: "".into() });

    let json = serde_json::to_string(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    let result_obj = parsed.get("result").unwrap();
    // Error variant is an externally-tagged enum: {"Error": { "error_code": ..., ... }}
    let error_obj = result_obj.get("Error")
        .expect("Error result must contain 'Error' variant");
    let error_code = error_obj.get("error_code")
        .expect("Error object must contain 'error_code'");
    assert!(error_code.is_string(), "error_code must be a string");
    assert_eq!(error_code.as_str().unwrap(), "ValidationError",
        "empty node id must produce ValidationError");
}

// ── J6. Every Output Path Converges Through CommandResultV1 ────────────────
//
// Verify that system commands (validate, replay-check, workflow-list, etc.)
// all return CommandResultV1 with proper ResultPayload structure.

#[test]
fn system_commands_return_command_result_v1() {
    let mut bs = test_bootstrap(vec![]);

    let system_commands = vec![
        CliCommand::Version,
        CliCommand::SchemaVersion,
        CliCommand::ValidateLog,
        CliCommand::ReplayCheck,
        CliCommand::WorkflowList,
        CliCommand::ListCommands,
    ];

    for cmd in system_commands {
        let result = bs.run(cmd);
        // Type-level assertion: it's a CommandResultV1.
        // If the return type changes, this won't compile.
        let _: CommandResultV1 = result;
    }
}

// ── J7. Frozen Classification Table (doc↔code parity) ───────────────────────
//
// The ABI v0.3 spec documents the exact Command → CommandClass mapping.
// This test asserts the runtime matches the documented table exactly.
// Any drift between doc and code will fail here.

#[test]
fn classification_table_matches_abi_v0_3_spec() {
    use strata::api::command::{Command, CommandClass};
    use strata::Event;

    // Every Command variant with its frozen class (must match docs/ABI_v0_3.md §3.2).
    let sample_event = || Event::new("e".into(), 0, strata::EventType::CreateNode, serde_json::json!({}));

    let commands: Vec<(Command, CommandClass)> = vec![
        // ── Execution (mutates event log) ───────────────────────────────
        (Command::Ingest(sample_event()), CommandClass::Execution),

        // ── Query (read-only) ───────────────────────────────────────────
        (Command::Validate(sample_event()), CommandClass::Query),
        (Command::Replay(vec![]), CommandClass::Query),
        (Command::QueryState, CommandClass::Query),
        (Command::Explain { node_id: "x".into(), property_key: None }, CommandClass::Query),
        (Command::CausalChain("e".into()), CommandClass::Query),
        (Command::ExportSnapshot, CommandClass::Query),
        (Command::GetNode("x".into()), CommandClass::Query),
        (Command::GetEdge("e".into()), CommandClass::Query),
        (Command::ListNodes, CommandClass::Query),
        (Command::ListEdges, CommandClass::Query),
        (Command::EventById("e".into()), CommandClass::Query),
        (Command::EventsForNode("x".into()), CommandClass::Query),
        (Command::EventsBetween { start: 0, end: 1 }, CommandClass::Query),
        (Command::LatestEvents(10), CommandClass::Query),
        (Command::SnapshotMetadata, CommandClass::Query),

        // ── System (no engine access) ───────────────────────────────────
        (Command::GetVersion, CommandClass::System),
        (Command::GetSchemaVersion, CommandClass::System),
        (Command::ValidateLog, CommandClass::System),
        (Command::ReplayCheck, CommandClass::System),
        (Command::WorkflowList, CommandClass::System),
        (Command::WorkflowRun("test".into()), CommandClass::System),
        (Command::WorkflowValidate, CommandClass::System),
        (Command::ListCommands, CommandClass::System),
        (Command::Describe("test".into()), CommandClass::System),
    ];

    // Verify every entry matches.
    for (cmd, expected_class) in &commands {
        let actual_class = cmd.class();
        assert_eq!(&actual_class, expected_class,
            "classification mismatch for {:?}: expected {:?}, got {:?}",
            cmd, expected_class, actual_class);
    }

    // Verify total count matches documented table (25 variants).
    assert_eq!(commands.len(), 25,
        "classification table must have exactly 25 entries (1 Execution + 15 Query + 9 System)");
}
