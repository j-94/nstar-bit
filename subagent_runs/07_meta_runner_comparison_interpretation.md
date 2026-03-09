# Meta-Runner Comparison — Design Interpretation

**Context:** Baseline vs consequence-enforced 50-turn runs. Manual fork → compare → adopt.

| Metric | Delta | Meaning |
|--------|-------|---------|
| Concepts | 59 → 48 (−11) | Consequence run has fewer concepts |
| Archived concepts | +1 | At least one concept archived (cascade applied) |
| Rejected fields | +11 | Relations blocked by no-placeholder rule |
| Evidence | +2 | More dialogue-sourced evidence in consequence run |

---

## 1. Did enforcement reduce ontology bloat?

**Yes.** The −11 concepts and +11 rejected relations line up: the no-placeholder rule blocks relations whose source or target concept does not already exist. The baseline allowed relations to non-existent concepts (placeholder behavior). That produced:

- Forward references to concepts the LM had not yet introduced
- Orphan relation attempts that inflated the concept count by implying new concepts
- A larger, looser ontology

Consequence enforcement forces **concepts before relations**. The LM must introduce a concept before it can relate it to anything. That removes speculative edges and slows ontology growth.

**Conclusion:** Enforcement reduces bloat by turning relation attempts into explicit rejections instead of silent placeholder creation.

---

## 2. Is the tradeoff acceptable?

**Yes, with the usual caveats.**

**Costs:**
- Fewer concepts (48 vs 59) — some may be legitimate ideas the LM tried to express via relation before defining the concept
- More rejections — the LM must learn ordering: introduce concepts, then relate them
- Stricter semantics — no "I’ll create the concept by using it in a relation"

**Benefits:**
- No ghost concepts implied by orphan relations
- Archive cascades actually run (status="archived" on a concept archives its relations)
- Clearer graph: every relation has two defined endpoints
- More evidence (+2) suggests the system is better at linking dialogue to structure

The ethos is "zero hardcoded control logic; the system discovers from its own operation." Consequence enforcement is not hardcoded control — it enforces graph consistency (no dangling references). That supports discovery rather than replacing it. The LM still decides *what* concepts and relations to propose; the runtime enforces *that* they form a valid graph.

**Conclusion:** The tradeoff is acceptable. Stricter graph validity is infrastructure; the LM’s ontology choices remain discoverable.

---

## 3. What does adoption imply for future runs?

**Adopting the consequence run implies:**

1. **Consequence enforcement is the default.** Future forks and runs should keep upsert_relation (no unknown source/target), upsert_concept (archive honored, cascade), and apply_alias (canonical must exist). These rules are now baseline behavior.

2. **Ontology growth is no longer unbounded.** The LM cannot grow the graph by writing relations to non-existent concepts. Growth requires explicit concept introduction first. That favors compression and revision over constant expansion.

3. **Archive is operational.** The +1 archived concept shows that status="archived" is used and that cascades run. The LM can retire concepts; the system will archive their relations. Future runs can test how the LM learns to archive vs. accumulate.

4. **Evidence linkage is stronger.** +2 evidence in the consequence run suggests that with tighter structure, dialogue-originated evidence is more consistently recorded. That supports the evidence gate (no confidence without dialogue evidence).

5. **Meta-runner lineage is usable.** Fork → compare → adopt worked. The receipt (concept_delta, archived_concept_delta, rejected_field_delta, evidence_delta, health stats) gives a machine-readable basis for future comparisons. The next step is LM-authored fork proposals, comparison reasons, and adoption decisions instead of manual CLI use.

---

## Summary

| Question | Answer |
|----------|--------|
| Did enforcement reduce ontology bloat? | Yes — no-placeholder rule blocked 11 relations, reduced concepts by 11 |
| Is the tradeoff acceptable? | Yes — graph consistency is infrastructure; LM still discovers content |
| What does adoption imply? | Consequence enforcement is default; archive works; evidence linkage improves; meta-runner ready for LM-driven fork/compare/adopt |
