# Purge Hardcoded Control Logic

**Ethos:** Zero hardcoded control logic. The system discovers everything from its own operation.

**Rule:** If the system can't change it, it shouldn't exist. If it must exist at bootstrap, the system must be able to override it within 5 turns.

---

## Phase 1: Make Every Constant LM-Mutable

These are parameters the LM should be able to propose changes to, the same way it proposes scoring rules today. Add them to the OVM operations vocabulary.

### 1.1 Graph Criteria → OVM-mutable
**File:** `src/canonical/types.rs` (CanonicalCriteria)

| Parameter | Current | How to make discoverable |
|---|---|---|
| `activation_cutoff: 0.4` | When a node "fires" | LM proposes via `define_activation_cutoff` |
| `propagation_steps: 2` | How far activation spreads | LM proposes via `define_propagation_steps` |
| `min_evidence_coverage: 0.7` | Gate rejection threshold | LM proposes via `define_evidence_threshold` |
| `contradiction_threshold: 0.1` | Gate rejection threshold | LM proposes via `define_contradiction_threshold` |
| `audit_rate: 0.33` | Stochastic audit probability | LM proposes via `define_audit_rate` |
| `max_risk: 0.8` | Operation risk ceiling | LM proposes via `define_max_risk` |

**Action:** Extend `OvmOp` enum with `DefineCriteria { key: String, value: f32 }`. The LM can tune any criteria value. The system starts with defaults but the LM can override them when it sees that the gate is too aggressive or too permissive.

- [ ] Add `DefineCriteria` to `OvmOp` enum
- [ ] Wire it in `core.rs` process_turn (same place as DefineScoringRule)
- [ ] Add criteria to the system prompt so the LM can see current values
- [ ] Add criteria changes to rule_trajectory logging

### 1.2 MAX_NODES cap → remove or make discoverable
**File:** `src/bin/canonical.rs:337`

- [ ] Remove `MAX_NODES = 30`. If nodes accumulate garbage, the pruning loop (Phase 2) handles it. The cap is a band-aid for a missing pruning mechanism.

### 1.3 Hardcoded numeric constants in graph.rs
**File:** `src/canonical/graph.rs`

| Constant | Line | Value | Replace with |
|---|---|---|---|
| Co-activation delta | 246 | `0.08` | Move to `CanonicalCriteria.coactivation_delta` → LM-mutable |
| Operator bleed | 455 | `score * 0.1` | Move to `CanonicalCriteria.operator_bleed` → LM-mutable |

- [ ] Add `coactivation_delta` and `operator_bleed` to `CanonicalCriteria`
- [ ] Replace hardcoded values with criteria lookups

### 1.4 Hardcoded constants in core.rs
**File:** `src/canonical/core.rs`

| Constant | Line | Value | Replace with |
|---|---|---|---|
| Session EMA decay | 366 | `0.8 / 0.2` | `CanonicalCriteria.session_decay` |
| Session cutoff multiplier | 324 | `0.8` | `CanonicalCriteria.session_cutoff_factor` |
| Project cutoff multiplier | 350 | `0.5` | `CanonicalCriteria.project_cutoff_factor` |
| Scorecard interval | 212 | every 5 turns | `CanonicalCriteria.scorecard_interval` |
| Scorecard K | 214 | 50 | `CanonicalCriteria.scorecard_k` |
| Promotion: min turns | 455 | 10 | `CanonicalCriteria.min_promotion_turns` |
| Promotion: slope threshold | 459 | -0.01 | `CanonicalCriteria.promotion_slope_threshold` |
| Contradiction history window | 428 | 100 | `CanonicalCriteria.history_window` |

- [ ] Add all to `CanonicalCriteria` with current values as defaults
- [ ] Replace hardcoded values with criteria lookups
- [ ] All become LM-mutable via `DefineCriteria`

---

## Phase 2: Replace Heuristics with LM Reasoning

### 2.1 Kill `missing_artifact_claim_is_false`
**File:** `src/lm.rs:154-183`

16 hardcoded string patterns to detect LM hallucination. This is patching a failure mode with grep.

**Replace with:** Feed the LM its own prior evaluation + the actual artifact list and ask it to self-correct. Or: just trust the structured output and drop the heuristic. If the LM said it emitted an artifact and the artifact is in the structured output, that's ground truth — no string matching needed.

- [ ] Remove the 16-pattern string matcher
- [ ] Trust `emitted_artifacts` as ground truth (it's derived from actual structured output, not LM prose)
- [ ] If self-correction is needed, do it in the evaluate_predicates prompt, not in post-hoc grep

### 2.2 Kill hardcoded contradiction weights
**File:** `src/canonical/invariants.rs:147-180`

```
assert:wrote → +0.70
assert:read  → +0.40
assert:cannot → +0.20
assert:definitely → +0.20
```

These are human-authored importance weights. The system should discover what constitutes a contradiction and how much it matters.

**Replace with:** Move contradiction weights to `CanonicalCriteria` (LM-mutable). Or better: let the LM evaluate contradiction severity as part of the reflect pass, returning a contradiction score directly instead of the Rust code computing it from signal matching.

- [ ] Move weights to `CanonicalCriteria` as initial defaults
- [ ] Make them LM-mutable via `DefineCriteria`
- [ ] Long term: replace the whole contradiction_score function with an LM call that evaluates consistency between what the model claimed and what actually happened

### 2.3 Kill hardcoded operation risk scores
**File:** `src/canonical/core.rs:606-631`

```
Shell → 0.7, FsRead → 0.2, FsWrite → 0.6, ...
```

**Replace with:** Move to `CanonicalCriteria` as a map (`operation_risks: HashMap<String, f64>`). Make LM-mutable. The system starts with defaults but learns that some operations are safe in context.

- [ ] Add `operation_risks` map to `CanonicalCriteria`
- [ ] Make LM-mutable via `DefineOperationRisk { operation: String, risk: f64 }`
- [ ] Replace hardcoded match arms with criteria lookups

### 2.4 Kill the escalation string match
**File:** `src/canonical/invariants.rs:40`

```rust
!proposal.response.to_lowercase().contains("escalat")
```

Word-presence check. Exactly what we're trying to move past.

- [ ] Remove this check entirely. The gate signal `escalate` already indicates the system thinks escalation is needed. Whether the prose mentions the word is irrelevant.

---

## Phase 3: Give the LM Actual Reasoning Tasks

### 3.1 Show the LM its own history
**Current:** The reflect prompt gets a flat list of predicate names. No trajectory, no scores, no survival data.

- [ ] Include in the reflect prompt:
  - Per-predicate activation history (last 5 turns)
  - Per-predicate reinforcement count
  - Which predicates have never fired (candidates for pruning)
  - The current scoring rule and its P@K
- [ ] Include in the evaluate prompt:
  - The predicate's historical activation rate
  - Whether it predicted held-out co-occurrence last time

### 3.2 Let the LM prune and merge
**Current:** The LM can only add predicates. Never remove, never merge.

- [ ] Extend the reflect response format:
  ```json
  {
    "prune": ["predicate_ids_to_kill"],
    "merge": [{"from": ["id1", "id2"], "into": "new_name", "condition": "..."}],
    "new_predicate": null | {...}
  }
  ```
- [ ] Wire prune in graph.rs: remove node, remove associated edges
- [ ] Wire merge: create new node, transfer edge history, remove old nodes

### 3.3 Let the LM adjust criteria
**Current:** The LM proposes scoring rules but can't touch any other parameter.

- [ ] Add current criteria values to the system prompt
- [ ] Add criteria change history (what was changed and when)
- [ ] Let the LM propose criteria changes with a reason (logged in trajectory)

### 3.4 Let the LM see the substrate
**Current:** The LM gets P@K and top hits/misses every 5 turns, but only in the action prompt. The reflect and evaluate prompts are blind.

- [ ] Include a substrate summary in all three prompts:
  - Number of hypothesis edges
  - Distribution of c11 counts
  - Which node pairs have strongest/weakest signal
  - Current scoring rule and selection predicate

### 3.5 Ask the LM to think, not just label
**Current prompts ask:** "Score each predicate 0-1" and "Propose a new predicate if needed"
**Should ask:** "Given this turn's outcome and the history of these predicates, what is your analysis of what happened and what should change?"

- [ ] Rewrite evaluate_predicates to ask for reasoning first, scores second
- [ ] Rewrite reflect to ask for analysis of the predicate space, not just "anything new?"
- [ ] Add a self-critique step: "Were your evaluations last turn accurate? What would you change?"

---

## Phase 4: Bootstrap Protocol

After purging, the system starts with:

**Fixed (infrastructure, not control logic):**
- The turn pipeline: propose → observe → discover → process → receipt
- The receipt chain format (SHA256, append-only)
- The evalexpr sandbox (variables: c11, c10, c01, c00, t, score)
- The graph data structure (nodes, edges, hypothesis edges)
- The LM client (API calls)

**Discoverable (starts at defaults, LM can change):**
- All criteria values (activation cutoff, thresholds, risk scores, etc.)
- The scoring rule and selection predicate
- Which predicates exist, which survive, which merge
- What constitutes a contradiction and how much it matters
- When to evaluate, how deep to evaluate

**Zero at start:**
- Predicates (discovered from turns)
- Hypothesis edges (built from co-activation)
- Scoring rule (proposed by LM when it has data)

The system prompt at turn 1 should say: "You have no predicates, no scoring rule, no history. Respond to the task. After this turn, you will reflect on what happened and begin building your awareness dimensions."

---

## Execution Order

| Step | What | Effort | Unblocks |
|---|---|---|---|
| 1 | Add `DefineCriteria` OvmOp, wire it | 1 hour | All criteria become LM-mutable |
| 2 | Move all constants to `CanonicalCriteria` | 2 hours | No more magic numbers in code |
| 3 | Remove MAX_NODES, string matchers, escalation grep | 30 min | Heuristic cleanup |
| 4 | Add prune/merge to reflect response | 1 hour | LM can manage predicate lifecycle |
| 5 | Enrich prompts with history + substrate data | 2 hours | LM can reason about system state |
| 6 | Rewrite prompts to ask for reasoning | 1 hour | LM used at >10% capability |
| 7 | Reset, run 20 turns, measure | 1 hour API | First valid test of the real system |

Total: ~8 hours of implementation + 1 hour of API time.

---

## The Test

After purging, run 20 turns. The system should:

1. Discover predicates without being told what a valid predicate is
2. Propose a scoring rule without being given examples
3. Adjust its own criteria when it detects problems (e.g., too many rollbacks → lower evidence threshold)
4. Prune predicates that don't predict anything
5. Produce P@K > baseline on behavioral co-occurrence

If it does all 5, the ethos is met. If it can't self-correct criteria (#3) or prune dead predicates (#4), the discovery loop is still open and needs more work.
