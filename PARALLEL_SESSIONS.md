# Parallel Sessions — What to Run Now

While epoch 1 grinds, these are independent work streams that don't touch
the canonical state or receipt chain. Each is a self-contained session.

---

## Session A: Adversarial Task Set

The current 18 prompts are essays. They produce no contradictions, so the
scoring rule never needs to evolve. Write a task set that creates selection
pressure.

Good tasks force hypothesis conflict:
- "Is X more predictive of Y than Z is?" → promotes a hypothesis
- "Show a case where X did NOT predict Y." → challenges that hypothesis
- "Given the last 5 turns, which hypothesis should be killed?" → forces falsification

Aim for 20 tasks. Alternate between: claim, challenge, falsify, restate.
The pattern is assert → stress-test → break → revise.

Save as: `epoch_tasks_adversarial.txt` (one prompt per line)
This is pure text work. No code. No state.

---

## Session B: Frozen-Rule CLI Flag

Add `--frozen-rule "expr"` to `src/bin/canonical.rs`.
When set, skip all `DefineScoringRule` OVM ops and use the provided expression.
This is the A/B baseline harness.

Touch list:
- `src/bin/canonical.rs` (add clap arg, pass to core)
- `src/canonical/core.rs` (skip DefineScoringRule when frozen_rule is Some)

Test: `cargo run --bin canonical -- --frozen-rule "(c11 * t) / ((c11 + c10) * (c11 + c01) + 1)" "test prompt"`
Verify the rule in graph state matches the CLI arg, not the LM proposal.

No state conflict — you're adding a flag, not changing the pipeline.

---

## Session C: Rule Trajectory Logger

Add `rule_trajectory.jsonl` output. Every time `DefineScoringRule` fires
in `core.rs`, append one line:

```json
{
  "turn": 4,
  "old_rule": "(c11 * t) / ...",
  "new_rule": "(c11 / sqrt((c10+1)*(c01+1))) * ...",
  "hypothesis_count": 12,
  "mean_weight": 0.34,
  "reason": "<extracted from LM proposal text>"
}
```

Touch list:
- `src/canonical/core.rs` (the DefineScoringRule match arm)
- One new file append, same pattern as receipt chain

This is additive. It doesn't change any existing behavior.

---

## Session D: Epoch Metrics Script

Write `scripts/epoch_metrics.py` that reads `canonical_receipts.jsonl`
and computes the 8 metrics from the critique:

1. Hypothesis precision (promoted edges confirmed by later turns)
2. False positive rate (promoted edges that die within 5 turns)
3. Contradiction slope (linear regression over contradiction_score)
4. Operator delta (did the scoring rule change at all?)
5. Operator fitness delta (late-epoch precision minus early-epoch)
6. Replay determinism (call `cargo run --bin replay` and check)
7. Receipt chain integrity (hash chain unbroken?)
8. Failure motif reduction (repeated failure patterns)

Input: `canonical_receipts.jsonl`
Output: `epoch_001_metrics.json`

This reads receipts. It doesn't write state. Fully independent.

---

## Session E: Fix macro-hard Promotion Gates

Mechanical replacement work from DIVERGENCE.md:
- `utility` thresholds → OVM edge weight thresholds
- `active_core` reads → hypothesis edges with weight > 0
- `pm`/`pv` convergence → `reinforcements` count or `c11/t`
- Promotion trigger → `passes_promotion` from canonical state

This is in the macro-hard repo, not nstar-bit. Completely independent.

---

## Priority Order

| Priority | Session | Time | Why first |
|----------|---------|------|-----------|
| 1 | A (task set) | 30 min | Current tasks produce no learning signal |
| 2 | D (metrics script) | 1-2 hr | Need this to evaluate epoch 1 |
| 3 | B (frozen-rule flag) | 1 hr | Need this for A/B comparison |
| 4 | C (trajectory logger) | 30 min | Free data, minimal code |
| 5 | E (macro-hard gates) | 2-3 hr | Important but not blocking epoch 1 |

Sessions A+D are highest leverage. A fixes the input quality.
D gives you the tool to evaluate output quality. B+C set up the
comparison harness for epoch 2.
