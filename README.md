# N★ Bit

**The causal collapse of things.**

A Rust implementation of the N★ protocol — a graph-first, simulation-first canonical execution core where control semantics are learned, not hardcoded.

---

## Quick Start

```bash
# Build
cargo build --bin canonical

# Run tests
cargo test

# Interactive session
cargo run --bin canonical -- --interactive

# Single prompt
cargo run --bin canonical -- "explain this bounds error"

# Print current graph state
cargo run --bin canonical -- --state

# Reset state and receipts
cargo run --bin canonical -- --reset

# With custom risk and audit thresholds
cargo run --bin canonical -- --max-risk 0.7 --audit-rate 0.5 --interactive
```

Requires: `OPENROUTER_API_KEY` environment variable (or macOS Keychain entry).

---

## How It Works

```
prompt ──► propose ──► simulate ──► invariant check ──► commit/rollback ──► receipt
               │            │              │
               ▼            ▼              ▼
          graph state   risk score    evidence score
          activation    write order   contradiction
          propagation   analysis      detection
```

Each turn:
1. **Propose** — LM generates a response + operations from the prompt.
2. **Observe** — LM evaluates graph nodes against the turn context.
3. **Discover** — LM reflects on whether new nodes are needed.
4. **Simulate** — operations are scored for risk and ordering violations.
5. **Invariants** — evidence coverage and contradictions are checked.
6. **Decide** — commit, rollback, halt, or escalate.
7. **Receipt** — append-only hash-chained receipt is written.

---

## Implemented

- Unified graph state (`nodes`, `edges`, `patterns`) — `src/canonical/types.rs`
- Activation propagation across edges — `src/canonical/graph.rs`
- Dynamic runtime node discovery with no fixed noun schema
- Gate evaluation from active nodes + learned gate patterns (Lane C: complete)
- Risk/quality criteria stored as mutable graph state (Lane D: complete)
- Simulation-before-materialization (risk + operation ordering checks)
- Invariant checks (evidence coverage, contradiction score, effect consistency)
- Stochastic audits (`audit_rate`) per turn
- Commit / Rollback / Halt / Escalate decision path
- Multi-scale coordinates per turn: Token → Turn → Session → Project
- Append-only receipt chain (`canonical_receipts.jsonl`)
- UTIR operation executor with guard config (`src/utir_exec.rs`)
- LM client with OpenRouter + macOS Keychain fallback (`src/lm.rs`)

## Planned Next

- **Lane B**: Deterministic replay verifier — same event log → same state hash, 100/100 runs
- **Lane E**: A/B harness — baseline vs learned-criteria path, trend gate (slope ≥ 0.5 over 20 turns)
- Receipt `version` field + `deterministic: bool` (align with `meta3-graph-core/receipt.rs`)

---

## State Files

| File | Purpose |
|------|---------|
| `nstar_canonical_state.json` | Canonical core graph state |
| `canonical_receipts.jsonl` | Canonical core receipt chain |

Both are `.gitignore`d — they are runtime artifacts, not source.

---

## Source Map

```
src/
  canonical/         ← canonical path (all development happens here)
    core.rs          ← turn pipeline: propose → simulate → invariants → decide → receipt
    graph.rs         ← graph mutation + activation propagation + gate evaluation
    invariants.rs    ← invariant evaluator (evidence, contradiction, effects)
    types.rs         ← all data types (state, trace, receipt, config)
    mod.rs           ← re-exports
  bin/
    canonical.rs     ← CLI binary (interactive + single-prompt modes)
  lm.rs              ← LM client (OpenRouter API, macOS Keychain fallback)
  utir.rs            ← UTIR operation types (Shell, FsRead, FsWrite, etc.)
  utir_exec.rs       ← UTIR executor with sandboxing + guard config
  receipt.rs         ← Effect types + SHA-256 helpers
  lib.rs             ← crate root

_legacy/             ← archived code from pre-canonical pipeline (not compiled)
experiments/         ← experiment harness + prompt sets + output logs
scripts/             ← report generation scripts (Python)
```

---

## Theory

`n★(ins, outs) → collapsed_state`

Takes what went into a computation and what came out. Returns where that computation sits in the space of all possible computations — expressed as an activation pattern over a discovered predicate set, with a prime-coordinate Ruliad address.

Dynamic in `n`. Starts at zero predicates. Discovers its own dimensionality from failures.

See `THEORY.md` for the full formulation.
