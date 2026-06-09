# Strata Kernel v1.0 — Release Notes

## System Description

Strata Kernel is a **fully deterministic, event-sourced graph kernel** designed for
reproducible, auditable, and verifiable knowledge operations. It processes natural
language input through a layered cognition pipeline, producing identical output for
identical input across any number of runs.

## Layer Architecture

The cognition pipeline consists of 12 stages executed in order:

1. **Semantic Interpreter** — Parse natural language input into structured intent
2. **Event Translator** — Translate intent into proposed graph events
3. **Goal Evaluator** — Evaluate goal predicates against current graph state
4. **Executive** — Score, rank, and select active goals for prioritization
5. **Policy Scorer** — Apply memory-weighted policy rules to candidate intents
6. **Execution Planner** — Build a deterministic execution plan from approved events
7. **Executor** — Execute kernel commands against the graph
8. **Tracer** — Record a full deterministic trace of the execution
9. **Coherence Checker** — Validate execution coherence and consistency
10. **Temporal Analyzer** — Analyze historical trace windows for drift/violations
11. **Memory Updater** — Update cognitive memory from execution results
12. **Memory Snapshot** — Capture final memory state

Each stage is pure, deterministic, and independently verifiable.

## System Invariants

- Deterministic replay: replaying the same event log always produces identical state
- Event-time consistency: all events carry monotonic logical sequence numbers
- State boundary separation: event state, config state, and derived state are isolated
- Frozen ABI registry: versioned (v2), read-only after initialization
- Frozen capability registry: declarative capability map, immutable after init
- Pipeline determinism: identical input always produces identical output

## What Is NOT Included

- No agent, LLM, or large-language-model integration
- No learning, planning, or autonomous decision-making
- No randomness, heuristics, or non-deterministic scoring
- No async, concurrent, or background processing
- No network, IPC, or external service dependencies
- No mutable global state beyond controlled CLI logging config
- No system clock or wall-clock time in pipeline logic

## Reproducibility Guarantee

The Strata Kernel guarantees that for any given input, the entire pipeline produces
byte-identical output across any number of runs. This is enforced by:

- Pure functions with no side effects
- `BTreeMap`-based deterministic ordering
- Manual `Ord` implementations for all f64-containing structs
- Event log replay equivalence validation
- 100-run stability tests throughout the codebase

## Build Instructions

### Minimal build (< 1 MB binary, < 55 MB total, no persistent artifacts)

```sh
# Build release binary (stripped, optimized, no debug symbols)
scripts/build-release.sh

# Run full test suite in isolated temp dir (auto-cleaned)
scripts/test-release.sh
```

### Manual equivalent

```sh
# Clean build with isolated target directory
TARGET=$(mktemp -d)
trap 'rm -rf "$TARGET"' EXIT
CARGO_TARGET_DIR="$TARGET" cargo build --release

# Binary is at $TARGET/release/strata (~718 KB)
# Full target dir (build cache) is ~53 MB, cleaned on exit
```

### Verify zero warnings

```sh
cargo build --release 2>&1 | grep -c "warning:" && echo "Warnings present" || echo "Clean build"
```

### Run CLI

```sh
# Executable is at target/release/strata
cargo run --release -- run "describe the graph"
cargo run --release -- --trace run "query all nodes"
cargo run --release -- replay <trace-file>
cargo run --release -- verify
```

## Profile Configuration

```toml
[profile.release]
opt-level = "z"    # optimize for size
lto = true          # link-time optimization
codegen-units = 1   # single codegen unit for max optimization
strip = true        # strip all symbols
debug = false       # no debug info

[profile.dev]
strip = "debuginfo" # strip debug from dev builds
debug = false        # no debug info

[profile.test]
strip = "debuginfo" # strip debug from test builds
debug = false        # no debug info
```

## Test Count Summary

```
lib tests:        785
strata tests:      45
verification:      16
e2e tests:          7
closure tests:     15
------------------
Total:            ~868+ passing
```

## v1.0 Release Tag

`v1.0-strata-kernel`
