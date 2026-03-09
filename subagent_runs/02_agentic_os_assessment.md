# Agentic-OS Assessment

**Task:** Follow clue from `tmp-meta3-engine-test/artifacts/requirements.lock.json` into agentic-os. Assess as standalone system. Determine if more important than tmp-meta3.

**Subagent:** `5b0ed010-9bbd-4f12-89b5-843c923c54de`

---

## 1. What System It Is

`/Users/jobs/Desktop/agentic-os` is a lightweight agent governance and orchestration kernel for running auditable R&D loops across other repos.

- Ingests prior context
- Runs a task under policy
- Emits receipts
- Scores result against benchmark
- Updates normalized state snapshot

---

## 2. Primitives

- `task` JSONs in `tasks/`: objective, workspace roots, scan markers, evidence checks, expected keywords
- `context_pack`: distilled prior session history
- `workspace_scan`: file/marker presence + substring-based evidence checks
- `policy`: hard gates (min evidence refs, required workspace scan, forbidden substrings)
- `benchmark`: weighted scoring over schema presence, evidence linkage, keyword alignment
- `gate_decision`: `promote` or `hold`
- `receipts`: run.json, inputs.json, outputs.json, policy.json, benchmark.json, gate.json
- `state snapshot`: `state/latest.snapshot.json`

---

## 3. Strongest Evidence It Is Real

- Real runner: `scripts/run_core_loop.py`
- Many receipt dirs under `receipts/runs/`
- Concrete promoted run: `receipts/runs/run-20260219T155745Z-a2fb7143/`
- State snapshot: 18 runs ending in green promoted state
- Live smoke proof with `exit_code: 0` into tmp-meta3

---

## 4. Role Relative to Other Systems

- `tmp-meta3` = capability/runtime substrate
- `agentic-os` = evaluator, coordinator, receipt-writer, promotion authority
- `tmp-meta3` depends on agentic-os (requirements.lock imports `agentic-os/docs/STATUS_REPORT.md`)
- `agentic-os` depends on tmp-meta3 for runtime proof and dogfood

**Judgment:** agentic-os is upstream in governance, downstream in capability.

---

## 5. Fatal Weakness

Execution and benchmark are shallow and self-referential:

- `run_task()` emits templated summary, not substantive execution
- `evaluate_benchmark()` can give perfect score from required fields, evidence IDs, keyword presence
- Promotion vulnerable to gaming by formatting/keyword alignment
- Strongest runtime proof is delegated to tmp-meta3

---

## 6. Verdict: 5/10

**Why not lower:** Right system shape (loop spec, policy pack, benchmark pack, receipts, state). Reusable control plane across repos. tmp-meta3 imports its outputs as authoritative.

**Why not higher:** Benchmark/promotion regime is shallow. Core task execution is mostly synthesized boilerplate. Best runtime evidence comes from another system. Dashboards/operator surfaces mostly placeholders.

**Bottom line:** Real and important governance kernel, but not stronger than tmp-meta3. It is the manager, not the engine.
