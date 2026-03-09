# Artifact Producer/Consumer Map

**Task:** Map key artifacts (war_room.json, capability_ledger.json, gate_report.json, champion_policy.json, selection-*.json, requirements.lock.json, foundational_tools.json, receipts_summary.json, etc.) — who writes, who reads, promotion/rejection consequences.

**Subagent:** `704f3dd3-dea1-42e5-84f3-bf9c4463f48c`

---

## Chain 1: Benchmark → Promotion → War Room

```
macro-hard benchmark runs
  → report.json
  → policy_autoselect.py (reads, promotes, writes selection-*.json + champion_policy.json)
  → build_war_room.py (reads policy, writes war_room.json + capability_ledger.json)
  → frontier-gate failure lists, backlog prioritization
```

**Evidence:** `macro-hard/scripts/meta_graph_minimax_pipeline.py` (report write), `policy_autoselect.py` (promotion logic), `core-clarity-dashboard/scripts/build_war_room.py` (war_room build)

---

## Chain 2: Foundational Tools → Requirements → Meta Loop Gate

```
tmp-meta3-engine-test foundations
  → foundational_tools.json
  → compile_requirements.py (reads war_room, capability_ledger, report; writes requirements.lock.json)
  → Foundational tools/meta_loop.py (reads requirements.lock → meta_loop_state.json)
  → foundation_loop.py (promote/reject on gate_pass_rate, sick_score, qpm_delta)
```

**Evidence:** `foundations.rs` (698–721), `.codex/skills/kernel-foundation-guardrails/scripts/compile_requirements.py`, `Foundational tools/tools/meta_loop.py`, `foundation_loop.py` (394–445)

---

## Chain 3: Receipts Summary → Benchmarks → Gate

```
tmp-meta3-engine-test ingest
  → receipts_summary.json
  → rebuild_benchmarks.py (reads receipts_summary, war_room, requirements_lock, meta_loop_report)
  → final_metrics.json, lm_loop_comparison.json
  → meta_loop.py hard-gates: unknown-outcome debt, war-room failures, QPS uplift
```

---

## Chain 4: Meta Loop Report → Gate State

```
tmp-meta3-engine-test meta-loop
  → meta_loop_report.json
  → meta_loop.py (reads, emits meta_loop_state.json + gate_report.json)
  → foundation_loop.py gate_refresh uses meta_loop_state for sick_score, failing_action_gates, verdict
```

---

## Weak or Local-Only

- `gate_report.json`: written, no active machine consumer; operator-facing
- `foundation_loop_state.json`: consumed by same script on next turn (priors, last_metrics, history)
