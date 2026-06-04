# Kernel Invariants

This document enumerates every invariant enforced by the Strata kernel.
Each invariant is labelled **I1**–**I10** and includes a rationale, the
enforcing component, and (where applicable) the proving test.

---

## I1 — Event Authority

**The event log is the sole source of truth for kernel state.**

- Every mutation passes through `commit()`, which appends to the log and
  then applies to in-memory state.
- Snapshot restores are verified by re-applying post-snapshot events.
- The log can be replayed from scratch via `replay()` to produce an
  authoritative `GraphState`.

**Enforced by:** `Kernel::commit()`, `Kernel::new_with_events()`

---

## I2 — Replay Determinism

**`replay(events)` always produces the same `GraphState` for the same
ordered event sequence.**

- `replay()` is a pure function with no side effects.
- It iterates events in order, applying each via `apply_event()`.
- `GraphState` uses `BTreeMap` exclusively, guaranteeing deterministic
  iteration order.

**Enforced by:** `replay()` in `kernel/replay.rs`
**Proved by:** `p1_replay_equivalence_*`, `hash_equiv_*` verification tests

---

## I3 — Validator Purity

**`propose()` is a pure function — it inspects state without mutation.**

- `propose()` takes `&self` and returns `Result<(), KernelError>`.
- It never modifies `self.state`, `self.prior_events`, or any other
  kernel field.
- It is called before any write in `commit()`.

**Enforced by:** Rust's borrow checker (`&self` signature)

---

## I4 — G₀/G₁ Separation

**Cycle detection operates exclusively on G₀ (explicit causal edges)
and never reads G₁ (derived causal inferences).**

- `extract_g0_causal_edges()` reads only `Event.causes`.
- `detect_causal_cycle()` builds its adjacency graph from G₀ edges only.
- G₁ is computed by the projection layer and is invisible to the cycle
  detector.

**Enforced by:** `detect_causal_cycle()` in `kernel/replay.rs`
**Proved by:** `tb83`, `tb84`, `tb85` unit tests (projection ignorance,
G₀ exclusivity, causal minimality)

---

## I5 — G₀-Bounded Cycle Validation

**A commit is rejected iff adding the candidate event to G₀ creates a
directed cycle.**

- Detection uses DFS with 3-state tracking (unvisited/in-stack/visited).
- Only G₀ edges participate (see I4).
- A cycle failure returns `KernelError::CausalCycleViolation` with the
  event ID and the cycle path.

**Enforced by:** `detect_causal_cycle()` in `kernel/replay.rs`
**Proved by:** `tb81`–`tb88` integration tests, `p4_cycle_rejection_*`
verification tests

---

## I6 — Atomic Commit

**A commit either fully succeeds or produces no visible state change.**

Commit order:

```
1. propose()              — validate (pure, no mutation)
2. detect_causal_cycle()  — check cycles (pure, no mutation)
3. assign_timestamp()     — advance monotonic clock
4. persistence::append_event() — write to disk
5. apply_event()          — update in-memory state
6. self.prior_events.push() — append to history
```

- Steps 1–2 must pass before any write occurs.
- If step 4 fails, the error propagates and steps 5–6 are skipped.
- Step 3 (clock advance) is an exception — a skipped timestamp is
  harmless because the clock guarantees monotonicity, not consecutiveness.
- The kernel never applies partial state.

**Enforced by:** `Kernel::commit()` in `kernel/engine.rs`

---

## I7 — Monotonic Clock

**Kernel-assigned timestamps are strictly monotonically increasing.**

- `assign_timestamp()` increments `self.clock` by 1 on each call.
- Timestamps are assigned only after validation passes (I6).
- On restart, the clock is initialised from the last event in the log
  (`events.last().map(|e| e.timestamp)`).

**Enforced by:** `Kernel::assign_timestamp()` in `kernel/engine.rs`

---

## I8 — Command Equivalence

**The CLI is a stateless transport layer — every invocation is
independent and produces the same result for the same input.**

- The CLI parses arguments into `CliCommand`, converts to `Command`,
  wraps in `CommandEnvelope`, and calls `Bootstrap::execute()`.
- No session state, no hidden caches, no interactive loops.
- Commands are classified as `Execution`, `Query`, or `System` with
  separate dispatch paths.

**Enforced by:** `main.rs`, `Bootstrap::execute()`, `Command::class()`
**Proved by:** single-entry bootstrap tests

---

## I9 — State Hash Equivalence

**For any event sequence: `state_hash(&replay(events)) == state_hash(&live_state)`.**

- `state_hash()` produces a SHA-256 digest of canonical JSON from
  `GraphState` (BTreeMap ensures sorted keys).
- `replay(events)` reconstructs state purely from the event log.
- Live state is produced by `Kernel::commit()` on the same events.
- The two must match for every valid event sequence.

**Enforced by:** `state_hash()` in `kernel/hash.rs`
**Proved by:** `hash_equiv_*` verification tests

---

## I10 — Serialization Determinism

**All serialized output is byte-for-byte reproducible from the same
`CommandResult`.**

- All maps in serializable types are `BTreeMap` — no `HashMap` anywhere.
- Output goes through `serde_json::to_string_pretty` of `ResultEnvelope`.
- No `{:?}` (Debug) formatting in user-facing output (uses `Display`).
- All `println!` in kernel code is routed to `eprintln!` (stderr),
  keeping stdout exclusively for envelope JSON.

**Enforced by:** project-wide `BTreeMap` usage, `format_output()` in
`command.rs`, envelope serialization in `main.rs`
