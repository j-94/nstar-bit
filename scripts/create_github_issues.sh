#!/bin/bash
# Creates all 10 GitHub issues from GITHUB_ISSUES.md
# Run from repo root: bash scripts/create_github_issues.sh
# Requires: gh auth login

set -e
cd "$(git rev-parse --show-toplevel)"

echo "Creating labels..."
gh label create "proven-unported"   --color "e4e669" --description "Proven in Thread 1 autogenesis, not ported to Thread 2 canonical Rust" 2>/dev/null || true
gh label create "port-from-thread1" --color "0075ca" --description "Port mechanism from run_all_epochs.py to src/canonical/" 2>/dev/null || true
gh label create "thread2-canonical" --color "d93f0b" --description "Canonical Rust OVM changes" 2>/dev/null || true
gh label create "governance"        --color "cfd3d7" --description "Policy/governance/threshold changes" 2>/dev/null || true
gh label create "hardcoded"         --color "fbca04" --description "Hardcoded value to make configurable" 2>/dev/null || true
echo "Labels done."

# ─── ISSUE 1 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P0] Port no_evidence_at_declaration gate to canonical invariants.rs" \
  --label "proven-unported,thread2-canonical" \
  --body '## The gap

Thread 1 (autogenesis) has held this gate for **73 epochs / 1,073 turns**.
- `inflation_score = 0.008` at turn 1,073
- `unsupported_confident = 0` across 1,601 relations
- Gate has never been violated in production

Thread 2 (canonical Rust) short-circuits the gate: `has_ovm_write` in
`evidence_satisfied()` at `src/canonical/invariants.rs:99` lets ANY OVM op
count as evidence. The LM can author a scoring rule with zero graph context.

Permissive lines: 104, 110, 122, 124 — all use `|| has_ovm_write`.

## Fix (~20 lines)

```rust
// After line 44 in evaluate_invariants():
let has_read_before_ovm = proposal.ovm_ops.is_empty()
    || effects.iter().any(|e| matches!(e, Effect::Read {..} | Effect::FsRead {..}));

// Replace every: || has_ovm_write
// With:          || (has_ovm_write && has_read_before_ovm)

// Add violation:
if has_ovm_write && !has_read_before_ovm {
    violations.push("no_evidence_at_ovm_op: scored without reading graph context".into());
}
```

## Evidence
- `FINDINGS.md` — "The gate works"
- `STATUS.md` — "What the 70+ autogenesis epochs contain that canonical does not"
- `epoch73_fork.json` — `inflation_score: 0.008`, `unsupported_confident: 0`

Proven since epoch 1. Durable through epoch 73. **Highest-priority unimplemented finding.**'

# ─── ISSUE 2 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P1] Add support_set to GraphEdge — enable confidence decay" \
  --label "proven-unported,thread2-canonical" \
  --body '## The gap

Thread 1: Relations enter at `confidence = 0.0`. Only dialogue-originated evidence
IDs in `support_set` raise confidence. When evidence is withdrawn, confidence decays
automatically (organic forgetting). Held 73 epochs.

Thread 2: `GraphEdge` in `src/canonical/types.rs:38-54` has `c11/c10/c01/c00` counts
but no `support_set`. Thread 2 counts co-occurrences but cannot decay confidence when
evidence is removed.

## Fix

```rust
// src/canonical/types.rs — GraphEdge struct:
pub support_set: Vec<String>,   // receipt IDs that grounded this edge
pub confidence: f32,            // 0.0 at declaration; raised only by support_set items
```

Add `confidence_decay()` that removes stale turn IDs and recomputes confidence.

## Evidence
- `Building Epistemic API.md` lines 104–139
- `epoch73_fork.json` schema — every relation has `support_set: []` and `confidence: 0.0`
- Cross-ref: `2.3.0a_support_set` in TAXONOMY.yaml'

# ─── ISSUE 3 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P1] Add success_criterion DSL to promote_policy_candidate" \
  --label "proven-unported,thread2-canonical" \
  --body '## The gap

Thread 1: before each epoch, meta-LM commits to a **falsifiable predicate**
(e.g. `"INFLATION_SCORE < 0.05 AND CONCEPT_COUNT < 950"`). Evaluated deterministically
after the epoch. If `criterion_met == False`, adoption blocked — LM cannot rationalize past it.
Active since epoch 35, held through epoch 71.

Thread 2: `evaluate_promotion()` in `src/canonical/promotion.rs:233` has no structured
predicate path. The LM can rationalize any outcome as an improvement.

## Fix

Require the LM to emit a `success_criterion` field alongside OVM ops:
```json
{
  "ovm_ops": ["DefineScoringRule ..."],
  "success_criterion": "P_AT_10 > 0.05 AND INFLATION_SCORE < 0.05"
}
```

`evaluate_promotion()` evaluates this deterministically against receipt fields before
allowing `PromotionAction::Promote`.

## Evidence
- `FINDINGS.md` — "success_criterion on proposals"
- `STATUS.md` — M1 Results
- Epoch 35–71 in `epoch_logs/thread.md`'

# ─── ISSUE 4a ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P1] Governance refactor 4a — add health fields to CanonicalReceipt" \
  --label "proven-unported,governance" \
  --body '## The gap

Thread 1: `RunComparisonReceipt` carries 6 health fields per epoch:
`inflation_score_lhs/rhs`, `unsupported_confident_lhs/rhs`, `violation_count_lhs/rhs`.
`epoch_logs/portfolio.json` is an append-only receipt registry. Complete since epoch 62.

Thread 2: `CanonicalReceipt` in `src/canonical/types.rs` has no health fields.
You cannot query receipt history to see how system health has changed over time.

## Fix

Add health snapshot to `CanonicalReceipt`:
```rust
pub inflation_score: f32,
pub unsupported_confident: u32,
pub violation_count: u32,
pub live_relations: u32,
pub mean_confidence: f32,
```

Emit from `make_receipt()` in `core.rs:428`. This is Step 1 of the governance refactor
that Thread 1 completed at epoch 62.'

# ─── ISSUE 4b ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P1] Governance refactor 4b — load CanonicalCriteria from policy.json" \
  --label "proven-unported,governance,hardcoded" \
  --body '## The gap

Thread 1: all thresholds in `epoch_logs/policy.json`, version-stamped. To change a
threshold: edit file, restart runner. Every portfolio entry carries `policy_version`.
Complete since epoch 62.

Thread 2: `CanonicalCriteria::default()` in `src/canonical/types.rs` hardcodes all
thresholds. Cannot audit which thresholds were active for a given epoch.

## Fix

Load from file:
```rust
// In main() / canonical engine startup:
let criteria = CanonicalCriteria::from_policy_json("policy.json")
    .unwrap_or_default();
// Stamp every CanonicalReceipt with criteria.version
```

## Evidence
- `FINDINGS.md` @ "Step 2: Governance parameters as data"
- `epoch_logs/policy.json` (existing file, already version-stamped)'

# ─── ISSUE 4c ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P1] Governance refactor 4c — add evaluate_criterion DSL (new criterion.rs)" \
  --label "proven-unported,governance" \
  --body '## The gap

Thread 1: `evaluate_criterion(criterion_str, receipt, state_summary)` — two-path evaluator:
- Structured DSL: `FIELD OP VALUE AND ...` (deterministic, no LM call)
- Natural language fallback: LM call with forced JSON output
Adoption blocked if `criterion_met == False`. LM cannot rationalize past a failed gate.
Complete since epoch 62.

Thread 2: No criterion DSL. evaluate_promotion() has no structured predicate path.

## Fix

New file `src/canonical/criterion.rs`:
```rust
pub fn evaluate_criterion(
    criterion: &str,
    receipt: &CanonicalReceipt,
) -> (bool, &str) {
    // Try DSL parse first: "INFLATION_SCORE < 0.05 AND P_AT_10 > 0.05"
    // Fall back to LM call only if DSL parse fails
    // Returns (criterion_met, method_used)
}
```

## Evidence
- `FINDINGS.md` @ "Step 3: Deterministic criterion gate"
- `scripts/corpus_agent.py` has the Python reference implementation'

# ─── ISSUE 5 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P2] Port semantic_utility_probe — nodes must prove routing utility" \
  --label "proven-unported,thread2-canonical" \
  --body '## The gap

Thread 1: nodes must prove **downstream routing utility** — not just structural position.
A node that is named and referenced but never actually routes a turn gets scored lower and
eventually quarantined. One of three core proven mechanisms in FINDINGS.md. Active since epoch 53+.

Thread 2: `evaluate_promotion()` in `promotion.rs` checks scoring rule candidates but does not
check whether nodes are being used as routing intermediaries. Surface-level co-occurrence
(`c11`) accumulates without utility validation.

## Evidence
- `FINDINGS.md` — "Three things were actually adopted" (item 1)
- `epoch73_fork.json` — concept `semantic_utility_probe` has 13 mentions, status=known'

# ─── ISSUE 6 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P2] Port measurement_escrow + lazy_reconstruction (ruler-can-not-measure-itself fix)" \
  --label "proven-unported,thread2-canonical" \
  --body '## Background

Epoch 37: `stratified_certainty` fell below its own survival threshold when internal signals
were removed. The ruler cannot measure itself. Two mechanisms adopted to address this:

1. **measurement_escrow**: verification tools must live *outside* the graph they measure;
   snapshot and reconstruct, do not make them immune to the same gates.
2. **lazy_reconstruction**: pay reconstruction cost on demand, not speculatively.

## The gap

Thread 2: `evaluate_promotion()` uses the **live graph state** to evaluate whether rules
should be promoted. The evaluator lives inside the same process as the graph it measures.

## Fix

`src/bin/replay.rs` already reads serialized state. Extend to support in-process snapshots
as the evaluation substrate:
```rust
let snapshot = graph.snapshot(); // serialize to bytes
let eval_result = evaluate_promotion(&CanonicalCore::from_snapshot(&snapshot), &candidate);
// evaluator reads snapshot, not live state
```

## Evidence
- `FINDINGS.md` — "The ruler cannot measure itself (epoch 37)"
- `FINDINGS.md` — "Three things were actually adopted" (items 2 and 3)'

# ─── ISSUE 7 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P2] Port deficit_scan — graph-driven corpus targeting" \
  --label "proven-unported,port-from-thread1" \
  --body '## The gap

Thread 1: instead of LM-driven epoch planning, `deficit_scan()` queries the graph for
contested relations (high violation_count), then fires targeted agents at those specific
nodes. Graph-driven, not LM-driven. Key optimization that scaled Thread 1 to 1,025 concepts.

Thread 2: `run_canonical_corpus.py` processes READMEs in sequence with no priority ordering.
Every README gets equal weight regardless of graph contention.

## Fix

Before each batch in `run_canonical_corpus.py`:
1. Query `nstar_canonical_state.json` for nodes with high `c01 + c10` relative to `c11` (contested)
2. Score READMEs by how many contested node labels they contain
3. Prioritize high-scoring READMEs for next batch

This mirrors the `deficit_scan()` logic in `run_all_epochs.py`.

## Evidence
- `FINDINGS.md` — "deficit_scan()"
- `run_all_epochs.py` — reference implementation in Python'

# ─── ISSUE 8 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P3] Execute PURGE_HARDCODED.md — strip all hardcoded control logic (~8h)" \
  --label "hardcoded,thread2-canonical" \
  --body '## Overview

`PURGE_HARDCODED.md` catalogs 13 hardcoded constants + 16 string matchers + 4 hardcoded
contradiction weights. ~8 hours of work.

## Priority items

1. `activation_cutoff: 0.4` in `CanonicalCriteria::default()` → `DefineCriteria` OVM op
2. `+0.08` edge weight delta in `graph.rs:246` → policy.json field
3. `op_risk()` scores (Shell=0.7, FsWrite=0.6) in `core.rs:721` → policy.json
4. Macro-hard gate dead field references — `DIVERGENCE.md §3` catalogs 5 vestigial Python fields in `invariants.rs`

## Pattern

Each constant becomes either:
- A field in `policy.json` (governance threshold, loaded at startup)
- A `DefineCriteria` OVM operation (LM-mutable parameter)'

# ─── ISSUE 9 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P2] Close open-loop seed queue (M2 milestone)" \
  --label "thread2-canonical" \
  --body '## The gap

The canonical engine `nstar_canonical_state.json` shows `seed_queue: []` after 58 turns.
Every turn required a human to pipe in the next README. The system is fully human-cranked.

## Fix

After each successful `Commit`, push 1–3 seed prompts to `graph.seed_queue`:
1. Query highest-void nodes (highest `1000/(c11+1)` score)
2. Query most-contested nodes (highest `c01 + c10`) 
3. Generate seeds targeting those nodes
4. Next turn consumes from queue before requesting external input

## Evidence
- `nstar_canonical_state.json`: `seed_queue: []` at turn 58
- `epoch73_fork.json`: seed_queue populated by the engine itself with 2 pending seeds
- MILESTONES.md: M2 milestone'

# ─── ISSUE 10 ─────────────────────────────────────────────────────────────────
gh issue create \
  --title "[P3] Add causal_anchor field to GraphEdge — complete provenance tracing" \
  --label "proven-unported,thread2-canonical" \
  --body '## The gap

The autogenesis engine independently discovered this schema gap **39 times** across 73 epochs
(second-highest mention count in belief graph after alias_deduplication_layer).

Currently `GraphEdge` tracks *that* co-occurrences happened (via c11/c10/c01/c00) but not
*which specific turns* contributed them. You cannot navigate from a hypothesis edge back to
the exact dialogue evidence that grounded it.

## Fix

```rust
// src/canonical/types.rs — GraphEdge:
pub causal_turns: Vec<String>,  // receipt IDs that caused c11 increments
```

When `update_hypothesis_substrate()` increments `c11`, also push the current `turn_receipt_id`
to `causal_turns`. Cap at last N=20 to prevent unbounded growth.

This completes the pointer architecture: graph edge → receipt ID → raw dialogue text.

## Evidence
- `epoch73_fork.json` concept `causal_anchor`: 39 mentions, identified as schema gap
- `Refining Graph Epistemics.md` line 149: "causal_anchor is not a feature request, it is a schema gap"
- TAXONOMY.yaml `3.3.0_causal_anchor` and `11.1.05_causal_anchor`'

echo ""
echo "Done. Run: gh issue list"
