# N★ Bit — Milestones

**Date:** 2026-03-06
**Starting state:** Canonical pipeline runs. OVM wiring complete. Replay deterministic. Experimental harness built (adversarial tasks, frozen-rule flag, trajectory logger, metrics script). Epoch 1 running.

---

## Milestone 1: The Hard Question Answered

**Gate:** Three epoch metric files exist and the comparison is unambiguous.

| Epoch | Config | Purpose |
|-------|--------|---------|
| 1 | Essay prompts, LM-free rule | No-pressure baseline |
| 2 | Adversarial prompts, LM-free rule | Does the rule evolve under pressure? |
| 3 | Adversarial prompts, frozen EMI rule | Does the LM-authored rule beat the human-authored one? |

**Pass condition:** Epoch 2 hypothesis precision exceeds epoch 3 by ≥ 0.1, contradiction slope is negative, and the rule mutated at least twice.

**Fail condition:** Rule is static across all 20 turns, or frozen EMI matches or beats the LM rule. If this fails, the system is infrastructure for a static scorer and the architecture needs revision before anything else proceeds.

**Deliverable:** `EPOCH_COMPARISON.md` with the three metric JSONs and a one-paragraph verdict.

---

## Milestone 2: Seed Queue Closes the Loop

**Depends on:** Milestone 1 passes.

**What:** The system selects its own next investigation target. After each turn, the top-K highest-uncertainty hypothesis edges (high `c01 + c10` relative to `c11`) are formatted as context for the next LM call. No human prompt needed.

**Gate:** Run 20 autonomous turns. Compare hypothesis precision and discovery rate to the human-prompted adversarial epoch. Autonomous exploration must discover at least one hypothesis that the fixed task set missed.

**Deliverable:** `epoch_autonomous_001_metrics.json` + comparison to epoch 2.

**Why this is milestone 2:** Until the operator proves it evolves (M1), autonomous exploration just amplifies a static system. Get the scoring right first, then let it steer.

---

## Milestone 3: Multi-Epoch Rule Trajectory

**Depends on:** Milestone 1 passes, trajectory logger exists.

**What:** Run 5 epochs sequentially. Each epoch starts with the previous epoch's final scoring rule as the seed. Track how the rule evolves across epochs, not just within one.

**Gate:** The rule at epoch 5 outperforms the rule at epoch 1 when both are evaluated against epoch 5's count data (rule regression test). The trajectory shows convergence, not drift or oscillation.

**Deliverable:** `rule_trajectory_cross_epoch.jsonl` + regression analysis showing monotonic improvement or identifying where it plateaus.

**Why this is milestone 3:** This is the first evidence of meta-learning — rules about rules. If cross-epoch improvement is real, it justifies building the meta-scoring loop. If the rule plateaus at epoch 2, you know the ceiling.

---

## Milestone 4: Legible Disagreement Demo

**Depends on:** Milestone 1 passes, frozen-rule flag exists.

**What:** Run the same adversarial task set through two different LMs (or two different system prompts). Diff the converged scoring rules. Produce a document showing exactly where the two systems disagree about what constitutes evidence.

**Gate:** The two rules are meaningfully different (cosine distance > 0.1 on tokenized expressions) and the performance difference is attributable to the rule difference, not random variance.

**Deliverable:** `LEGIBLE_DISAGREEMENT.md` with rule diffs, metric comparison, and interpretation. This is the demo you show people.

**Why this is milestone 4:** It's the highest-novelty artifact the system can produce. Nobody else can generate a readable, falsifiable diff of what two LMs think matters. But it requires M1 to be credible.

---

## Milestone 5: Multi-Scale Activation

**Depends on:** Milestones 2 and 3.

**What:** Wire up Session and Project scale coordinates. Session scale aggregates across turns within one epoch. Project scale aggregates across epochs.

**Gate:** Session-scale activation produces at least one signal that Turn-scale alone does not — e.g., detecting that a hypothesis is strong within-turn but unstable across-turns (oscillation detection). Project-scale produces at least one cross-epoch insight.

**Deliverable:** Updated `canonical_receipts.jsonl` schema with multi-scale coordinates populated. Evidence of at least one decision that used Session or Project scale information.

**Why this is milestone 5:** The theory depends on multi-scale recursion. The implementation has been Token-only. But wiring it up without evidence that Turn-scale works (M1–M3) is premature. Get the foundation right first.

---

## Milestone 6: macro-hard Promotion Gate Update

**Depends on:** Milestone 1 (so you know what the canonical state looks like).

**What:** Replace all dead field references in macro-hard:
- `utility` → OVM edge weight
- `active_core` → hypothesis edges with weight > 0
- `pm`/`pv` → reinforcements count or `c11/t`
- Promotion trigger → `passes_promotion` from canonical state

**Gate:** macro-hard promotion runs against canonical Rust state without errors. Promotion correctly blocks when `passes_promotion` is false and passes when true.

**Deliverable:** Updated macro-hard config + CI test that validates promotion against a known-good canonical state file.

**Why this is milestone 6:** Important but mechanical. Doesn't generate new knowledge. Can be done anytime after M1 proves what the canonical state actually contains.

---

## Milestone 7: Meta-Scoring Loop

**Depends on:** Milestone 3 (multi-epoch trajectory exists with enough data).

**What:** Build the second-order loop. The system evaluates which scoring rules produced good epochs, and uses that history to propose better rules. The input to the LM includes not just the current count data but the (rule, epoch_metrics) pairs from previous epochs.

**Gate:** An epoch seeded with meta-scoring history outperforms an epoch without it. Hypothesis precision at turn 20 is ≥ 0.15 higher with history than without.

**Deliverable:** `meta_scoring_protocol.md` + `epoch_meta_001_metrics.json` showing the improvement.

**Why this is milestone 7:** This is the theoretical crown jewel — the system learns what makes good rules, not just good hypotheses. But it requires 5+ epochs of trajectory data (M3) to have anything to learn from. Do not attempt before M3 is complete.

---

## Milestone 8: First External Validation

**Depends on:** Milestones 1, 4, and either 3 or 5.

**What:** Run the system on a domain someone else cares about. Pick a concrete task — code review, medical triage, financial screening — where hypothesis quality is independently measurable. Show that the LM-authored scoring rule outperforms a hand-tuned baseline in that domain.

**Gate:** External evaluator (not you) agrees the system produced better results than the baseline on a task they defined.

**Deliverable:** Write-up with methodology, metrics, and the external evaluator's assessment. This is what makes the project credible outside your workspace.

---

## Dependency Graph

```
M1 (Hard Question)
├── M2 (Seed Queue)
│   └── M5 (Multi-Scale)
│       └── M7 (Meta-Scoring)
├── M3 (Multi-Epoch Trajectory)
│   └── M7 (Meta-Scoring)
├── M4 (Legible Disagreement)
├── M6 (macro-hard Gates)
└── M8 (External Validation) ← requires M1 + M4 + (M3 or M5)
```

Everything flows from M1. If M1 fails, the project pivots. If M1 passes, the path to M8 is clear and each milestone produces a numbered artifact with hard metrics.

---

## Timeline (Aggressive but Realistic)

| Week | Milestone | Artifact |
|------|-----------|----------|
| 1 (now) | M1 — Three epochs, hard question answered | `EPOCH_COMPARISON.md` |
| 2 | M2 — Seed queue closure + autonomous epoch | `epoch_autonomous_001_metrics.json` |
| 2–3 | M3 — Five sequential epochs, cross-epoch regression | `rule_trajectory_cross_epoch.jsonl` |
| 3 | M4 — Two-LM comparison, legible disagreement | `LEGIBLE_DISAGREEMENT.md` |
| 4 | M5 — Multi-scale wiring + evidence of Session-scale signal | Updated receipt schema |
| 4 | M6 — macro-hard gate update | CI test passing |
| 5–6 | M7 — Meta-scoring loop with history | `epoch_meta_001_metrics.json` |
| 6–8 | M8 — External domain validation | External write-up |

The first month is M1–M4. That is where the project either proves its thesis or discovers it needs to change direction. Everything after M4 is building on confirmed ground.
