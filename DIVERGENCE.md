# DIVERGENCE.md

**What changed when `nstar-autogenesis/engine.py` was stripped to raw counts.**

This document exists to keep the outer pipeline (`nstar-autogenesis` → `nstar-bit` → `meta3` → `macro-hard`) honest about what the math layer now is versus what it used to be. Promotion gates in `macro-hard` were calibrated against Python-computed scores. Those scores no longer exist. Read this before touching any promotion threshold.

---

## 1. What `engine.py` Used to Compute

The original `engine.py` (pre-strip) computed three things per turn beyond raw counts:

### Utility (Expected Mutual Information)
Each hypothesis `h:{a}|{b}` had a `utility` field. This was expected mutual information derived from the contingency table:

```
P(a,b)  = c11 / t
P(a,¬b) = c10 / t
P(¬a,b) = c01 / t
P(¬a,¬b)= c00 / t

utility = sum over cells: P(x,y) * log2(P(x,y) / (P(x) * P(y)))
        = EMI — a real-valued score in [0, 1] bits
```

Zero utility meant the two symbols were statistically independent. Positive utility meant co-occurrence was non-random.

### Beta Posterior (`pm`, `pv`)
Each hypothesis carried a Bayesian running estimate of its co-occurrence probability using a Beta(α, β) posterior:

```
α = c11 + 1   (pseudo-count prior)
β = (c10 + c01 + c00) + 1

pm = α / (α + β)   ← posterior mean (expected co-occurrence rate)
pv = α*β / ((α+β)^2 * (α+β+1))  ← posterior variance
```

`pm` was the direct input to active core selection. `pv` measured uncertainty — high variance meant the hypothesis hadn't converged yet and was a candidate for the seed queue.

### Active Core Selection
The `active_core` list was computed from the distribution of `pm` values across all hypotheses:

```
mean_pm = mean of all hypothesis pm values
active_core = [h for h in hypotheses if h.pm > mean_pm]
```

This was a distribution-relative threshold, not a fixed cutoff. As the corpus grew, the bar rose. Hypotheses that were once in the active core could fall out as new, stronger associations emerged.

The active core was the direct input to `meta3`'s node activation seed for the next turn.

---

## 2. What the Rust OVM Does Instead

The `engine.py` math layer is **gone**. `engine.py` now only accumulates raw counts (`c11`, `c10`, `c01`, `c00`) and does nothing else. The hypothesis struct still has `pm` and `pv` fields in the JSON schema but they are never updated — they sit at their initial values (0.5 and 0.083) forever.

The Rust OVM (`src/canonical/graph.rs: apply_operator`) takes over scoring with a different architecture:

**The LM defines the math.** Via `define_optimization_target` and `define_selection_predicate` operations in its UTIR proposal, the LM writes an `evalexpr` expression that gets stored as `graph.scoring_rule` and `graph.selection_predicate`. The OVM then evaluates these expressions against every hypothesis edge's raw counts each turn.

Available variables in the scoring context: `c11`, `c10`, `c01`, `c00`, `t` (total observations), and functions `log()`, `sqrt()`, `abs()`.

Example scoring rule from bootstrap Turn 1:
```
(c11 * t) / ((c11 + c10) * (c11 + c01) + 1)
```

This is an approximation of PMI odds ratio — but the LM proposed it, not a human. Future turns can mutate it.

**Key differences from the old math:**

| Property | Old `engine.py` | Rust OVM |
|---|---|---|
| Who defines the metric | Human (EMI formula hardcoded) | LM (expression string in graph state) |
| Activation threshold | Distribution-relative (above mean `pm`) | LM-defined `selection_predicate` |
| Uncertainty tracking | Beta `pv` per hypothesis | Not tracked — falsification loop instead |
| Active core | List computed from `pm` distribution | Hypothesis edges with `score > 0` (or predicate) become `EdgeKind::Supports` |
| Updates to scoring | Never (hardcoded) | Every turn the LM can propose a new operator |
| Failure mode | Silent (always computes) | Explicit violation if rule is empty or eval fails |

The OVM does not compute Beta posteriors. It does not track `pm` or `pv`. It does not select an active core by distribution statistics. The LM's proposed rule replaces all of that.

---

## 3. What `macro-hard` Promotion Gates Need to Update

`macro-hard` was calibrated expecting the following inputs from the pipeline:

1. **`utility` scores** — these no longer exist. Any gate that thresholds on `utility > X` is evaluating against a field that is always 0.0 (default) in the current schema. **Action: replace utility thresholds with OVM edge weight thresholds from `canonical_receipts.jsonl`.**

2. **`active_core` list** — this is no longer computed by `engine.py`. The equivalent in the Rust pipeline is the set of `GraphEdge` entries with `kind: Hypothesis` and `weight > 0` after `apply_operator` runs. These appear as boosted nodes in `recorded_observations` in each receipt. **Action: read active hypothesis edges from graph state, not from `active_core` field.**

3. **`pm` / `pv` convergence criteria** — `pv` was used to detect when a hypothesis had stabilized (low variance = high confidence). This is gone. The equivalent signal in the new system is: a hypothesis edge that has survived multiple falsification rounds without its weight being zeroed. **Action: use `reinforcements` count on the associated graph nodes as the convergence proxy, or use `c11 / t` directly as a raw co-occurrence rate.**

4. **Promotion gate timing** — the old system promoted from `engine.py` → `meta3` when `active_core` stabilized. The new system promotes when `evaluate_promotion()` in `core.rs` returns `passes_promotion: true`, which requires:
   - All receipts in the chain have `deterministic: true` (set by `cargo run --bin replay`)
   - `turn_count >= 10`
   - Either `contradiction_slope < -0.01` OR `(turn_count >= 20 AND failure_rate < 0.5)`

   **Action: gate macro-hard promotion on `passes_promotion` from the canonical state file, not on `active_core` stability.**

5. **Scoring rule validity** — the OVM scoring rule is an `evalexpr` string stored in `graph.scoring_rule`. If it is empty or fails to parse, violations appear in `receipt.violations[]` as `ovm:*` prefixed strings. **Action: add a macro-hard preflight check that rejects any canonical state where `graph.scoring_rule` is empty before promotion runs.**

---

## Summary

The math moved from hardcoded Python (EMI + Beta + mean-threshold) to LM-authored expressions evaluated against raw counts. The outer pipeline must stop reading `utility`, `pm`, `pv`, and `active_core` from Python state and start reading `graph.scoring_rule`, hypothesis edge weights, and `passes_promotion` from the canonical Rust state. Until `macro-hard` is updated to these inputs, it is evaluating against criteria the system no longer uses.
