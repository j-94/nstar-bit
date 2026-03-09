# Taxonomy Grounding Audit — v2 Delta
# What changed between the original audit and the full history scan (2026-03-09)
# Read this alongside taxonomy_grounding_audit.md.resolved (the base document)
# Source: FINDINGS.md, STATUS.md, thread_summary_2026-03-08.md, epoch73_fork.json, nstar_canonical_state.json

---

## What the Original Audit Missed

The original audit (`taxonomy_grounding_audit.md.resolved`) was written from `src/` only.
This delta documents everything the full history scan changed or added.

---

## Section 0 — The Real Purpose (Missing Entirely From Original)

> [!IMPORTANT]
> The original audit had no Section 0. The purpose of the system was mischaracterized as a "developer tool" or "epistemic scoring system." The real purpose surfaced in `thread_summary_2026-03-08.md`, Phase 5.

**What the system actually is:**

A **cognitive externalisation engine**. Not a developer tool.

It maps the operator's thinking patterns as a live graph. Not what they said — what *structure* they keep returning to. The manifest is not documentation; it is the distilled, executable, transferable encoding of how you think. Feed it any signal — a conversation, an idea, a decision — and it updates the graph, writes an immutable receipt, and the new state is the new best-understanding.

The six primitives the engine discovered that describe this:

| Primitive | What it is |
|-----------|-----------|
| **Signal** | Any input — idea, conversation, decision, event |
| **Pattern** | Recurring structure across signals — what you keep doing |
| **Receipt** | Immutable record — what happened, when, why |
| **Void** | Gap between what you know and what you need |
| **Manifest** | Live best-understanding — executable, transferable |
| **Turn** | One unit: signal in → pattern updated → receipt written |

**This reframes the entire taxonomy.** The theoretical layers (Active Inference, VSA, Curry-Howard) are not just "aspiration" — they are formal descriptions of what the system is trying to *become* as a cognitive externalisation engine. Section 1 is not decoration; it is the target specification for a system that maps thinking patterns at scale.

---

## Section 1 — Updates

### 1.1.2 Surprise / Entropy Minimization
**Original:** 🔴 ASPIRATIONAL — "negentropy_gate exists as concept in graph"
**Updated:** 🔵 CONCEPT — upgraded because:
- `negentropy_gate` is actively used as the **adoption criterion** in the autogenesis engine (epoch 73)
- `void_score = 1000/(evidence+1)` operationalizes "surprise" as inverse evidence count
- The engine adopted this as a survival criterion, not just a named concept
- Still not formal FEP math, but it is a live operational mechanism in Thread 1

### 1.1 Active Inference verdict — new nuance
The original verdict ("retroactive interpretation") is correct. But the framing matters:
- The system achieves *behavioral* surprise minimization via `no_evidence_at_declaration`
- `inflation_score = 0.008` at turn 1,073 proves near-zero confidence inflation
- This IS entropy minimization in practice, achieved through a gate rather than variational inference
- The gap is formal math, not functional behavior

---

## Section 2 — Major Updates

### 2.0 The Grounding Problem (NEW — not in original)
🟢 **LIVE** — This is a *proven finding*, not a taxonomy node.

Every structural fix fails in one of two ways:
- **Grounding starvation** — strict rules kill structurally necessary nodes
- **Adversarial equivalence** — any exemption becomes the attack route

7 approaches tried and rejected: binary_origin_gate, grounding_ratio, structural_dependency_floor, semantic_utility_probe (v1), alias_deduplication_layer (v1), commitment_chain (cryptographic).

**Implication for the taxonomy:** Sections 2.2 (Conformal Prediction) and 2.3 (Semantic Anchor) must account for this. Any structural grounding mechanism will fail. The gate that works is behavioral (`no_evidence_at_declaration`) not structural.

### 2.0b Delusion Delta (NEW — not in original)
🔵 **CONCEPT** — Named in `Building Epistemic API.md`. The measurable gap between declared confidence and evidence-backed confidence.

**Proven on real data:** `episteme/graph/belief_graph.json` has 8 medical beliefs with `mean_declared_confidence=0.871`, `mean_computed_confidence=0.831`, delusion_delta=0.040, total_harm=$1.678T.

The system's current inflation_score=0.008 means it operates with near-zero delusion_delta.

### 2.3.0a Support Set (NEW — PROVEN-UNPORTED)
⚪ **PROVEN-UNPORTED** — The epistemic core of Thread 1.

Relations enter at `confidence=0.0`. Only dialogue-originated evidence in `support_set` can raise confidence. When evidence is withdrawn, confidence decays automatically. Held 73 epochs. **0 unsupported_confident relations** at turn 1,073.

**Not in canonical Rust.** `GraphEdge` has `c11/c10/c01/c00` counts but no `support_set` field. The canonical engine cannot decay confidence when evidence is withdrawn.

### 2.3.1 Mitigating Entropic Drift — evidence strengthened
**Original:** ⚪ PROVEN-UNPORTED — "40+ epochs"
**Updated:** ⚪ PROVEN-UNPORTED — **73 epochs, 1,073 turns, inflation_score=0.008**

The evidence is dramatically stronger. The gate has **never been violated in production.** This is not a 40-epoch prototype — it is a proven, durable mechanism at significant scale.

### The gap is bigger than the original audit stated
The original audit said: "`has_ovm_write` is too permissive."
`STATUS.md` is more precise:

> **The fix:** separate "LM read graph context" from "LM emitted an OVM op". Require **both**.

Currently, any OVM op counts as evidence. The LM can author a scoring rule with no graph context at all. The canonical engine will Commit it. This is the highest-priority unimplemented finding.

---

## Section 5 — Major Update

### 5.1.2 Evolutionary Fitness — deeper than original stated
**Original:** 🟢 LIVE — "M1 proved the rule evolves (E0→E1→E2 trajectory)"
**Updated:** 🟢 LIVE — but the *interpretation* was shallow.

The full rule trajectory:
```
E0 (frozen):  (c11*c11) / ((c11+c10)*(c11+c01)+1)           ← no time variable
E1 (LM):      (c11*c11) / (c11 + 2*(c10+c01) + 1/(c11+1))  ← denominator restructured
E2 (LM):      (c11/(c11+3*(c10+c01)+1))*(log(c11+1)/log(t+10)) ← added t-dependence
```

When the E1 rule was **attacked on large-t behavior**, the LM independently invented time-dependence and added `log(t+10)`. The frozen rule had no `t` at all. The trajectory is not random — each mutation was driven by a specific attack and responded correctly.

**This is the crown jewel:** The system can evolve its own evaluation criteria in response to challenges it didn't anticipate. The frozen rule P@10=0.000. The LM rule P@10=0.100.

### 5.3 Governance As Data (NEW — not in original)
⚪ **PROVEN-UNPORTED** — Three sub-mechanisms fully implemented in Thread 1 (autogenesis), not in Thread 2 (canonical Rust):

1. **Health signals in receipts** — `RunComparisonReceipt` carries 6 health fields; `portfolio.json` = per-epoch receipt registry (complete since epoch 62)
2. **Versioned governance thresholds** — `epoch_logs/policy.json`; every portfolio entry carries `policy_version` (complete since epoch 62)
3. **Deterministic criterion gate** — `evaluate_criterion()` DSL path; adoption blocked if criterion_met==False; LM cannot rationalize past a failed structured gate (complete since epoch 62)

The canonical Rust engine hardcodes its thresholds in `CanonicalCriteria::default()`.

---

## Section 8 — Critical Omission Fixed

### 8.5 N★ Bit Own Measurements (ADDED in TAXONOMY.yaml, missing from original audit)
🟢 **LIVE** — The original audit noted this entire section was external citations. That is correct for 8.1–8.4. But **8.5 was missing entirely**:

- M1: LM rule P@10=0.100 vs frozen P@10=0.000 (epoch 2)
- `no_evidence_at_declaration` held for 73 epochs, inflation_score=0.008
- Rule trajectory proved 3 principled mutations under adversarial pressure
- Live corpus run: 27,534 edges after 58 turns on 3,379 READMEs

### 8.6 Proven Failures (NEW)
🟢 **LIVE** — The negative result is itself a finding.

7 structural grounding approaches tried and rejected. The failure modes are categorized: grounding_starvation or adversarial_equivalence. This negative result narrows the search space for Section 2 mechanisms.

---

## Section 11 — Engine Discoveries (NEW — entirely absent from original)

The original audit had no section for what the engine discovered about itself. This is the largest gap.

**42 high-signal concepts** (≥10 mentions) emerged from 73 epochs that were not in any human-authored design doc:

| Concept | Mentions | What it is |
|---------|---------|-----------|
| `alias_deduplication_layer` | 58 | Semantic deduplication — necessary but currently a "black box" |
| `self_citation_firewall` | 45 | Prevents using own prior statements as evidence |
| `stratified_certainty` | 45 | Multi-tier confidence — historically self-defeated (epoch 37) |
| `causal_anchor` | 39 | Schema gap: link belief changes to specific evidence IDs |
| `cosine_similarity_gate` | 24 | **Engine independently discovered need for VSA's core operation** |
| `evidence_laundering` | 9 | Attack pattern: routing ungrounded beliefs through valid chains |
| `origin_spoofing` | 7 | Relations falsely claiming external dialogue provenance |
| `penetration_ceiling` | 7 | When audit + bypass are neutralized, component becomes ungrounded |

> [!WARNING]
> `cosine_similarity_gate` (24 mentions in engine graph) is particularly significant: **the engine independently discovered the need for cosine similarity before the human taxonomy described VSA/HDC**. The engine knows it needs this. Section 4.1.3 is ASPIRATIONAL in human design terms but CONCEPT in the engine's own knowledge graph.

**9 relation types the engine invented** (not pre-specified, emerged from 1,601 relations):
`refines` (374), `supports` (349), `tests` (320), `contradicts` (287), `depends_on` (128), `preserves` (88), `weakens` (53), `strengthens` (1), `tracks` (1)

The dominance of `tests` (320 uses) over `strengthens` (1 use) reveals the engine's epistemic posture: it probes and challenges its own beliefs far more than it reinforces them. This is exactly the behavior the grounding architecture was designed to produce.

---

## Section 12 — Graph Inventory (NEW — entirely absent from original)

The original audit assumed one graph. There are 7 distinct graph types:

| Graph | Type | Scale |
|-------|------|-------|
| epoch73_fork.json | Autogenesis (self-reflection) | 1,025 concepts, 1,601 relations, turn 1,073 |
| corpus_agent_state.json | Autogenesis (external corpus) | 1,042 concepts, 1,721 relations |
| nstar_canonical_state.json | Canonical OVM (live corpus run) | 233 nodes, **27,534 edges**, 58 turns |
| 4× M1 state files | Canonical OVM (experiment) | 4-8 nodes, 16-62 edges each |
| episteme/graph/belief_graph.json | Demo belief graph | 8 medical beliefs, $1.678T harm data |
| nstar-autogenesis/*.json | Pre-canonical era (13 snapshots) | GEN 1 schema |
| experiments/ | Suite states (8 experiments) | 4/8 passed |

**The most important finding from the graph inventory:**

> **GEN 2 (autogenesis)** has `support_set` + grounding gates.
> **GEN 3 (canonical Rust)** has `evalexpr` OVM + SHA256 receipt chain.
> **Neither has both. The system that has both does not yet exist.**

---

## The Corrected Summary Statistics

**Original audit count:**

| Status | Count | % |
|--------|-------|---|
| 🟢 LIVE | ~30 nodes | ~40% |
| 🔵 CONCEPT | ~5 nodes | ~7% |
| 🟡 PLANNED | ~8 nodes | ~11% |
| 🟠 EXTERNAL | ~10 nodes | ~13% |
| 🔴 ASPIRATIONAL | ~15 nodes | ~20% |
| ⚪ PROVEN-UNPORTED | ~4 nodes | ~5% |

**After full history scan (TAXONOMY.yaml v2):**

| Status | Count | % | Notes |
|--------|-------|---|-------|
| 🟢 LIVE | ~35 nodes | ~28% | Corpus run, engine health, proven failures added |
| 🔵 CONCEPT | ~25 nodes | ~20% | 20 engine-discovered CONCEPTs added (Section 11) |
| 🟡 PLANNED | ~10 nodes | ~8% | Dual tools upgraded from ASPIRATIONAL |
| 🟠 EXTERNAL | ~10 nodes | ~8% | Unchanged |
| 🔴 ASPIRATIONAL | ~15 nodes | ~12% | Unchanged |
| ⚪ PROVEN-UNPORTED | ~12 nodes | ~10% | 8 new PROVEN-UNPORTED items discovered |
| NEW (Sections 11-12) | ~30 nodes | ~24% | Engine discoveries + graph inventory |

**Total taxonomy nodes grew from ~72 to ~127** by grounding against full history.

---

## The Unchanged Core Verdict

The original audit's bottom line still holds:

> *"Your taxonomy is the right map for where you're going. The thread analysis is the right map for where you are. You need both."*

What changed: the gap between the two maps is now **precisely quantified**. The single most important action remains:

**Port `no_evidence_at_declaration` to `evaluate_invariants()` in `src/canonical/invariants.rs`:**

```rust
// In evaluate_invariants(), after checking has_ovm_write:
if proposal.ovm_ops.iter().any(|op| op.is_rule_authoring()) 
   && !execution_effects.iter().any(|e| matches!(e, Effect::Read { .. })) {
    violations.push("no_evidence_at_ovm_op: scored without reading graph context".into());
}
```

This one gate closes the largest gap between GEN 2 and GEN 3, costs ~20 lines of Rust, and is proven over 73 epochs.
