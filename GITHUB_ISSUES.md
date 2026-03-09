# GitHub Issues — Thread 1 → Thread 2 Port Backlog

*Generated 2026-03-09. Source: full audit of 73 autogenesis epochs + canonical Rust src/.*

**Core finding from the audit:**
The taxonomy was systematically understating how much work has been done. The real gap is
**Thread 1 (autogenesis, `run_all_epochs.py`) → Thread 2 (canonical Rust, `src/canonical/`)**.

The autogenesis system ran 73 epochs / 1,073 turns, grew to 1,025 concepts / 1,601 relations,
and proved a dozen mechanisms the canonical Rust engine doesn't have. Most important work is
PROVEN_UNPORTED — not aspirational.

Labels to create: `proven-unported`, `port-from-thread1`, `thread2-canonical`, `governance`, `hardcoded`

---

## Issues

---

### Issue 1 — [proven-unported] Port `no_evidence_at_declaration` to canonical invariants
**Priority: P0 — highest leverage, ~20 lines of Rust**

**What Thread 1 proved:**
The anti-inflation gate has held for 73 epochs / 1,073 turns. `inflation_score = 0.008` at
turn 1,073. Zero `unsupported_confident` relations. The gate has never been violated in production.

**What Thread 2 is missing:**
`evidence_satisfied()` in `src/canonical/invariants.rs:99` uses `has_ovm_write` (line 44) as a
shortcut that lets **any OVM operation count as evidence**. The LM can author a scoring rule with
zero graph context and the gate fires as satisfied.

```
src/canonical/invariants.rs:44    has_ovm_write = !proposal.ovm_ops.is_empty()
src/canonical/invariants.rs:104   || has_ovm_write   ← too permissive
src/canonical/invariants.rs:110   || has_ovm_write   ← too permissive  
src/canonical/invariants.rs:122   || has_ovm_write   ← too permissive
src/canonical/invariants.rs:124   || has_ovm_write   ← too permissive
```

**The fix:**
Separate "LM read graph context" from "LM emitted an OVM op". Require **both** for an OVM
op to count as evidenced.

```rust
// In evaluate_invariants() — after computing has_ovm_write:
let has_read_before_ovm = proposal.ovm_ops.is_empty()
    || effects.iter().any(|e| matches!(e, Effect::Read { .. } | Effect::FsRead { .. }));

// Replace all `|| has_ovm_write` with:
|| (has_ovm_write && has_read_before_ovm)

// And add a violation if OVM op present but no prior read:
if has_ovm_write && !has_read_before_ovm {
    violations.push("no_evidence_at_ovm_op: scoring rule authored without reading graph context".into());
}
```

**Evidence:** FINDINGS.md §"The gate works", STATUS.md §"What the 70+ autogenesis epochs contain"
**Epoch source:** gate active since epoch 1, proven durable through epoch 73

---

### Issue 2 — [proven-unported] Add `support_set` to GraphEdge schema
**Priority: P1 — enables confidence decay, cascading invalidations**

**What Thread 1 proved:**
Relations enter at `confidence = 0.0`. Only dialogue-originated evidence IDs in `support_set`
can raise confidence. When evidence is withdrawn, confidence decays automatically. This gives
the graph "organic forgetting" without garbage collection. Held 73 epochs.

**What Thread 2 is missing:**
`GraphEdge` in `src/canonical/types.rs:38–54` has `c11/c10/c01/c00` counts but NO `support_set`
field. Thread 2 counts co-occurrences but cannot decay confidence when evidence is removed.

**The fix:**
```rust
// src/canonical/types.rs — in GraphEdge struct:
pub support_set: Vec<String>,    // evidence IDs that raised confidence
pub confidence: f32,             // 0.0 at entry; raised only by support_set items
```

Add `confidence_decay()` fn that removes stale evidence IDs and recomputes confidence.
The `support_set` items are turn IDs from `canonical_receipts.jsonl`.

**Evidence:** `Building Epistemic API.md` lines 104–139, `epoch73_fork.json` (0 unsupported_confident)

---

### Issue 3 — [proven-unported] Port `success_criterion` on proposals
**Priority: P1 — prevents LM rationalizing past a failed gate**

**What Thread 1 proved:**
Before each epoch, the meta-LM commits to a **falsifiable predicate** (e.g., "inflation_score
will decrease AND concept_count will not exceed 950"). The predicate is evaluated deterministically
against the `RunComparisonReceipt` after the epoch. If `criterion_met == False`, adoption is blocked
regardless of LM reasoning. Held since epoch 35–39 through epoch 71.

**What Thread 2 is missing:**
`evaluate_promotion()` in `src/canonical/promotion.rs:233` checks `deterministic_ratio`,
benchmark metrics, and candidate validation — but has **no structured predicate DSL**. The LM
can rationalize any outcome as an improvement.

**The fix:**
Before the LM authors a scoring rule, require it to emit a structured `success_criterion` field:
```json
{
  "ovm_ops": [...],
  "success_criterion": "P_AT_10 > 0.05 AND INFLATION_SCORE < 0.05"
}
```
`evaluate_promotion()` evaluates the criterion deterministically against receipt fields before
allowing `PromotionAction::Promote`.

**Evidence:** FINDINGS.md §"success_criterion on proposals", STATUS.md §M1 Results

---

### Issue 4 — [proven-unported] Port governance refactor (Steps 1–3, epoch 62)
**Priority: P1 — makes thresholds auditable and changeable without code changes**

Three sub-tasks, all proven complete in Thread 1 at epoch 62:

#### 4a — Health signals in receipts
`RunComparisonReceipt` (Thread 1: `run_all_epochs.py`) carries 6 health fields:
`inflation_score_lhs/rhs`, `unsupported_confident_lhs/rhs`, `violation_count_lhs/rhs`.

Thread 2 `CanonicalReceipt` in `src/canonical/types.rs` has no health fields.

**Fix:** Add health fields to `CanonicalReceipt`. Emit them from `make_receipt()` in `core.rs:428`.

#### 4b — Versioned governance thresholds via `policy.json`
Thread 1 loads all thresholds from `epoch_logs/policy.json`. Every portfolio entry carries
`policy_version`. To change a threshold: edit file, restart runner.

Thread 2 uses `CanonicalCriteria::default()` in `types.rs` with hardcoded values.

**Fix:** Load `CanonicalCriteria` from a `policy.json` file at startup. Validate and stamp
every receipt with `policy_version`.

#### 4c — Deterministic criterion gate (`evaluate_criterion` DSL)
Thread 1: two-path evaluator — structured DSL (`FIELD OP VALUE AND ...`) or LM fallback.
Adoption blocked if `criterion_met == False`. LM cannot rationalize past a failed structured gate.

Thread 2: no criterion DSL. `evaluate_promotion()` has no structured predicate path.

**Fix:** New file `src/canonical/criterion.rs` implementing the DSL path.

**Evidence:** FINDINGS.md §"Architecture refactor (Steps 1–3 complete — epoch 62)"

---

### Issue 5 — [proven-unported] Port `semantic_utility_probe`
**Priority: P2**

**What Thread 1 proved:**
Nodes must prove **downstream routing utility** — not just structural position in the graph.
A node that is named and referenced but never actually routes a turn through itself gets scored
lower and eventually quarantined. One of three core proven mechanisms in FINDINGS.md.

**What Thread 2 is missing:**
`evaluate_promotion()` in `promotion.rs` checks candidate scoring rules but does not check
whether nodes are being used as routing intermediaries. A node can accumulate `c11` counts
via surface-level co-occurrence without ever contributing to a resolved turn.

**Evidence:** FINDINGS.md §"Three things were actually adopted", autogenesis epoch 53+

---

### Issue 6 — [proven-unported] Port `measurement_escrow` + `lazy_reconstruction`
**Priority: P2 — addresses ruler-can't-measure-itself finding**

**What Thread 1 proved (epoch 37):**
`stratified_certainty` fell below its own survival threshold when internal signals were removed.
The ruler cannot measure itself. Two adopted mechanisms address this:
- **measurement_escrow**: verification tools must live *outside* the graph they measure;
  snapshot and reconstruct, don't make them immune to the same gates
- **lazy_reconstruction**: pay reconstruction cost on demand, not speculatively

**What Thread 2 is missing:**
The canonical OVM's `evaluate_promotion()` uses the live graph state to evaluate whether
rules should be promoted. The evaluator lives inside the same process as the graph it's evaluating.

**Fix:** Snapshot graph state before evaluation. Evaluator reads the snapshot, not live state.
`src/bin/replay.rs` has the right shape — it already reads a serialized state. Extend to support
in-process snapshots as the evaluation substrate.

**Evidence:** FINDINGS.md §"The ruler can't measure itself", §"Three things were actually adopted"

---

### Issue 7 — [proven-unported] Port `deficit_scan()` — graph-driven targeting
**Priority: P2 — changes epoch planning from LM-driven to graph-driven**

**What Thread 1 proved:**
Instead of asking the LM to plan each epoch from scratch, `deficit_scan()` first queries the
graph for contested relations (high `violation_count`), then fires targeted agents at those
specific nodes. Graph-driven, not LM-driven. Key optimization that made epochs efficient at scale.

**What Thread 2 is missing:**
`run_canonical_corpus.py` processes READMEs in sequence with no priority ordering.
Every README gets equal weight regardless of which graph nodes have contested evidence.

**Fix:** Before each corpus batch, query the canonical graph for nodes with high `c01 + c10`
(evidence against) relative to `c11` (evidence for). Prioritize READMEs that contain those
node labels. This is the same mechanism that made Thread 1 scale to 1,025 concepts efficiently.

**Evidence:** FINDINGS.md §"deficit_scan()", `run_all_epochs.py`

---

### Issue 8 — [hardcoded] Execute PURGE_HARDCODED.md — strip all hardcoded control logic
**Priority: P3 — deferred but necessary**

`PURGE_HARDCODED.md` catalogs 13 hardcoded constants + 16 string matchers + 4 hardcoded
contradiction weights. Estimated ~8 hours of work.

Highest priority items:
- `activation_cutoff: 0.4` in `CanonicalCriteria` — make it a `DefineCriteria` OVM op
- `+0.08` edge weight delta in `graph.rs:246` — make LM-mutable
- `op_risk()` scores (Shell=0.7, FsWrite=0.6) in `core.rs:721` — make policy-driven
- Macro-hard gate dead field references (`DIVERGENCE.md §3`, 5 vestigial Python fields)

**Fix:** Each constant becomes either:
a) A field in `policy.json` (governance threshold), or  
b) A `DefineCriteria` OVM operation (LM-mutable parameter)

---

### Issue 9 — [thread2-canonical] Close the open-loop seed queue (M2)
**Priority: P2 — system is human-cranked without this**

The seed queue exists in both threads but requires manual input. The canonical engine
`nstar_canonical_state.json` shows `seed_queue: []` after 58 turns. Every turn needed
a human to provide the next README.

**Fix:** After each successful `Commit`, the system should generate 1–3 seed prompts and
push them to `graph.seed_queue` as next-turn inputs. Use the active focus nodes + highest-void
concepts to generate seeds. This is the M2 milestone in MILESTONES.md.

---

### Issue 10 — [port-from-thread1] Add `causal_anchor` to GraphEdge schema
**Priority: P3 — makes provenance traceable**

The engine independently discovered this schema gap 39 times across 73 epochs (second most
mentioned concept in the belief graph after alias_deduplication_layer).

**What it is:** A field linking each belief change to the specific `turn_id`s from
`canonical_receipts.jsonl` that caused the change. Currently `GraphEdge` tracks *that*
co-occurrences happened (via c11/c10/c01/c00 counts) but not *which specific turns* contributed.

**Fix:**
```rust
// src/canonical/types.rs — in GraphEdge:
pub causal_turns: Vec<String>,  // receipt IDs that caused c11 increments
```

This is the missing link that makes the pointer architecture complete: you can navigate from
a hypothesis edge back to the exact dialogue evidence that grounded it.

---

## Summary Table

| # | Issue | Priority | Size | Proven Since |
|---|-------|----------|------|-------------|
| 1 | `no_evidence_at_declaration` → `invariants.rs` | **P0** | ~20 lines | Epoch 1 |
| 2 | `support_set` → `GraphEdge` schema | P1 | ~50 lines | Epoch 1 |
| 3 | `success_criterion` → `promote_policy_candidate` | P1 | ~40 lines | Epoch 35 |
| 4a | Health signals → `CanonicalReceipt` | P1 | ~30 lines | Epoch 62 |
| 4b | `policy.json` → load `CanonicalCriteria` | P1 | ~30 lines | Epoch 62 |
| 4c | `evaluate_criterion` DSL → new `criterion.rs` | P1 | ~80 lines | Epoch 62 |
| 5 | `semantic_utility_probe` → scoring | P2 | ~40 lines | Epoch 53+ |
| 6 | `measurement_escrow` + `lazy_reconstruction` | P2 | ~60 lines | Epoch 37 |
| 7 | `deficit_scan()` → corpus runner | P2 | ~50 lines | Epoch ~50 |
| 8 | PURGE_HARDCODED execution | P3 | ~8 hours | — |
| 9 | Close seed queue loop (M2) | P2 | ~40 lines | — |
| 10 | `causal_anchor` → `GraphEdge` schema | P3 | ~20 lines | Epoch 4 |

**Total P0+P1:** ~250 lines of Rust, ~1 day of focused work.
**Unlocks:** The canonical engine becomes as honest as the autogenesis engine has been for 73 epochs.

---

## What's NOT an issue (correctly aspirational)

These are legitimately ASPIRATIONAL — no epoch result points toward them:
- Section 1: Active Inference / Free Energy math (FEP) → the engine achieves the *behavior* (entropy minimization) but not the math
- Section 4: VSA / HDC 10,000D vectors → the *properties* (one-shot, no forgetting, pointer) are achieved via counting; the math is unbuilt
- Section 1.2: Curry-Howard → entirely theoretical target
- Section 9.3.1: Vector search + Cypher tools → design documented, not started

The system does NOT need VSA math to have VSA properties. The question is whether you need
the math later for scale. At 1,025 concepts / 1,601 relations, counting is adequate.
At 1M+ concepts, you'd need the math for efficient similarity queries.

---

## To create these as real GitHub issues

```bash
# Create labels first
gh label create "proven-unported" --color "e4e669" --description "Proven in Thread 1 (autogenesis), not ported to Thread 2 (canonical Rust)"
gh label create "port-from-thread1" --color "0075ca" --description "Port mechanism from run_all_epochs.py to src/canonical/"
gh label create "thread2-canonical" --color "d93f0b" --description "Canonical Rust OVM changes"
gh label create "governance" --color "cfd3d7" --description "Policy/governance/ threshold changes"
gh label create "hardcoded" --color "e4e669" --description "Hardcoded value to make configurable"

# Then create each issue from the sections above
gh issue create --title "[P0] Port no_evidence_at_declaration to canonical invariants.rs" \
  --label "proven-unported,thread2-canonical" \
  --body "$(sed -n '/^### Issue 1/,/^### Issue 2/p' GITHUB_ISSUES.md | head -n -1)"
```
