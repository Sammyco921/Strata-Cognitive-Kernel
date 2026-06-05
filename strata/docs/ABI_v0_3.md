# Strata ABI Specification — v0.3

> **Status:** Frozen  
> **ABI Version:** `"0.3"`  
> **Envelope Schema Version:** `1` (wire format)  
> **This document is the single authoritative reference for all external contracts.**

---

## 1. ABI Versioning

The system exposes a single ABI version constant:

```
ABI_VERSION = "0.3"
```

All external interfaces enforce this version. Any change to the contracts
listed below requires an ABI version bump.

### Version Mapping

| ABI Version | Envelope `version` | Status   |
|-------------|---------------------|----------|
| `0.3`       | `1`                 | Current  |

### Freeze Statement

The following are **immutable for v0.3**:

- Command classification (`CommandClass` enum)
- `ResultPayload` variants
- `ErrorCode` enum values
- Bootstrap execution order
- `CommandResultV1` output schema
- `CommandEnvelope` input schema

Any change to the above requires a new ABI version (`0.4` or later).

---

## 2. Input ABI — `CommandEnvelope`

### 2.1 Schema

```json
{
  "version": 1,
  "trace_id": 0,
  "command": "<CommandVariant>",
  "session_id": null
}
```

### 2.2 Fields

| Field        | Type          | Required | Description |
|-------------|---------------|----------|-------------|
| `version`    | `u32`         | Yes      | Envelope schema version. MUST be `1` for ABI v0.3. |
| `trace_id`   | `u64`         | Yes      | Deterministic monotonically increasing trace identifier. The caller supplies this value; the system uses it as-is (never generates). |
| `command`    | `Command`     | Yes      | The command variant (string or object, see §2.3). |
| `session_id` | `Option<u64>` | No       | Purely informational REPL session grouping. **NON-ABI affecting**: MUST NOT influence execution, output, or determinism. |

### 2.3 Command Variants

Commands are serialized as either:
- A bare string for parameterless variants (e.g., `"QueryState"`, `"GetVersion"`)
- An object with a single key for parameterized variants (e.g., `{"Ingest": {...}}`)

The full command enumeration is defined in `src/api/command.rs`.

### 2.4 TraceId Semantics

- `TraceId` is a monotonically increasing `u64`.
- In batch CLI mode, the system generates `TraceId(0)` for the single command.
- In API mode, the caller provides the `trace_id` in the input envelope.
- The system MUST preserve the `trace_id` through to `CommandResultV1`.
- `TraceId` values are NOT guaranteed globally unique — only unique within a single process session.

### 2.5 SessionId Semantics

- `session_id` is purely informational metadata for REPL session grouping.
- **It MUST NOT affect execution, determinism, or output.**
- It is `#[serde(default)]` and `#[serde(skip_serializing_if = "Option::is_none")]`.
- If omitted, the envelope is identical to one without a session (for determinism).
- Batch mode always sets `session_id: None`.
- API mode may optionally include it; it is stripped before execution.

---

## 3. Execution ABI — Bootstrap Pipeline

### 3.1 Pipeline Ordering

```
CommandEnvelope
  → Bootstrap::execute()
    → Command::class()       (classification)
    → Bootstrap::dispatch()  (system or executor routing)
      → CommandExecutor      (engine-bound commands)
      → System handler       (metadata/diagnostic commands)
    → CommandResultV1        (output contract)
```

The pipeline order is **immutable for v0.3**.

### 3.2 Classification Rules

Every `Command` variant belongs to exactly one `CommandClass`:

| Command Variant                  | Class       | Semantics |
|----------------------------------|-------------|-----------|
| `Ingest`                         | Execution   | Mutates the event log |
| `Validate`                       | Query       | Dry-run, no mutation |
| `Replay`                         | Query       | Read-only replay |
| `QueryState`                     | Query       | Read current state |
| `Explain`                        | Query       | Trace explanation |
| `CausalChain`                    | Query       | Trace predecessors |
| `ExportSnapshot`                 | Query       | Export state |
| `GetNode`                        | Query       | Single node lookup |
| `GetEdge`                        | Query       | Single edge lookup |
| `ListNodes`                      | Query       | All nodes |
| `ListEdges`                      | Query       | All edges |
| `EventById`                      | Query       | Event lookup |
| `EventsForNode`                  | Query       | Node events |
| `EventsBetween`                  | Query       | Time-range events |
| `LatestEvents`                   | Query       | Recent events |
| `SnapshotMetadata`               | Query       | Snapshot info |
| `GetVersion`                     | System      | Kernel version |
| `GetSchemaVersion`               | System      | Schema version |
| `ValidateLog`                    | System      | Log integrity |
| `ReplayCheck`                    | System      | Replay comparison |
| `WorkflowList`                   | System      | Workflow list |
| `WorkflowRun`                    | System      | Workflow execution |
| `WorkflowValidate`               | System      | All workflows |
| `ListCommands`                   | System      | Command catalog |
| `Describe`                       | System      | Command details |

### 3.3 Deterministic Constraints

- `Command::class()` is a pure function of the variant — always returns the
  same class for the same variant.
- Bootstrap execution is deterministic given the same event history and the
  same input command.
- No system state influences command classification.
- No timing, logging, or ordering side effects influence output.

---

## 4. Output ABI — `CommandResultV1`

### 4.1 Schema

```json
{
  "version": 1,
  "trace_id": 0,
  "class": "Query",
  "result": <ResultPayload>
}
```

### 4.2 Fields

| Field      | Type            | Required | Description |
|-----------|-----------------|----------|-------------|
| `version`  | `u32`           | Yes      | Envelope schema version (always `1` for ABI v0.3). |
| `trace_id` | `TraceId` (`u64`) | Yes    | Matches the originating `CommandEnvelope.trace_id`. |
| `class`    | `CommandClass`  | Yes      | The classification of the executed command. |
| `result`   | `ResultPayload` | Yes      | The command output (see §4.3). |

### 4.3 ResultPayload Variants (Fully Enumerated)

All 21 variants are listed below. Each maps 1:1 to a command output type.

| # | Variant Name        | Inner Type              | Description |
|---|---------------------|-------------------------|-------------|
| 1 | `Ingested`          | `IngestResult`          | Event ingestion succeeded |
| 2 | `Valid`             | (unit)                  | Validation passed |
| 3 | `StateView`         | `StateView`             | Full graph state (nodes + edges) |
| 4 | `NodeView`          | `NodeView`              | Single node lookup |
| 5 | `EdgeView`          | `EdgeView`              | Single edge lookup |
| 6 | `Nodes`             | `Vec<NodeView>`         | All nodes |
| 7 | `Edges`             | `Vec<EdgeView>`         | All edges |
| 8 | `EventView`         | `EventView`             | Single event |
| 9 | `Events`            | `Vec<EventView>`        | Event collection |
| 10 | `ExplanationView`  | `ExplanationView`       | Causal explanation |
| 11 | `CausalChainView`   | `Vec<CausalChainLink>`  | Causal chain trace |
| 12 | `SnapshotExport`    | `String`                | Snapshot JSON |
| 13 | `SnapshotMetadata`  | `SnapshotView`          | Snapshot metadata |
| 14 | `Version`           | `String`                | Kernel version |
| 15 | `SchemaVersion`     | `String`                | Event schema version |
| 16 | `ValidateLog`       | `LogValidationResult`   | Log validation |
| 17 | `ReplayCheck`       | `ReplayCheckResult`     | Replay comparison |
| 18 | `WorkflowList`      | `WorkflowListResult`    | Available workflows |
| 19 | `WorkflowRun`       | `WorkflowRunResult`     | Workflow result |
| 20 | `WorkflowValidate`  | `WorkflowValidateResult`| All-workflows result |
| 21 | `CommandList`       | `CommandListResult`     | Command catalog |
| 22 | `Describe`          | `DescribeResult`        | Command description |
| 23 | `Error`             | `CommandError`          | Structured error |

### 4.4 ErrorCode Stability Rules

`ErrorCode` values are **immutable for v0.3**:

| Code                     | Meaning |
|--------------------------|---------|
| `ValidationError`        | Event failed semantic validation |
| `CausalCycleViolation`   | Causal cycle detected at commit |
| `NotFound`               | Referenced entity not found |
| `UnsupportedOperation`   | Operation not supported |
| `IoError`                | I/O failure |
| `InternalError`          | Internal invariant violation |
| `Unknown`                | Unclassified failure |

- Variants may be added but never removed.
- Variant names (as serialized strings) must never change.
- Each variant's semantics must never change.

---

## 5. Transport ABI

### 5.1 API Dispatcher Contract

**Interface:** `ApiDispatcher::dispatch(&mut self, input: &str) -> Result<CommandResultV1, ApiError>`

**Input:** JSON-serialized `CommandEnvelope`.  
**Output:** JSON-serializable `CommandResultV1` on success, `ApiError` on validation failure.  
**Determinism:** Identical input → identical output across runs, sessions, and interfaces (for read-only commands).

Validation sequence:
1. Input is valid JSON
2. Input is a JSON object
3. `version` field present and equals `1`
4. `trace_id` field present and is an integer
5. `command` field present and is a string or object
6. Deserialize to `CommandEnvelope`
7. Execute via `Bootstrap::execute()`

### 5.2 CLI JSON Mode Contract

**CLI:** `strata api --input '<json>'`

**Behavior:** Equivalent to calling `ApiDispatcher::dispatch(input)` in the
binary. Output is printed to stdout as formatted JSON.

### 5.3 REPL Equivalence Constraints

- Batch mode (`Bootstrap::run`) and API mode (`ApiDispatcher::dispatch`) with
  the same command MUST produce the same `ResultPayload`.
- REPL mode wraps each input line in a `CommandEnvelope` with a session_id;
  the `ResultPayload` is identical to batch/API for the same command.
- The only difference between interfaces is:
  - `trace_id` source (batch: generated; API: caller-provided; REPL: generated)
  - `session_id` presence (batch: None; REPL: Some(n); API: caller-option)
- `ResultPayload` is identical across all three interfaces.

---

## 6. Invariants

### I1 — ABI is the only versioned contract surface

No subsystem may introduce its own versioning scheme. All versioning is
governed by `ABI_VERSION` and this document.

### I2 — Kernel is ABI-agnostic internally but ABI-consistent externally

The kernel (`Kernel`, `CausalGraph`, etc.) does not enforce versioning.
All version enforcement is in the API layer (`ApiDispatcher`, `CommandEnvelope`,
`CommandResultV1`).

### I3 — No semantic drift without ABI bump

Any change to:
- Output schema (`CommandResultV1`, `ResultPayload`)
- Command classification (`Command::class()`)
- Execution order (`Bootstrap::execute()` pipeline)
- Error codes (`ErrorCode`)

requires an ABI version increment.

### I4 — Determinism preserved under ABI enforcement

ABI validation must not affect execution outcome, only acceptance.
An accepted request must produce the same result with or without ABI
validation.

---

## 7. Appendix: Cross-Reference

| Concept             | Source Location               |
|---------------------|-------------------------------|
| ABI_VERSION         | `src/api/mod.rs`              |
| ENVELOPE_VERSION    | `src/api/envelope.rs`         |
| CommandEnvelope     | `src/api/envelope.rs`         |
| Command             | `src/api/command.rs`          |
| CommandClass        | `src/api/command.rs`          |
| CommandResultV1     | `src/api/result.rs`           |
| ResultPayload       | `src/api/result.rs`           |
| ErrorCode           | `src/api/result.rs`           |
| ApiDispatcher       | `src/api/dispatcher.rs`       |
| Bootstrap           | `src/bootstrap.rs`            |

---

*End of ABI v0.3 Specification*
