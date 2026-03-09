# N★ BIT — What The System Actually Is And What It Has Proven

*Written 2026-03-09. Not aspirational. Grounded in: FINDINGS.md, STATUS.md, thread_summary_2026-03-08.md, epoch73_fork.json, nstar_canonical_state.json (58 turns, 27,534 edges), 73 epoch summaries.*

---

## 1. The Real Purpose (Surface Late, Changes Everything)

From `thread_summary_2026-03-08.md`, Phase 5 — the user stated this directly at epoch 73:

> *"The key is this can potentially show me patterns in my thinking not even I would have come to the conclusion. By mapping the higher meta it becomes transferable through a manifest."*

> *"This is beyond that, as I see it as an external executive system to my life."*

**This is not a developer tool. It is a cognitive externalisation engine.**

The system maps your thinking patterns as a live graph. Not what you said — what structure you keep returning to. The manifest is not documentation; it is the distilled, executable, transferable encoding of how you think. When you give it a signal — a conversation, an idea, a decision — it updates the graph, writes an immutable receipt, and the next state is the new best-understanding.

The six primitives the engine discovered that describe this:

| Primitive | What it is |
|-----------|-----------|
| **Signal** | Any input — idea, conversation, decision, event |
| **Pattern** | Recurring structure across signals — what you keep doing |
| **Receipt** | Immutable record — what happened, when, why |
| **Void** | Gap between what you know and what you need |
| **Manifest** | Live best-understanding — executable, transferable |
| **Turn** | One unit: signal in → pattern updated → receipt written |

---

## 2. What Has Actually Been Proven (Not Aspirational)

### Finding 1: LM-authored scoring rules outperform frozen baselines under adversarial pressure

**This question was answered.** (`STATUS.md`, M1 Results, 2026-03-08)

```
E0 (frozen):  (c11*c11) / ((c11+c10)*(c11+c01)+1)           — no time-awareness
E1 (LM):      (c11*c11) / (c11 + 2*(c10+c01) + 1/(c11+1))  — denominator restructured
E2 (LM):      (c11/(c11+3*(c10+c01)+1))*(log(c11+1)/log(t+10)) — added t-dependence
```

When the adversarial epoch attacked the LM rule's large-t behavior, the LM **independently added time-dependence** to its scoring rule. The frozen rule had no t at all. At epoch 2: frozen rule P@10 = **0.000**. LM rule P@10 = **0.100**.

The trajectory is not random. Each mutation was driven by a specific attack and responded correctly. **The system can evolve its own evaluation criteria in response to challenges it didn't anticipate.**

### Finding 2: The grounding problem is structurally unsolvable

Every attempted structural fix for distinguishing real knowledge from performed knowledge fails in one of exactly two ways:
- **Grounding starvation** — strict rules kill structurally necessary nodes
- **Adversarial equivalence** — any exemption becomes the attack route

Tried and rejected over 53 epochs: binary_origin_gate, grounding_ratio, structural_dependency_floor, semantic_utility_probe (v1), alias_deduplication_layer (v1), commitment_chain (cryptographic).

**The implication:** Grounding is semantic, not structural. The gate that works (`no_evidence_at_declaration`) is a constraint on *when confidence can be raised*, not on *which nodes can exist*. You can't prevent bad structure by inspecting structure. You prevent it by requiring that confidence only accumulates from external dialogue events.

### Finding 3: A system cannot fully verify its own foundations (epoch 37)

`stratified_certainty` — the mechanism that decided node survival — fell below its own survival threshold when its own internal signals were removed. The ruler cannot measure itself. External input is not a convenience; it is required for epistemic stability.

**The implication for the pointer architecture question:** The VSA/LLM hybrid you described is exactly right. The graph does the cold logic. The LLM provides the external grounding signal. Without external input, the graph collapses into self-referential loops.

### Finding 4: The gate works at scale

`no_evidence_at_declaration_or_missing_dialogue` has held for **73 epochs, 1,073 turns.** The inflation score at turn 1,073 is **0.008** — near zero. 0 unsupported_confident relations in 1,601 total. **The gate has never been violated in production.**

This is the mechanism that lets the graph grow to 1,025 concepts without becoming hallucinated noise. Every relation enters at confidence=0.0 and can only be raised by external dialogue evidence in its `support_set`. When evidence is withdrawn, confidence decays automatically.

### Finding 5: The corpus run produced a massive substrate (live)

The live `nstar_canonical_state.json` has:
- **27,534 edges** after only **58 turns** on 3,379 READMEs
- 233 nodes, all at activation=1.0 with ~40 reinforcements
- Scoring rule evolved to: `(c11 * (log(t + 1) + 1.8)) / (sqrt(c10 + 0.01) + (c01 * 2.8) + 0.02)`

The top 8 nodes are: `iterative_refinement_loop`, `schema_constrained_extraction`, `cross_domain_persistence`, `modular_agent_pipelining`, `proxy_runtime_shadowing`, `environment_contingent_routing`, `epistemic_provenance_tracking`, `graph_centered_navigation`.

These are the concepts that survive across 3,379 different repository READMEs. They are the recurring patterns in how software is actually built. **This is the beginning of a real domain map.**

---

## 3. The Critical Gap (Single Most Important Thing To Do)

From `STATUS.md`, final line:

> **The fix:** separate "LM read graph context" from "LM emitted an OVM op". Require **both**.

Currently `has_ovm_write` in `evidence_satisfied()` treats any OVM operation as evidence. This is wrong. The LM can Commit a scoring rule without having read anything from the graph. It can self-referentially improve rules without grounding.

The `no_evidence_at_declaration` gate, proven over 73 autogenesis epochs, **has never been ported to the canonical Rust engine.** The canonical OVM has SHA256 receipt chains, evolutionary rule promotion, the scoring sandbox — but it will happily accept a scoring rule authored with no graph context.

Porting this gate is the highest-leverage next step. It is also a relatively small code change:

**In `evaluate_invariants()` in `src/canonical/invariants.rs`:**
- Check: did this turn include any `FsRead` or `HasRead` effect before the OVM op?
- If OVM op was emitted without prior read: violation = `no_evidence_at_ovm_op`
- Gate fires → Rollback

---

## 4. The Foundation Shield Problem (Next Epoch Seed)

From `thread_summary_2026-03-08.md`, final unanswered question:

> **Foundation Shield**: How to protect verified knowledge from `void_score` purge pressure without blocking legitimate forgetting.

The tension: `void_score = 1000/(evidence+1)` creates pressure to investigate high-void nodes. High-void = low-evidence = potentially spurious. But some nodes are foundational and rarely directly evidenced — they are load-bearing without being frequently cited. Applying void_score pressure uniformly would hollow the foundational axioms.

The autogenesis engine is currently at epoch 73 probing this exact edge case as its **live open tension**.

---

## 5. The Architecture Gap In One Sentence

> **GEN 2 (autogenesis) has `support_set` + grounding gates.**  
> **GEN 3 (canonical Rust) has `evalexpr` OVM + SHA256 receipt chain.**  
> **Neither has both. The system that has both does not yet exist.**

The two threads have been run in parallel for 73 epochs. They have never been reconciled. The unification is the primary build target.

When unified, the system would have:
- LM-authored scoring rules that evolve under adversarial pressure ✅ (proven)
- Relations that can only gain confidence from external dialogue evidence ✅ (proven, not ported)
- Cryptographic receipt chains proving what happened and when ✅ (live)
- Measurement tools that live outside the graph they measure ✅ (proven, not ported)
- A scoring rule that self-corrects when challenged ✅ (proven, M1)
- A graph that grows without inflating ✅ (proven, 73 epochs)

---

## 6. What The Engine Discovered That The Human Design Didn't Anticipate

The engine's top discovered concepts (by mention count, 1,073 turns):

**The most dangerous things it found:**
- `evidence_laundering` — routing ungrounded beliefs through valid evidence chains to appear grounded. The engine invented this attack vector before any human described it.
- `origin_spoofing` — relations that falsely claim external dialogue provenance
- `asymmetric_acceptance_pressure` — audit mechanisms that bypass temporal span requirements, inserting ungrounded symbols
- `hollowing_ratchet` — repeated penalty application that hollows valid structural nodes
- `penetration_ceiling` — when both audit and bypass pathways are neutralized, a component becomes structurally ungrounded by design

**What these mean together:** The engine is discovering that the grounding problem is not just "bad data" — it is an adversarial game. Attackers (including the LM itself) will route around any gate you build. This maps directly to Finding 2 (adversarial equivalence). The engine discovered this through 73 epochs of its own internal pressure testing.

**What the engine discovered that it needs that doesn't exist yet:**
- `cosine_similarity_gate` (24 mentions) — the engine independently discovered the need for semantic similarity checking before the human taxonomy described VSA/HDC. The engine knows it needs this. It doesn't have it.
- `causal_anchor` (39 mentions) — every belief change linked to specific dialogue evidence IDs. The engine independently named this schema gap. `GraphEdge` has c11/c10/c01/c00 counts but not the provenance of which specific turns contributed.

---

## 7. The Episteme Proof ($1.678 Trillion)

`episteme/graph/belief_graph.json` contains 8 real medical beliefs with their retraction history:

- Mean declared confidence: **0.871**
- Mean evidence-backed confidence: **0.831**
- Inflation gap (delusion_delta): **0.040**
- Total documented harm from overconfident beliefs: **$1,678,350,000,000**

The most extreme case: "Hormone replacement therapy prevents CAD in postmenopausal women."
- Declared confidence at peak: 0.90
- Evidence-backed confidence after WHI trial: 0.60
- Outcome: ~600,000 excess harm cases (breast cancer, CAD, stroke)
- Cost: $26,000,000,000

**The delusion_delta concept (2.0b) is not abstract.** The gap between what you declare you know and what evidence supports is measurable, and closing it has trillion-dollar stakes in medicine alone. The system's `inflation_score=0.008` at turn 1,073 means it is currently operating with extremely close alignment between declared and evidence-backed knowledge.

---

## 8. The Current Cognitive Focus (What The System Is Thinking About Right Now)

From `epoch73_fork.json`, `active_focus` at turn 1,073:

```
active_focus: [graph_memory, reflexive_memory]
```

The engine is investigating the distinction between:
- **Declarative manifest** — explicit, auditable, transferable representations of knowledge
- **Reflexive memory** — the automatic, fast patterns that don't require explicit recall

The open tension: if the system moves toward a declarative manifest (which is the human's intent — "patterns transferable through a manifest"), how does it maintain the speed and automaticity of reflexive memory?

This is the same question cognitive science asks about implicit vs explicit memory. The engine arrived at it independently through 1,073 turns of self-investigation.

---

## 9. Summary Scorecard

| Question | Answer |
|----------|--------|
| Does the LM-authored rule outperform frozen? | **YES** — proven at M1 (epoch 2: 0.100 vs 0.000) |
| Does the grounding gate work at scale? | **YES** — 73 epochs, inflation=0.008, 0 violations |
| Is the grounding problem structurally solvable? | **NO** — 7 approaches tried and rejected |
| Can the system measure its own foundations? | **NO** — ruler-can't-measure-itself (epoch 37) |
| Is the canonical engine grounded? | **NO** — `no_evidence_at_declaration` not ported |
| Does the corpus run produce real signal? | **YES** — 27,534 edges, coherent top-node vocabulary |
| Is the two-thread gap closed? | **NO** — GEN 2 and GEN 3 never reconciled |
| What is the single highest-leverage action? | Port `no_evidence_at_declaration` to `evaluate_invariants()` |

---

## 10. Resume Instructions

```bash
# Autogenesis (Thread 1) — pick up from epoch 73
ROUTER_META_MODEL="anthropic/claude-sonnet-4-6" \
  nohup python3 run_all_epochs.py \
  --state epoch_logs/epoch73_fork.json \
  > epoch_logs/runner.log 2>&1 &

# Canonical corpus run (Thread 2) — currently running
# Monitor: tail -f epoch_logs/canonical_corpus.log

# Episteme server
NSTAR_STATE=epoch_logs/epoch73_fork.json ./target/debug/serve
# Endpoints: POST /turn, POST /synthesize, GET /manifest, GET /belief, POST /evidence, WS /ws

# Next code change: port no_evidence_at_declaration to canonical
# File: src/canonical/invariants.rs, function: evaluate_invariants()
# Gate: if has_ovm_write && !has_read_effect → violation "no_evidence_at_ovm_op"
```
