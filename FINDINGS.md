# Permanent Findings — nstar-bit autogenesis
*Only adopted/proven conclusions live here. Active exploration is in epoch_logs/thread.md.*
*Updated after each session. Stays under 100 lines.*

---

## What this system is
A self-evolving epistemic graph. An LM (Flash) processes turns and writes concepts/relations. A meta-LM (Sonnet) proposes fork experiments and evaluates adoption. A Rust binary enforces evidence gates. The loop runs indefinitely, forking and testing hypotheses about its own architecture.

## Proven findings (as of epoch 53, turn ~710)

### The grounding problem is unsolvable structurally
Every structural fix for "distinguish real knowledge from performed knowledge" fails in one of two ways:
- **Grounding starvation**: strict rules kill valid nodes that are structurally necessary but rarely directly evidenced
- **Adversarial equivalence**: any exemption or floor becomes the new attack route

Tried and rejected: binary origin gate, grounding_ratio, structural_dependency_floor, semantic_utility_probe, alias_deduplication_layer, commitment_chain (cryptographic). All failed. The graph proved grounding is a semantic problem, not a structural one.

### The ruler can't measure itself (epoch 37)
`stratified_certainty` — the mechanism the graph uses to decide what survives — fell below its own survival threshold when internal signals were removed. A self-contained epistemic system cannot fully verify its own foundations. External input is not optional; it's a survival dependency.

### Three things were actually adopted
1. **`semantic_utility_probe`** — nodes must prove downstream routing utility, not just structural position
2. **`measurement_escrow`** — verification tools must live outside the graph they measure; snapshot and reconstruct, don't make them immune
3. **`lazy_reconstruction`** — pay reconstruction cost on demand, not speculatively

These three together: *evaluate on utility, verify externally, reconstruct lazily.*

### The gate works
`no_evidence_at_declaration_or_missing_dialogue` — prevents confidence inflation. Relations enter at 0.0, only dialogue-originated evidence can raise them. This has held for 40+ epochs without failing. Inflation score stays near 0.

## Architectural changes that fixed real bugs
- **`success_criterion` on proposals** — meta-LM commits to falsifiable predicate before epoch runs; `should_adopt` evaluated against it, not vague judgement
- **`deficit_scan()`** — targeted agents hit contested relations before generic planning; graph-driven not LM-driven
- **Epoch counter** — starting from `epochN_fork.json` correctly starts at N+1
- **Fallback fork/plan** — if meta-LM returns empty JSON, copies current state as fork and retries
- **Deficit agent timeout** — 90s hard limit prevents subprocess hangs
- **Meta model = Sonnet** via OpenRouter (`ROUTER_META_MODEL=anthropic/claude-sonnet-4-6`)

## Architecture refactor (Steps 1–3 complete — epoch 62)

### Step 1: Health signals in receipts
- `RunComparisonReceipt` now carries 6 health fields: `inflation_score_lhs/rhs`, `unsupported_confident_lhs/rhs`, `violation_count_lhs/rhs`
- `compare_states()` calls `health_check()` on both states
- `epoch_logs/portfolio.json`: append-only receipt registry per epoch

### Step 2: Governance parameters as data
- `epoch_logs/policy.json` — all thresholds in one file, version-stamped
- Runner loads at startup: `[policy] loaded v1 from epoch_logs/policy.json`
- Every portfolio entry carries `policy_version` — thresholds are auditable per epoch
- To change a threshold: edit `policy.json`, restart runner

### Step 3: Deterministic criterion gate
- `evaluate_criterion(criterion, receipt, state_summary)` — two-path evaluator
  - Structured DSL: `FIELD OP VALUE AND ...` (no LM call, deterministic)
  - Natural language fallback: LM call with receipt data, forced JSON output
- Adoption is blocked if `criterion_met == False` — LM cannot rationalize past a failed gate
- `criterion_met` + `criterion_method` written to every portfolio entry
- LM system prompt updated to request structured predicates over named receipt fields

## M1 Results — LM-authored scoring rules vs frozen baseline (2026-03-08)

**Canonical Rust OVM unblocked** (B1/B2/B3 fixed). Two full adversarial epochs ran.

**Epoch 1 (18 turns):** LM hypothesis precision 1.00 vs baseline 0.80. Held-out P@10 tied at 0.200 — corpus too small to separate rules.

**Epoch 2 (18 turns, attacking evolved rule):** Frozen rule degraded to P@10=0.000. LM rules held at P@10=0.100. **Divergence confirmed at scale.**

**Rule trajectory:**
- Frozen: `(c11*c11) / ((c11+c10)*(c11+c01)+1)` — no t-dependence
- LM e1: `(c11*c11) / (c11+2*(c10+c01)+1/(c11+1))` — denominator restructured
- LM e2: `(c11/(c11+3*(c10+c01)+1)) * (log(c11+1)/log(t+10))` — added t-dependence when challenged on large-t behavior

Each mutation was driven by a specific adversarial attack. The trajectory is not random; the LM responded to the attacks correctly.

**Missing from canonical:** `no_evidence_at_declaration_or_missing_dialogue` (proven over 40+ autogenesis epochs) was never ported. The canonical gate fires on behavioral node patterns; the LM can Commit a scoring rule without any grounding evidence from the graph. This is the highest-value unimplemented finding.

## Autonomous run findings — supervisor_daemon (2026-03-10)

Three independent supervisor runs (turns 1075→2176, 408+361+328 turns fired) converged to identical late-stage seeds. Replicated without human turns.

### ghost_core and shadow_mutation (system-named, autonomously discovered)
The engine named its own pre-fix failure mode across all three runs:
- **`ghost_core`**: ungrounded high-frequency concepts — declared without dialogue evidence, accumulating mention weight through repetition alone. The 649 `candidate` concepts in the final graph are ghost_core candidates.
- **`shadow_mutation`**: the process by which ghost_core concepts gradually displace `foundational_axiom` nodes through weight accumulation, without ever passing the evidence gate. Silent inflation that doesn't trigger the firewall because it operates below the declaration threshold.

Final seed (all three runs): *"If shadow_mutation bypasses the self_citation_firewall, does stratified_certainty collapse into a flat graph?"* — the system asked whether its own confidence hierarchy survives the attack it just named.

Detection mechanism proposed by the system: SHA-256 receipt comparison before/after shadow_mutation activity. The receipt chain it already has.

### Gate bypass bugs found and fixed (this session)
1. **`source_uri` silent drop** — every evidence entry had `source_uri: ""`. Raw signal never stored.
2. **`origin="dialogue"` never set** — gate checked `origin=="dialogue"` but LM path wrote `origin=""`. 0/1293 evidence entries had dialogue origin. Gate was a broken clock.
3. **Concept-only turn bypass** — gate looped over `relation_ids`. Empty loop → `missing_evidence=false` → noise concepts landed as `known`. Engine minted a concept called `known` and marked it `known`.
4. **Gate fired after write** — concepts upserted before evidence check. Gate set `allow_act=false` but 170 noise concepts already committed as `known`.

All four closed. Post-fix concept rate: 0.50/turn vs 5.67/turn pre-fix. Gate split at halt: 1028 `known` / 649 `candidate`.

## What the system is currently working on (epoch 60)
Testing `sustained_presence_requirement` with W=3 windows (up from W=2). Deficit agents targeting `window_straddling` edge case — nodes with activity spanning turn X.3→(X+1).1 that get dropped. Epoch 59 specced the W=3 boundary formally. Epoch 60 probing for blind spots.

## Known failure modes to watch
- **OpenRouter 402**: credits depleted, Flash turns fail silently, runner aborts
- **`missing_gate`** on last task of epoch: LM drops gate object, next epoch generates 0 tasks
- **Deficit agents hitting same relations every epoch**: relation is genuinely unresolvable, needs manual archival
- **Opus/Sonnet returning empty fork JSON**: fork file not created, epoch aborts (fallback now handles this)

## Resume
```bash
# Check runner
ps aux | grep run_all_epochs

# Start (use latest epoch N from ls epoch_logs/epoch*_fork.json)
ROUTER_META_MODEL="anthropic/claude-sonnet-4-6" nohup python3 run_all_epochs.py --state epoch_logs/epochN_fork.json > epoch_logs/runner.log 2>&1 &

# Monitor
tail -f epoch_logs/runner.log
```


ARCH [58]: In the sustained_presence_requirement window config, change the 2-turn sub-window to 3 turns — because the open tension identifies that 2-turn boundaries cause false rejections for legitimate low-frequency signals delayed by dependency_weighted_quarantine, and the next-epoch seed explicitly calls for testing 3-turn windows — expected: fewer false quarantine rejections for valid slow-arriving nodes without materially loosening the gate.
