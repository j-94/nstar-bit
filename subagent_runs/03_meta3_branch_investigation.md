# Meta3 Branch Investigation

**Task:** Investigate tmp-meta3-engine-test across branches (main, singularity_main, feat/distilled-engine, feat/graph-emergence, feat/mia-injection, meta-autopoiesis-cycle-1). Find best branch for nstar-bit replacement.

**Subagent:** `28d6f084-7e0d-428a-b11f-8e398925c273`

---

## Verdict

**Best branch for direct nstar-bit replacement: `feat/mia-injection`.**

Not the cleanest branch, but the only one that combines:
- separate runtime substrate
- real hypergraph control plane
- eval manifest generation
- receipt ingestion/auditability
- explicit policy/guard machinery

`feat/distilled-engine` is cleaner; `feat/mia-injection` is materially more complete.

---

## Branch Ranking

1. `feat/mia-injection` — most complete
2. `feat/distilled-engine` — cleanest conceptual baseline
3. `meta-autopoiesis-cycle-1`
4. `singularity_main` — canonical remote
5. `codex/negentropy-hologram-mode`
6. `codex/live-integrations`
7. `feat/graph-emergence` — research only
8. `main` — historical stem

---

## Key Architectural Changes by Branch

| Branch | Architecture |
|--------|--------------|
| `main` | Historical stem, monolithic one-engine |
| `singularity_main` | Canonical remote, still monolithic |
| `feat/graph-emergence` | Research spike, proof-space/policy experiments |
| `feat/distilled-engine` | First pivot: separate meta3-graph-core, receipt-first execution |
| `meta-autopoiesis-cycle-1` | Autopoietic CI, self-updating workflow, governance |
| `feat/mia-injection` | Full control-plane: UTIR, guarded executor, receipts, hypergraph probes, eval harnesses |

---

## Exact Files (feat/mia-injection)

**Runtime:**
- `meta3-core-repro/engines/meta3-graph-core/src/{lib,utir,utir_exec,hypergraph,llm_gateway}.rs`

**Receipt/audit:**
- `receipt.rs`, `receipt_event.rs`, `reproduce.sh`, `protocols/golden_run.json`

**Hypergraph binaries:**
- `graph_context_bundle`, `graph_eval`, `graph_guard`, `graph_harness_emit`, `graph_lens`, `graph_merge_packets`, `graph_nativeize_tasks`, `graph_probe`, `graph_receipt_ingest`, `graph_utir_ingest`, `merge_mission_hypergraph`, `render_hypergraph`

**Eval:**
- `scripts/eval_manifest.py`, `scripts/harness_loop.sh`, `scripts/graph_core_eval.sh`, `scripts/eval_gate.sh`

---

## Canonical vs Abandoned

- `singularity_main`: canonical
- `feat/distilled-engine`: superseded milestone, not abandoned
- `feat/mia-injection`: best architecture, no upstream tracking, active experimental head
