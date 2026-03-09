# Architecture Search Synthesis

**Task:** System-wide search for abstractions that trump nstar-bit. Desktop sweep: dreaming-kernel, Foundational tools, agentic-os, tmp-meta3-engine-test. Deep dive into nested Rust projects.

**Source:** Parent transcript synthesis from multiple subagent runs.

---

## 1. Eval Must Measure the Real Runtime

**dreaming-kernel** already hit the same failure class: the runtime routed one way, but the quality gate measured a different function. Diagnosis in `dreaming-kernel/docs/DIAGNOSTIC_REPORT.md`.

For nstar-bit: Are `heldout_eval`, invariants, scorecards, promotion checks, and predicate evaluation all measuring the *same object* the runtime uses? If not, you optimize mirrors.

---

## 2. Stronger Architecture: 3-Layer, Not 1-Layer

Across dreaming-kernel, core-clarity-dashboard, Foundational tools, agentic-os:

- **Runtime physics layer**
- **Aligned evaluation/gate layer**
- **Decision/portfolio layer**

nstar-bit compresses substrate + evaluator + policy selection into one loop. Stronger systems separate them.

---

## 3. Strongest Abstraction: Policy Selection Under Gate-Linked Evidence

Across macro-hard, agentic-os, dreaming-kernel, core-clarity-dashboard, tmp-meta3-engine-test:

- thin runtime
- explicit evaluator
- separate policy/promotion layer
- machine-readable decision surface

More mature than "one loop discovers nodes and writes a scoring rule."

---

## 4. Meta3-Graph-Core as Deepest Step Above nstar-bit

**Typed hypergraph of the whole control system:**
- capabilities, tasks, policies, receipts, triggers, operations, artifacts

Key files:
- `tmp-meta3-engine-test/meta3-graph-core/src/hypergraph.rs`
- `tmp-meta3-engine-test/meta3-graph-core/src/capability_graph.rs`
- `tmp-meta3-engine-test/meta3-graph-core/src/bin/graph_harness_emit.rs`

---

## 5. Low-Level Primitives Worth Stealing

- **meta3-core-repro/meta3-graph-core** typed hypergraph — best substrate upgrade over nstar-bit’s GraphState
- **one-engine-snapshot** — best typed effect algebra (shell, fs.read, fs.write, assert.*, sequence, parallel, conditional, retry)
- **meta2-engine** — best control-law primitive (L2Params, L3Rules, guarded self-modification)

---

## 6. Closest Whole-Stack Replacement

`tmp-meta3-engine-test` on `feat/mia-injection` + `tribench-v2` artifacts + Foundational tools governor.

Not a single repo. A partially-run stack with:
- thin-ish runtime substrate
- typed graph-native control plane
- guarded execution
- receipts lifted into evidence lane
