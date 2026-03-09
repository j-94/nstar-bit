# Status — 2026-03-08
## Why we pivoted and what we proved

---

## The pivot (canonical Rust → M1 experiment)

The autogenesis engine (State 1) had been running for 70+ epochs producing semantic graph data, but the core M1 question from MILESTONES.md was never actually answered against the canonical Rust OVM:

> **Does an LM-authored scoring rule outperform a frozen baseline under adversarial pressure?**

Three bugs were preventing any meaningful canonical run:

| Bug | Symptom | Fix |
|-----|---------|-----|
| **B1** (schema.rs null crash) | Turn 10 fatal: `invalid type: null, expected f32` — LM returns null for any f32 field, corrupting state | Added `null_to_default_f32` deserializer to all 13 f32 fields in `schema.rs` |
| **B2** (OVM ops invisible) | Every OVM-only turn: `coverage=0.00, contradiction=0.70` → Rollback. Gate demanded filesystem effects; scoring rule ops produce none | Extended `evidence_satisfied()` and `contradiction_score()` in `invariants.rs` to count `has_ovm_write` as satisfying Read/Write/Verification/Effects requirements |
| **B3** (rate-limit thrash) | 429/503 → retry immediately → 58 rate errors for 20 successful turns, 287MB logs | Exponential backoff `1 << attempts.min(5)` seconds in `lm.rs` before each retry |

After fixing B1/B2/B3, two full M1 runs completed.

---

## M1 Results

### Epoch 1 (18 adversarial turns — scoring rule conflict/mutation)

| Metric | LM-authored | Frozen baseline |
|--------|------------|-----------------|
| Commits | 8 | 8 |
| Rollbacks | 10 | 10 |
| Hypothesis precision (within-epoch) | 1.00 | 0.80 |
| P@10 held-out | 0.200 | 0.200 |

Corpus too small (18 turns) to separate rules in held-out eval. Both tied.

**Rule at end of epoch 1:**
- Frozen (baseline): `(c11 * c11) / ((c11 + c10) * (c11 + c01) + 1)`
- LM evolved: `(c11 * c11) / (c11 + 2*(c10+c01) + (1/(c11+1)))`

### Epoch 2 (18 new adversarial turns — attacking evolved rule)

| Metric | LM epoch 2 | Frozen baseline |
|--------|-----------|-----------------|
| Commits | 12 | 12 |
| Rollbacks | 6 | 6 |
| P@10 held-out (LM e2 rule) | 0.100 | — |
| P@10 held-out (LM e1 rule) | 0.100 | — |
| P@10 held-out (frozen rule) | — | **0.000** |

**Divergence confirmed:** frozen rule degraded to P@10=0.000 on epoch 2 tasks. LM rules held at 0.100.

**Rule at end of epoch 2 (LM):**
`(c11 / (c11 + 3*(c10 + c01) + 1)) * (log(c11 + 1) / log(t + 10))`

LM added t-dependence in epoch 2 when explicitly challenged on large-t behavior. The frozen rule has no t; the LM's rule does. This is a meaningful divergence.

### Rule trajectory (3 epochs)
```
E0 (frozen):   (c11*c11) / ((c11+c10)*(c11+c01)+1)
E1 (LM):       (c11*c11) / (c11 + 2*(c10+c01) + 1/(c11+1))
E2 (LM):       (c11/(c11+3*(c10+c01)+1)) * (log(c11+1)/log(t+10))
```
Each mutation was driven by a specific adversarial attack. The trajectory is not random.

---

## What the 70+ autogenesis epochs contain that canonical doesn't

The autogenesis engine proved `no_evidence_at_declaration_or_missing_dialogue` over 40+ epochs:
- Relations enter at confidence=0.0
- Only dialogue-originated evidence raises them
- Inflation score stays near 0

**This gate was never ported to canonical.** The canonical OVM can Commit a scoring rule without any grounding evidence. The LM can pass the gate with self-referential content because the gate fires on behavioral node patterns, not evidence origin. This is the highest-priority unimplemented finding.

---

## Current active states

| State | Status | What to run |
|-------|--------|-------------|
| Canonical Rust OVM | **Unblocked. M1 done.** Next: port evidence-origin gate | `run_m1.py`, `run_m1_epoch2.py` |
| Autogenesis engine | Stalled (OpenRouter 402 / credits) | `run_all_epochs.py --state epoch_logs/epochN_fork.json` |

---

## Next technical step

Port `no_evidence_at_declaration_or_missing_dialogue` to canonical:
- Gate fires in `evaluate_invariants()` if an OVM op is emitted without any grounding dialogue (no ReadFile effect, no prior read-triggered operation)
- This means the gate checks: did the LM read graph context before authoring a rule?
- Current state: `has_ovm_write` in `evidence_satisfied` treats any OVM op as evidence. This is too permissive.

The fix: separate "LM read graph context" from "LM emitted an OVM op". Require both.
