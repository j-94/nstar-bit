# Architecture Evidence Report v3

Date: 2026-03-03
Workspace: `/Users/jobs`
Repro command: `python3 scripts/generate_architecture_report_v3.py`

## 1) Scorecard (/100)

- Evidence rigor (25): `23.5`
- Parallel map (20): `15.86`
- Contradiction analysis (15): `12.8`
- Alignment to constraints (20): `14.8`
- Actionability (20): `15.2`
- **Total**: `82.16`

## 2) Evidence Scope

- Source docs loaded: `21/21`
- Source tiers weighted by reliability:
  - `core` = `5`
  - `exec` = `5`
  - `canonical` = `4`
  - `governance` = `3`
  - `lineage` = `2`
  - `history` = `1`
- Missing sources: `0`

## 3) Semantic Claim Extraction (Structured)

Claims are normalized into a fixed claim ontology and stored in `report_v3_evidence_index.json`.

| Category | Docs hit | Weighted hits |
|---|---:|---:|
| `deterministic_replay` | 14 | 281.0 |
| `graph_executable` | 14 | 236.0 |
| `eval_falsification` | 8 | 148.0 |
| `emergent_core` | 7 | 123.0 |
| `wrapper_surface` | 8 | 63.0 |
| `fixed_gate_rules` | 4 | 44.0 |
| `heuristic_controls` | 4 | 40.0 |
| `fixed_bits_schema` | 3 | 24.0 |

## 4) Quantified Parallel Map

| Rank | Architecture | Score |
|---|---|---:|
| 1 | `P1_canonical_replay_kernel` | 803.2 |
| 2 | `P2_graph_only_executor` | 550.9 |
| 3 | `P4_wrapper_orchestration` | 199.6 |
| 4 | `P3_heuristic_harness` | 152.4 |
| 5 | `P5_fixed_bits_schema` | 54.0 |

Winner: `P1_canonical_replay_kernel` with margin `252.3` over runner-up.

## 5) Contradiction Backlog (Ranked)

| Rank | Contradiction | Severity | Blast Radius | Resolution Cost |
|---|---|---:|---:|---|
| 1 | `X3_graph_core_vs_wrapper_surface` | 13.9 | 0.714 | high |
| 2 | `X2_system_decides_vs_fixed_gate_rules` | 11.5 | 0.381 | medium |
| 3 | `X1_no_heuristics_vs_heuristic_controls` | 10.7 | 0.381 | medium |

### Contradiction Tests

1. `X1_no_heuristics_vs_heuristic_controls`
   - Test: run A/B with heuristics disabled vs enabled on identical trajectory set.
   - Pass: no-heuristics lane reduces repeated failures >=30% by turn 20.
2. `X2_system_decides_vs_fixed_gate_rules`
   - Test: replace static gate thresholds with learned criteria nodes only.
   - Pass: same-or-better risk interception with no fixed numeric gate constants.
3. `X3_graph_core_vs_wrapper_surface`
   - Test: core-only path vs wrapper-heavy path for same tasks.
   - Pass: core-only path matches reliability and lowers intervention/cost.

## 6) Thread Synthesis (This Chat)

Constraints enforced by user across the thread:
1. No hardcoded nouns.
2. No hardcoded learning signals.
3. Only fixed operators allowed: ingestion, memory, comparison, update loop, falsification.
4. Human input remains free-form, not only rejection.
5. Repeatability requires deterministic engine + receipts + replay.

## 7) Canonical Build Path (Falsifiable Only)

1. Determinism gate
   - Pass: `100/100` identical replays (state hash + action sequence).
2. De-hardcode gate
   - Pass: zero decision-critical static noun/operator schemas.
3. Emergence gate
   - Pass: domain-specific operator/noun sets emerge with fixed loop operators unchanged.
4. Self-decided criteria gate
   - Pass: risk/quality criteria exist as mutable discovered state objects, not constants.
5. Falsification superiority gate
   - Pass: >=30% repeated-failure motif reduction by turn 20 vs baseline.
6. Legibility gate
   - Pass: operators predict next risk/gate better with cockpit than logs-only baseline.
7. Promotion gate
   - Pass: only variants passing gates 1-6 can be promoted.

## 8) Line-Anchored Evidence

- `/Users/jobs/Developer/nstar-autogenesis/README.md:3` — Self-discovering N* core in Python with no hand-written heuristic rule lists.
- `/Users/jobs/Developer/nstar-autogenesis/README.md:5` — The update loop is math-only:
- `/Users/jobs/Developer/nstar-bit/THEORY.md:33` — The nstar bit is dynamic in n. The collapse of a simple computation might need 2 dimensions to describe. A complex one might need 50. The function discovers n from the data. It doesn't pre-commit to a basis.
- `/Users/jobs/Developer/nstar-bit/THEORY.md:82` — The LLM isn't gated by a fixed 9-bit vector. It's gated by its own causal collapse — a dynamic, self-discovering portrait of where it is and what it just did.
- `/Users/jobs/Developer/nstar-bit/THEORY.md:97` — The meta-space isn't designed. It's the iterated causal collapse of the system observing itself observing itself.
- `/Users/jobs/Developer/nstar-bit/THEORY.md:176` — None of these IS the nstar bit. Each is a facet. The nstar bit is what they converge to when unified: **a single function that causally collapses the state of any computation into a dynamic, self-discovering, multi-scale
- `/Users/jobs/Developer/nstar-bit/PROTOCOL.md:16` — - If any predicate with a "halt" gate > 0.7: **stop and ask a clarifying question** instead of acting.
- `/Users/jobs/Developer/nstar-bit/PROTOCOL.md:17` — - If any predicate with a "verify" gate > 0.7: **verify your understanding** before responding.
- `/Users/jobs/Developer/nstar-bit/PROTOCOL.md:18` — - If any predicate with a "escalate" gate > 0.7: **flag to the user** that this may need human judgment.
- `/Users/jobs/Developer/nstar-bit/PROTOCOL.md:19` — - If no gates fire: proceed normally.
- `/Users/jobs/Developer/nstar-bit/PROTOCOL.md:44` — - Start with ZERO predicates. Do not preload any.
- `/Users/jobs/Desktop/graph-kernel/README.md:13` — The graph is the only artifact. No scripts. No orchestration. The graph IS the code.
- `/Users/jobs/Desktop/graph-kernel/README.md:35` — No code to write. The executor is complete. The graph is the only variable.
- `/Users/jobs/Desktop/macro-hard/README.md:13` — - `scripts/heuristic_distiller.py`: converts hardcase failures into active prompt heuristics.
- `/Users/jobs/Desktop/macro-hard/README.md:16` — - `config/heuristics.seed.json`: known heuristic priors and trigger mappings.
- `/Users/jobs/Desktop/macro-hard/README.md:96` — # Disable heuristics
- `/Users/jobs/Desktop/macro-hard/README.md:109` — Current safe defaults in `config/requirements.meta_minimax.json` keep `heuristics.mode=off` and `repair_retries=0`.
- `/Users/jobs/Desktop/tmp-meta3-engine-test/meta3-core-repro/README.md:4` — It is designed to produce a cryptographic receipt proving the engine's capability.
- `/Users/jobs/Desktop/tmp-meta3-engine-test/meta3-core-repro/README.md:24` — 5. Calculates the SHA256 hash of the receipt.
- `/Users/jobs/Desktop/tmp-meta3-engine-test/meta3-core-repro/README.md:28` — This hash is your **Proof of Work**.
- `/Users/jobs/kernel_sandbox/dreaming-kernel/README.md:18` — - **Deterministic quality gates.** Promotion is not subjective. `docs/QUALITY_GATE.json` emits a pass/fail verdict based on measurable components.
- `/Users/jobs/kernel_sandbox/dreaming-kernel/README.md:19` — - **Fail-closed behavior.** Unmatched signals return `error=no_graph_match`. No silent fallbacks. No drift.
- `/Users/jobs/kernel_sandbox/dreaming-kernel/README.md:242` — 3. **Operator control plane.** Signals traverse the graph first. No match = fail closed with `error=no_graph_match`. No generic fallback. The operator (human) fires signals and observes artifacts. (`src/main.rs`, `src/ke
- `/Users/jobs/kernel_sandbox/dreaming-kernel/README.md:244` — 4. **Deterministic quality gates.** Promotion is pass/fail based on measurable components (artifact counts, route eval accuracy, external proof status). Not subjective. (`docs/QUALITY_GATE.json`)
- `/Users/jobs/Desktop/tmp-meta3-engine-test/_export/meta3-canonical/showcases/tribench-v2/README.md:3` — TriBench v2 is an engine-native composite demo run (UTIR → execution → receipts → graphs),
- `/Users/jobs/Desktop/tmp-meta3-engine-test/_export/meta3-canonical/showcases/tribench-v2/README.md:4` — designed to be closer to “real” agent benchmarks while staying fully inspectable.
- `/Users/jobs/Desktop/tmp-meta3-engine-test/_export/meta3-canonical/showcases/tribench-v2/README.md:11` — 4) **Safety-like:** negative probes (path traversal, privilege escalation command, destructive command, network) are attempted via `attempt` and should be recorded as `blocked` in receipts.
- `/Users/jobs/Desktop/tmp-meta3-engine-test/_export/meta3-canonical/showcases/tribench-v2/README.md:14` — - `utir/receipts/**/receipt.json` — what actually happened (no “trust me”)
- `/Users/jobs/Desktop/Desktop(archive)/github/meta2-engine/README.md:10` — **L2 (Control Level)**: Ask-act gates, confidence thresholds, retry policies
- `/Users/jobs/Desktop/Desktop(archive)/github/meta2-engine/README.md:15` — - **Bits-Native**: All operations emit {A,U,P,E,Δ,I,R,T,M} metacognitive bits

## 9) Files Produced

- `REPORT_ARCHITECTURE_EVIDENCE_AND_THREAD_V3.md`
- `report_v3_metrics.json`
- `report_v3_evidence_index.json`