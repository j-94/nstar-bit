# N★ Bit

**The causal collapse of things.**

A Rust implementation of the N★ protocol — a graph-first, simulation-first canonical execution core where control semantics are learned, not hardcoded.

---

## Runnable Now

### Canonical Core (the target path)

```bash
# Interactive session — graph-first, simulation-first pipeline
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

### Other Binaries (legacy pipeline)

```bash
# Interactive REPL via legacy NstarState pipeline
cargo run -- --interactive

# Autopoietic loop (self-generates tasks)
cargo run --bin autopoiesis

# Repo grokker (analyze a codebase)
cargo run --bin grok <path_to_repo> [max_files]

# Stress test (8 fixed prompts)
cargo run --bin stress

# Python math kernel (autogenesis)
python3 nstar-autogenesis/engine.py --state nstar-autogenesis/state.json init
python3 nstar-autogenesis/engine.py --state nstar-autogenesis/state.json turn "<message>"
python3 nstar-autogenesis/engine.py --state nstar-autogenesis/state.json show
```

### Tests

```bash
cargo test
```

---

## Implemented Now

- Unified graph state (`nodes`, `edges`, `patterns`) — `src/canonical/types.rs`
- Activation propagation across edges — `src/canonical/graph.rs`
- Dynamic runtime node discovery with no fixed noun schema
- Gate evaluation from active nodes + gate patterns
- Simulation-before-materialization (risk + operation ordering checks)
- Invariant checks (evidence coverage, contradiction score, effect consistency)
- Stochastic audits (`audit_rate`) per turn
- Commit / Rollback / Halt / Escalate decision path
- Multi-scale coordinates per turn: Token → Turn → Session → Project
- Append-only receipt chain (`canonical_receipts.jsonl`)
- UTIR operation executor with guard config (`src/utir_exec.rs`)
- LM client with OpenRouter + macOS Keychain fallback (`src/lm.rs`)

---

## Planned Next (not yet implemented)

- **Lane B**: Deterministic replay verifier — same event log → same state hash, 100/100 runs
- **Lane C**: Replace fixed `GateAction` enum + lexical heuristics with graph-stored control objects
- **Lane D**: Risk/quality criteria stored as mutable graph state, tracked in receipts
- **Lane E**: A/B harness — baseline vs learned-criteria path, trend gate (slope ≥ 0.5 over 20 turns)
- Receipt `version` field + `deterministic: bool` (align with `meta3-graph-core/receipt.rs`)

---

## State Files

| File | Purpose |
|------|---------|
| `nstar_canonical_state.json` | Canonical core graph state |
| `canonical_receipts.jsonl` | Canonical core receipt chain |

---

## Source Map

```
src/
  canonical/         ← THE canonical path (development target)
    core.rs          ← turn pipeline
    graph.rs         ← graph mutation + activation + gates
    invariants.rs    ← invariant evaluator
    types.rs         ← all data types
  bin/
    canonical.rs     ← runnable binary (live LM + UTIR)
    autopoiesis.rs   ← self-generating task loop
    grok.rs          ← codebase analyzer
    stress.rs        ← 8-prompt stress test
  lm.rs              ← LM client (shared)
  utir.rs            ← UTIR operation types (shared)
  utir_exec.rs       ← UTIR executor (shared)
  main.rs            ← legacy CLI binary
  state.rs           ← legacy NstarState
  collapse.rs        ← legacy collapse math
  turn.rs / gate.rs / predicate.rs / receipt.rs  ← legacy pipeline
nstar-autogenesis/
  engine.py          ← Python math kernel (highest priority source)
```

---

## Theory

`n★(ins, outs) → collapsed_state`

Takes what went into a computation and what came out. Returns where that computation sits in the space of all possible computations — expressed as an activation pattern over a discovered predicate set, with a prime-coordinate Ruliad address.

Dynamic in `n`. Starts at zero predicates. Discovers its own dimensionality from failures.

See `THEORY.md` for the full formulation.
