# N* Canonical Core

This is the graph-first, simulation-first core implementation for `nstar-bit`.

## What is implemented

- Unified graph state (`nodes`, `edges`, `patterns`) in `src/canonical/types.rs`.
- Activation propagation across graph edges in `src/canonical/graph.rs`.
- Dynamic runtime node discovery (`NodeDiscovery`) with no fixed noun schema.
- Gate evaluation from active nodes and gate patterns.
- Simulation-before-materialization (`simulate_operations`) with risk and ordering checks.
- Invariant checks (`evidence_coverage`, `contradiction_score`, operation/effect consistency).
- Stochastic audits (`audit_rate`) per turn.
- Commit / rollback / halt / escalate decisions.
- Multi-scale coordinates each turn:
  - `Token`
  - `Turn`
  - `Session`
  - `Project`
- Append-only receipt chain (`canonical_receipts.jsonl`) and persisted state (`nstar_canonical_state.json`).

## Entry point

```bash
cargo run --bin canonical -- --interactive
```

Other commands:

```bash
cargo run --bin canonical -- --state
cargo run --bin canonical -- --reset
cargo run --bin canonical -- --max-risk 0.7 --audit-rate 0.5 --interactive
```

## File map

- `src/canonical/core.rs`: canonical turn pipeline
- `src/canonical/graph.rs`: graph mutation + activation propagation + gate logic
- `src/canonical/invariants.rs`: invariant evaluator
- `src/canonical/types.rs`: state/trace/receipt data model
- `src/bin/canonical.rs`: runnable binary using live LM + UTIR
