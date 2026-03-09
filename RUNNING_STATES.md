# Running States Catalog

**Last updated:** 2026-03-07
**Purpose:** Orient across sessions. Each state is independent. Each has a resume command.

---

## State 1: nstar-autogenesis engine (ACTIVE — furthest along)

**System:** Python `engine.py` + `target/debug/autogenesis` Rust binary
**Driver:** `run_all_epochs.py`
**Current state file:** `epoch_logs/epoch9_fork.json`

### Where it is
- Turn **212**, epoch **9** in progress (2 of ~12 tasks completed)
- Graph: **227 concepts**, **364 relations**, 0 evidence entries
- Epoch 9 focus: `architecture, routing_ledger, topological_inference`
- Epoch 9 tension: routing_ledger claims are "inferred from topology" — not yet empirically grounded

### What has run
| Epoch | Turns | Topic | Verdict |
|-------|-------|-------|---------|
| 1 | 1–50 | Graph self-organization, scoring rule evolution | Rule mutated 7x, gate discriminates |
| 2 | 50–62 | Pruning impact audit | ~6.2% load-bearing floor, assumption_hardening pathology identified |
| 3 | 62–74 | Stress tests, belief revision | Graph memory is lossy, alias resolution compromised |
| 4 | 74–86 | Structural liquidity | Over-liquid, introduced causal_anchor |
| 5 | 86–? | (in epoch_logs) | see epoch5_summary.tsv |
| 6 | ?–198 | Information closure, explanatory autonomy | IC = valid formal tool, not a solution |
| 7 | ~198 | (partial, see epoch7_summary.tsv) | |
| 8 | 198–209 | Provenance paradox, routing survival score | Paradox confirmed as empirical reality |
| 9 | 209–212 | Routing ledger, topological inference | In progress — 2 tasks done |

### Resume
```bash
python3 run_all_epochs.py --state epoch_logs/epoch9_fork.json --resume
```

### Checkpoints available
All fork files in `epoch_logs/` — can resume from any prior epoch:
```
epoch_logs/epoch1_fork.json   (turn ~50)
epoch_logs/epoch2_fork.json   (turn ~62)
...
epoch_logs/epoch9_fork.json   (turn 212, current)
```

---

## State 2: Canonical Rust OVM (STALLED — blockers unresolved)

**System:** `src/canonical/` Rust core, `src/bin/canonical.rs`
**State file:** `nstar_canonical_state.json`
**Receipts:** `canonical_receipts.jsonl` (16 receipts, last 5+ all Rollback)

### Where it is
- Epoch 1 ran 18 tasks, produced 18 receipts — **but mostly rollbacks**
- `epoch1_summary.tsv` (root): 1 Commit + 17 Rollbacks. Gate broken.
- Last receipt: turn 16, decision=Rollback, cov=0.00, con=0.70
- Rule last seen: `(c11 / (c11 + c10 + 1)) * log(t+1) / ...`
- Graph: 2 behavioral nodes, 1 hypothesis edge

### Blockers (from CHECKLIST.md)
- **B1** — `null` f32 crash: `GraphNode.activation/threshold` not `Option<f32>`, LM can brick state permanently. Fix: add `#[serde(default)]` in `types.rs`.
- **B2** — Gate rejects everything: `evidence_coverage=0.00` + `contradiction_score=0.70` on every turn. Root: OVM ops not counted as write effects. Fix: count `DefineScoringRule`/`DefineSelectionPredicate` as satisfying `assert:wrote` in `invariants.rs`.
- **B3** — Rate-limit thrash: no backoff on 429/503, 58 rate errors for 20 successful turns, 287 MB log. Fix: exponential backoff in `lm.rs::chat_raw`.

### Resume (after fixing blockers)
```bash
# 1. Fix B1, B2, B3 (see CHECKLIST.md Phase 0)
cargo build --release
./target/release/canonical --reset
# smoke test 3 turns manually, verify all Commit
# then run:
./target/release/canonical < epoch_tasks_adversarial.txt
```

### State to compare against
- `baseline_receipts.jsonl` + `baseline_state.json` — frozen baseline run (epoch pre-fixes)
- `rule_trajectory.jsonl` — mutation history

---

## State 3: nstar-autogenesis subdir (OLDER — 3 fork snapshots)

**System:** `nstar-autogenesis/engine.py` (original Python engine, own git repo)
**Location:** `nstar-autogenesis/`

### Where it is
Three fork snapshots saved here from an earlier parallel run:
- `nstar-autogenesis/epoch2_fork.json` — earlier epoch 2 state
- `nstar-autogenesis/epoch3_fork.json`
- `nstar-autogenesis/epoch4_fork.json`

Also has `lm_authoritative_state.json` (485k), `lm_review_50_consequence_fork.json` (1.2M) — named snapshots from interactive review sessions.

### Resume
```bash
cd nstar-autogenesis
python3 engine.py --state epoch4_fork.json
```
Note: this is a diverged branch from the main autogenesis run in `epoch_logs/`. The canonical chain is `epoch_logs/epoch9_fork.json`. Unless you need to re-examine an older decision, use State 1.

---

## State 4: Architecture planning threads (READING MATERIAL — not runnable)

These are exported chat conversations representing direction decisions made in other sessions. Read these to understand why State 1 went where it did.

| File | Topic | Key decision |
|------|-------|-------------|
| `Building Epistemic API.md` | Medical audit demo, "epistemic substrate" framing | System is "missing persistence layer for AI reasoning", not just an audit tool |
| `Refining Graph Epistemics.md` | Sunset schedule, confidence decay, governance | First decay cycle live; audit_resolves_gap flagged as circular metric |
| `Declarative Architecture Refinement.md` | Full architecture rethink | (read for context) |
| `cursor_casual_greeting.md` | Large mixed session | Epoch 1-2 summary, lessons from early runs |

---

## Priority Map

| Priority | State | Action |
|----------|-------|--------|
| 1 | State 1 (autogenesis) | Resume `epoch9_fork.json` — most data, most progress |
| 2 | State 2 (canonical Rust) | Fix B1/B2/B3, run 20 turns on behavioral substrate, get P@K number |
| 3 | State 3 (nstar-autogenesis subdir) | Reference only — don't run unless debugging old decisions |

---

## What We're Actually Trying to Answer

From MILESTONES.md M1 (the hard question):
> Does the LM-authored scoring rule outperform a frozen baseline? Does it evolve under adversarial pressure?

**State 1** is generating the multi-epoch trajectory but using a different engine (autogenesis, not canonical Rust).
**State 2** is the canonical Rust OVM that's supposed to answer M1 — but is stalled.

The gap: State 1 has deep semantic graph data. State 2 has the rigorous OVM/scoring rule machinery. They haven't been compared yet. Once B1/B2/B3 are fixed in State 2, the next step is running State 2 against the same adversarial task sets and comparing rule trajectory.

---

## File Map (quick reference)

```
epoch_logs/              — State 1 checkpoints (epochs 1–9)
nstar_canonical_state.json — State 2 live graph
canonical_receipts.jsonl — State 2 receipt chain
rule_trajectory.jsonl    — State 2 rule mutation log
nstar-autogenesis/       — State 3 (older)
CHECKLIST.md             — State 2 fix plan
MILESTONES.md            — Overall goal structure
PURGE_HARDCODED.md       — Next architectural step (after blockers fixed)
scripts/                 — epoch_metrics.py, heldout_eval.py, eval_rule.py
```
