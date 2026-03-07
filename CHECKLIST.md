# Checklist: Get to a Valid Test of the Thesis

**Date:** 2026-03-07
**Goal:** One 20-turn run on a behavioral substrate that produces enough data to answer: does the LM-authored scoring rule predict held-out behavioral co-occurrence better than chance?

---

## Current State (Verified)

- 2 behavioral nodes, 1 hypothesis edge, 5 receipts (4 rolled back)
- 34 behavioral nodes exist across the strict experiment suite (Mar 3) but with 0 hypothesis edges
- The LM writes behavioral conditions without FORBIDDEN/REQUIRED constraints (confirmed)
- The scoring rule evolves (12 mutations in 18 turns on epoch 1)
- The hypothesis substrate only uses `node:` prefixed behavioral nodes (sym: filtered out)
- The receipt chain, replay, OVM, and held-out evaluator all work mechanically

## What's Blocking a Valid Run

### B1. State deserialization crash on null f32
**Symptom:** `Error: invalid type: null, expected f32 at line 62 column 28` — repeated in ab_adversarial.log. Once a null lands in the state JSON, every subsequent run dies on load.
**Root cause:** `GraphNode` fields (`activation`, `threshold`) and `CanonicalProposal.quality` are `f32` in serde, not `Option<f32>`. If the LM returns null for any field that flows into state, the next load crashes.
**Fix:** Add `#[serde(default)]` to all f32 fields in `GraphNode`, `GraphEdge`, and `CanonicalCriteria` in `types.rs`. This is 10 minutes of work. Without it, any extended run will eventually hit a null from an LM response and corrupt state permanently.
**Cost of not fixing:** Every run longer than ~10 turns has a chance of bricking state.

### B2. Gate rejects bootstrap turns (evidence_coverage=0.00)
**Symptom:** 4/5 turns on clean state rolled back with `insufficient_evidence_coverage:0.00, contradiction_score_exceeded:0.70`.
**Root cause:** Two compounding issues:
1. `contradiction_score` adds 0.70 when `gate.has_signal("assert:wrote")` but no write effect exists. The LM says it wrote something (because it emitted `define_scoring_rule`) but the system doesn't count OVM operations as write effects. Threshold is 0.10. One false assertion = rollback.
2. `evidence_coverage` requires effects to satisfy gate signals. When the task is conceptual (no files to read/write), required evidence is non-empty but satisfied evidence is empty → coverage=0.00. Threshold is 0.70.
**Fix options (pick one):**
- **(a) Minimal:** Count OVM operations (`DefineScoringRule`, `DefineSelectionPredicate`) as satisfying the `assert:wrote` signal in `invariants.rs`. ~20 lines.
- **(b) Bootstrap relaxation:** When `graph.nodes.len() < 5`, skip evidence_coverage and contradiction checks entirely. Let the system accumulate data before applying the gate. Remove the relaxation once the system can evaluate its own quality. ~10 lines in `core.rs`.
- **(c) Criteria adjustment:** Lower `contradiction_threshold` from 0.10 to 0.80 and `min_evidence_coverage` from 0.70 to 0.30 in the default criteria. Blunt but quick.
**Recommended:** (a), because it fixes the actual semantic error (OVM ops ARE write behavior) without relaxing the gate.

### B3. Rate limit and retry thrash
**Symptom:** 58 rate-limit errors for 20 successful turns in ab_adversarial.log. 287 MB of retry noise.
**Root cause:** Each turn makes 3 LM calls (propose, evaluate predicates, reflect). At 5+ retries per call with model fallback, one bad stretch produces hundreds of failed requests.
**Fix:** Add exponential backoff with jitter on 429/503 in `chat_raw` (currently just retries immediately). Cap retries at 5 per model. Log retry count per turn so you can see the actual cost.
**Cost of not fixing:** API budget burns on retries; logs are unreadable; long runs take 10x longer than necessary.

---

## Checklist: Minimum Viable Run

### Phase 0: Fix the blockers (afternoon)

- [ ] **B1:** Add `#[serde(default)]` to f32 fields in `types.rs` (`GraphNode.activation`, `GraphNode.threshold`, `GraphEdge.weight`, `CanonicalCriteria` floats). Rebuild. Verify old state loads without crash.
- [ ] **B2:** In `invariants.rs`, count OVM operations as satisfying `assert:wrote`. Specifically: if `proposal.operations` includes any `DefineScoringRule` or `DefineSelectionPredicate` (check via the proposal's OVM ops list), treat `assert:wrote` as satisfied.
- [ ] **B3:** Add `tokio::time::sleep` with exponential backoff (1s, 2s, 4s, 8s) on 429/503 in `lm.rs::chat_raw`. Cap at 4 retries per model before falling through to backup.
- [ ] Reset state: `./target/release/canonical --reset`
- [ ] Smoke test: run 3 turns manually, verify all 3 commit (not rollback), verify state loads cleanly after.

### Phase 1: Accumulate data (one session, ~1 hour of API time)

- [ ] Prepare 20 prompts that create selection pressure. Use `epoch_tasks_adversarial.txt` as base — it already has claim/challenge/falsify structure. Verify it has 20 lines.
- [ ] Run: `./target/release/canonical < epoch_tasks_adversarial.txt` (or equivalent batch mode).
- [ ] After run, verify:
  - [ ] 20 receipts in `canonical_receipts.jsonl`
  - [ ] ≥5 behavioral nodes discovered (check `nstar_canonical_state.json`)
  - [ ] ≥1 hypothesis edge with c11 > 0
  - [ ] Scoring rule mutated at least once (check `rule_trajectory.jsonl`)
  - [ ] Receipt chain intact (no hash breaks)
  - [ ] Commit rate > 50% (≥10 of 20 turns committed, not rolled back)

### Phase 2: Evaluate (30 minutes)

- [ ] Run held-out eval on behavioral data:
  ```
  python3 scripts/heldout_eval.py \
    --receipts canonical_receipts.jsonl \
    --k 20 \
    "<current_scoring_rule>"
  ```
- [ ] Record P@K and R@K. This is the first number on behavioral substrate.
- [ ] Compare to the 4% P@50 baseline on sym: substrate. Any improvement is signal.
- [ ] Run epoch metrics:
  ```
  python3 scripts/epoch_metrics.py canonical_receipts.jsonl
  ```
- [ ] Save results as `epoch_behavioral_001_metrics.json`.

### Phase 3: Interpret (decide next move)

- [ ] **If P@K > 0.10 on behavioral substrate:** The architecture works when the input is right. Proceed to multi-epoch runs and seed queue closure (Milestone 2 from MILESTONES.md).
- [ ] **If P@K ≈ 0.04 (same as sym: baseline):** The substrate change didn't help. The problem is deeper — either the behavioral conditions aren't actually predictive of each other, or 20 turns isn't enough data. Next step: run 50 turns, or change the task domain.
- [ ] **If the run crashes or most turns roll back:** The blockers weren't fully fixed. Debug, fix, try again.

---

## What Makes This Quicker and Cheaper

### Cheaper LM usage
- **Use the cheapest model that writes behavioral conditions.** The strict suite proved `gemini-3-flash-preview` works. Don't fall back to Claude Opus unless flash is down.
- **Reduce meta-pass token budget.** `max_completion_tokens: 2048` in both `chat_raw` and `execute_task`. The evaluate_predicates and reflect calls rarely need 2048 tokens. Set evaluate to 1024, reflect to 512. Saves ~40% of meta-pass cost.
- **Backoff on retries.** Current behavior: immediate retry, burns tokens on 429 responses that will also 429. Exponential backoff costs nothing and prevents cascade waste.

### Faster iteration
- **Offline eval is microseconds.** `scripts/heldout_eval.py` and `scripts/eval_rule.py` evaluate scoring rules against receipt data without any LM calls. Use these to test rule candidates before committing to a live run.
- **Batch mode.** Pipe prompts from a file rather than interactive mode. One command, walk away, check results.
- **Don't rerun what already worked.** The strict suite has 60 turns of behavioral data across 3 experiments (exp3, exp5, exp7) with 34 behavioral nodes. If the receipt format is compatible, run held-out eval on those first — you may already have enough data without a new run.

### Smaller scope
- **One domain, one task set, 20 turns.** Don't run A/B comparisons, legible disagreement demos, or multi-epoch trajectories until Phase 2 produces a number. Everything else is downstream of "does behavioral P@K beat lexical P@K?"
- **Skip multi-scale.** Session/Project scale coordinates are inert. Don't wire them up until Turn scale proves the operator works.
- **Skip seed queue closure.** Autonomous exploration is Milestone 2. Human-prompted runs are fine for the first valid test.

---

## Quick-Check: Can We Reuse Strict Suite Data?

The strict suite (Mar 3) has 60 turns with behavioral nodes. Before running a new 20-turn session, check:

- [ ] Do the strict suite receipts have `coordinates` with `Scale::Turn` and `active_nodes` containing `node:` entries?
- [ ] If yes: run `heldout_eval.py` against those receipts now. You might already have the answer.
- [ ] If no: the receipt format changed since Mar 3 and the data isn't compatible. New run needed.

This takes 5 minutes and could save an hour of API cost.

---

## State We Need To Be In

After completing this checklist, we should have:

1. A canonical binary that doesn't crash on null f32 and doesn't reject bootstrap turns
2. ≥20 receipts on a behavioral substrate with ≥5 nodes and ≥1 meaningful hypothesis edge
3. A held-out P@K number on behavioral data
4. A clear comparison to the 4% P@50 lexical baseline
5. A go/no-go decision on whether the architecture produces signal when the substrate is right

That's the minimum viable evidence for the thesis. Everything in MILESTONES.md flows from it.
