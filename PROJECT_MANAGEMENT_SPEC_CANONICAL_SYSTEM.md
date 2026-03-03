# Project Spec: Canonical N* System

Date: 2026-03-03

## 1) What we are building

Build one canonical core with only 5 fixed loop parts:
- ingestion
- memory
- comparison
- update loop
- falsification

Everything else must be learned at runtime:
- operators
- criteria/gates
- domain nouns/concepts
- dimensionality `n`

## 2) Non-negotiable rules

- No decision-critical hardcoded gate schema in canonical path.
- No keyword/string heuristics deciding control flow.
- Promotion only by replay + falsification evidence.
- Every state-changing step must emit receipt-linked evidence.

## 3) Work lanes

### Lane A: Docs and interface truth
- Remove stale commands.
- Keep docs split as:
  - implemented now
  - planned next
- Exit gate:
  - top-level docs only show runnable commands and current behavior.

### Lane B: Deterministic replay
- Add replay verifier for canonical state + receipts.
- Exit gate:
  - same event log gives same decisions and same final state hash on `100/100` runs.

### Lane C: De-hardcode control
- Replace fixed gate action path with learned graph control objects.
- Remove lexical heuristic checks from invariant path.
- Exit gate:
  - static scan + code review find no decision-critical hardcoded control schema in canonical path.

### Lane D: Criteria as mutable state
- Store risk/quality criteria as graph state objects.
- Track criteria changes in receipts (before/after).
- Exit gate:
  - criteria updates replay exactly from receipts.

### Lane E: Falsification and promotion
- Run A/B:
  - baseline: current fixed/heuristic control path
  - candidate: learned-criteria control path
- Metrics:
  - repeated-failure motifs
  - intervention count
  - contradiction trend
- Exit gate:
  - candidate reduces repeated-failure motifs by at least 30% by turn 20
  - no regression on critical tasks

## 4) Execution order

1. Lane A first.
2. Lane B and Lane C in parallel.
3. Lane D after Lane C stabilizes.
4. Lane E after Lane B + Lane D are ready.
5. Promote only if all active gates pass.

## 5) Immediate next actions (high-leverage)

1. Freeze the target lane.
- Declare one path as canonical for this build: `src/canonical/*` + `src/bin/canonical.rs`.
- Block new control logic in other lanes until this spec passes.

2. Create one hard fail gate in CI now.
- Fail CI if canonical path contains decision-critical lexical heuristics (`contains("read")`, `contains("verified")`, etc.).
- This prevents new heuristic drift while we refactor.

3. Lock one deterministic replay contract.
- Define the replay artifact contract now:
  - input log format
  - expected decision trace format
  - final state hash check
- Do not start broader refactors until this contract is fixed.

4. Cut one “control core” refactor slice.
- First slice only:
  - move gate decisions from fixed action enums to graph-stored control objects
  - keep external behavior unchanged
- Ship this slice before touching criteria learning.

5. Start A/B harness scaffolding immediately.
- Create baseline and candidate runners with identical task streams.
- Store outputs in one comparable schema from day one (same metrics keys, same receipt pointers).

6. Define promotion vetoes before any benchmark run.
- Candidate is auto-rejected if:
  - replay contract fails
  - critical task reliability regresses
  - receipt chain is incomplete

## 6) Agentic assumption check (validated against current code)

Assumption A: We already have one canonical execution lane.
- Status: `True`
- Grounding:
  - `src/bin/canonical.rs` runs one turn pipeline (propose -> observe -> discover -> process -> receipt).
  - `src/canonical/core.rs`, `src/canonical/graph.rs`, `src/canonical/invariants.rs` hold that pipeline logic.

Assumption B: Control semantics are fully learned (not hardcoded).
- Status: `False` (partially hardcoded today)
- Grounding:
  - Fixed gate actions are hardcoded (`Halt/Verify/Escalate/Simulate/None`) in `src/canonical/types.rs`.
  - Gate decisions dispatch those fixed actions in `src/canonical/graph.rs`.

Assumption C: Invariant/evidence checks are free of lexical heuristics.
- Status: `False`
- Grounding:
  - Invariant path uses keyword checks like `prompt.contains("read")`, `response.contains("verified")`, and other string checks in `src/canonical/invariants.rs`.

Assumption D: Runtime can discover new dimensions/nodes.
- Status: `True`
- Grounding:
  - `src/bin/canonical.rs` calls `reflect_new_nodes(...)` and feeds discoveries into canonical processing.
  - `CANONICAL_CORE.md` declares dynamic runtime node discovery and graph mutation.

Assumption E: Multi-scale coordinates exist in runtime.
- Status: `True`
- Grounding:
  - `Scale` includes `Token`, `Turn`, `Session`, `Project` in `src/canonical/types.rs`.
  - Canonical run output prints per-scale coordinates in `src/bin/canonical.rs`.

Assumption F: “Start with zero predicates” is the intended model.
- Status: `True` (intent), `Mixed` (runtime still has fixed control schema)
- Grounding:
  - `PROTOCOL.md` says start with zero predicates.
  - Canonical implementation still uses fixed gate action enum and thresholds.

## 7) Build-aid context snippets (for implementers)

Use these snippets as working context while coding.

- Product intent (`THEORY.md`):
  - dynamic in `n`
  - node-state interchangeable
  - activation spreads across topology
  - multi-scale recursion

- Protocol intent (`PROTOCOL.md`):
  - start with zero predicates
  - discover predicates from failures
  - current gate examples are fixed (`halt/verify/escalate > 0.7`)

- Executable reality (`CANONICAL_CORE.md` + `src/canonical/*`):
  - implemented now:
    - graph state (`nodes`, `edges`, `patterns`)
    - activation propagation
    - discovery + gate evaluation
    - simulation before materialization
    - invariant checks
    - append-only receipts
  - still to change for target architecture:
    - fixed gate action enums in decision path
    - lexical heuristics in invariant path
    - fixed decision-critical thresholds/constants

- Primary refactor boundary (to avoid sprawl):
  - edit focus:
    - `src/canonical/types.rs`
    - `src/canonical/graph.rs`
    - `src/canonical/invariants.rs`
    - `src/bin/canonical.rs`
  - keep UTIR execution and receipt plumbing stable unless replay contract requires changes.

## 8) Wider-system source priority (use this to avoid stale reads)

When sources disagree, use this priority order:

1. Math kernel behavior (highest)
- `nstar-autogenesis/engine.py` (actual update equations)
- `nstar-bit/src/collapse.rs` + `src/state.rs` (prime-coordinate collapse + state growth)

2. Executable canonical runtime
- `nstar-bit/src/bin/canonical.rs`
- `nstar-bit/src/canonical/*`

3. Evidence substrate and benchmark harness
- `tmp-meta3-engine-test/meta3-graph-core/src/receipt.rs`
- `tmp-meta3-engine-test/_export/meta3-canonical/showcases/tribench-v2/README.md`
- `tmp-meta3-engine-test/_export/meta3-canonical/graphs/canonical_core.hypergraph.json`

4. Governance and promotion
- `macro-hard/config/requirements.meta_minimax.json`
- `macro-hard/scripts/network_effects.py`
- `agentic-os/policies/core-policy.json`
- `dreaming-kernel/scripts/quality_gate.py`

5. Narrative/history (lowest, context only)
- `raw-chat.md`
- `antigravity_Refining Summary Accuracy.md`

## 9) Context snippets to build from (math + wider system)

### A) Math kernel (what is already explicit)
- In `nstar-autogenesis/engine.py`:
  - hypothesis counts: `c11, c10, c01, c00`
  - PMI utility:
    - `pxy = c11/t`
    - `px = (c11+c10)/t`
    - `py = (c11+c01)/t`
    - `pmi = log((pxy+eps)/(px*py+eps))`
    - `utility = pxy * pmi`
  - posterior for co-occurrence (Beta prior):
    - `alpha = 1 + c11`
    - `beta = 1 + c10 + c01`
    - mean `pm = alpha/(alpha+beta)`
    - variance `pv = (alpha*beta)/((alpha+beta)^2*(alpha+beta+1))`
  - active core selection: utility >= mean utility
  - seed generation: top uncertainty (`pv`) + boundary-near-mean probes

### B) Collapse math in nstar-bit (current implementation)
- In `src/collapse.rs`:
  - coordinate contains:
    - `event_id` (product of active predicate primes)
    - `primes` (active prime factors)
    - `intensity` (average activation)
    - `scale` (currently turn-level in this function)
- In `src/state.rs`:
  - each discovered predicate gets next prime ID
  - merge-candidate signal uses co-activation Jaccard over collapse history

### C) Wider system evidence model
- In `meta3-graph-core/src/receipt.rs`:
  - deterministic receipt structure with `input_sha256` + typed effects
  - effect taxonomy: write/read/http/git/assert/blocked/exec
- In TriBench v2:
  - benchmark chain is explicit:
    - UTIR -> execution -> receipts -> hypergraph/eval artifacts
  - includes negative probes expected to be `blocked` in receipts

### D) Governance math already in wider system
- In `macro-hard/scripts/network_effects.py`:
  - trend gate uses linear regression slope over macro scores
  - pass if:
    - latest score >= `min_score`
    - slope >= `min_slope`
    - critical regressions <= `max_critical_regressions`
- In `macro-hard/config/requirements.meta_minimax.json`:
  - current defaults keep heuristics off:
    - `heuristics.mode = off`
    - `repair_retries = 0`
  - promotion thresholds are explicit (`min_score`, `min_slope`, critical suites)

### E) Practical build implication
- Treat the project as one pipeline, not one file tree:
  - discovery math (`autogenesis`) -> runtime collapse (`nstar-bit`) -> receipts/eval (`meta3`) -> promotion gates (`macro-hard/agentic-os`)
- Build changes are valid only if they preserve this full chain.
