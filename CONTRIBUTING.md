# Contributing to N★ Bit

## The One Rule

Every change must pass: `cargo test && cargo clippy`.

No exceptions. If it breaks tests, it does not merge.

---

## Understanding the System in 60 Seconds

N★ Bit does one thing: **evaluate a turn**.

```
Input: (prompt, context)
  │
  ├─ 1. PROPOSE ─── LM generates response + operations
  ├─ 2. OBSERVE ─── LM scores graph nodes against the turn
  ├─ 3. DISCOVER ── LM proposes new nodes if needed
  ├─ 4. SIMULATE ── operations checked for risk + ordering
  ├─ 5. INVARIANT ─ evidence coverage + contradiction score
  ├─ 6. DECIDE ──── commit / rollback / halt / escalate
  └─ 7. RECEIPT ─── hash-chained proof appended to log
  │
Output: (graph state, receipt, decision)
```

That is the entire system. Everything else is plumbing.

### Five files that matter

| File | What it does |
|------|--------------|
| `src/canonical/core.rs` | Runs the 7-step pipeline above |
| `src/canonical/graph.rs` | Mutates graph state (nodes, edges, activations) |
| `src/canonical/invariants.rs` | Checks evidence and contradictions |
| `src/canonical/types.rs` | Every data type in one place |
| `src/bin/canonical.rs` | CLI that wires LM calls to the pipeline |

### Three files that support

| File | What it does |
|------|--------------|
| `src/lm.rs` | Talks to OpenRouter (any OpenAI-compatible API) |
| `src/utir.rs` | Defines operations (Shell, FsRead, FsWrite, etc.) |
| `src/utir_exec.rs` | Executes operations with sandboxing |

### Everything else

`_legacy/` is archived code. `experiments/` has test harnesses. `scripts/` has report generators. None of these affect the core.

---

## Sprint Methodology

### Sprint = 1 Week, 1 Lane

Each sprint targets exactly one lane from the spec. A sprint has three phases:

```
Mon-Tue: BUILD   (write code)
Wed-Thu: VERIFY  (write tests, run experiments)
Fri:     SHIP    (review, merge, update docs)
```

### Definition of Done

A lane is done when ALL of these are true:

1. `cargo test` passes (all existing + new tests)
2. `cargo clippy` has zero warnings
3. The lane's exit gate (from the spec) passes
4. README "Implemented" section is updated
5. README "Planned Next" section is updated

### Current Lane Status

| Lane | Description | Status |
|------|-------------|--------|
| A | Docs + interface truth | Done |
| B | Deterministic replay | Next |
| C | De-hardcode control | Done |
| D | Criteria as mutable state | Done |
| E | Falsification + promotion | Blocked on B + D |

### How to Pick What to Work On

1. Look at the lane table above.
2. Pick the first lane that is not "Done" and not "Blocked".
3. Read its exit gate in `PROJECT_MANAGEMENT_SPEC_CANONICAL_SYSTEM.md`.
4. Write a failing test for that exit gate.
5. Make it pass.

---

## Commit Conventions

```
feat: <what> (new capability)
fix:  <what> (bug fix)
test: <what> (new test)
docs: <what> (documentation only)
chore: <what> (cleanup, deps, refactoring)
```

One logical change per commit. Do not batch unrelated changes.

---

## Architecture Decision Record

When changing how the canonical pipeline works, add a section here:

### ADR-001: No hardcoded gate schema in canonical path (2026-03-03)

Gate decisions use graph-stored control patterns, not fixed enums. The old `GateAction::Halt | Verify | Escalate | Simulate | None` enum was removed in Lane C. Gate signals are now arbitrary strings stored in `GatePattern` nodes.

### ADR-002: Criteria are graph state, not constants (2026-03-03)

Risk thresholds, audit rates, and quality gates are stored as `CanonicalCriteria` in the graph state. Changes to criteria are tracked in receipts (before/after). This was completed in Lane D.

---

## Testing Strategy

### Unit tests: `src/canonical/core.rs`

Test the pipeline in isolation with synthetic inputs. No LM calls.

```bash
cargo test
```

### Experiment harness: `experiments/`

Full integration tests that call the LM and measure predicate discovery, gate behavior, and cross-domain adaptation.

```bash
bash experiments/scripts/run_all.sh
```

### What needs tests (gaps)

- `src/canonical/graph.rs` has zero unit tests
- `src/canonical/invariants.rs` has zero unit tests
- `src/lm.rs` has zero unit tests (hard to test without mocking, see Lane B)
- `src/utir_exec.rs` has zero unit tests

Priority: graph.rs and invariants.rs are pure functions — easy to test, high value.

---

## Environment Setup

```bash
# Rust (stable, 1.75+)
rustup default stable

# OpenSSL dev headers (Linux)
sudo apt-get install -y libssl-dev pkg-config

# API key (required for LM calls)
export OPENROUTER_API_KEY="sk-..."

# Build + test
cargo build && cargo test && cargo clippy
```
