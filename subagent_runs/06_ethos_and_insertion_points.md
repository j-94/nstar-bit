# Ethos Audit and LM Insertion Points

**Tasks (subagent prompts):**

1. **Autogenesis audit** (`571c45f1`): List exact deterministic/statistical mechanisms in the autogenesis path, with file paths and function names. Focus on what would need to be demoted, removed, or wrapped if the LM should make nearly all decisions.

2. **LM insertion points** (`c85c458a`): Analyze src/autogenesis.rs, src/lm.rs, src/bin/autogenesis.rs. Return best 5–8 insertion points or seams for replacing deterministic logic with LM-driven state updates, plus what data is available at each seam.

---

## Status

Subagent tasks ran; results were synthesized into the parent transcript and later refactors. The parent transcript does not contain standalone "autogenesis audit" or "insertion points" blocks; the findings drove:

- LM transition schema and turn contract changes in `src/lm.rs`
- Consequence enforcement in `src/autogenesis.rs` (upsert_relation, upsert_concept, apply_alias)
- Meta-runner (fork/compare/adopt) in `src/bin/autogenesis.rs`

---

## Reference: Existing Ethos Documentation

For concrete audit and remediation, see:

- **`PURGE_HARDCODED.md`** — Hardcoded constants table (MAX_NODES, activation_cutoff, propagation_steps, etc.), Phase 1–2 checklist, LM-mutable criteria
- **`CRITIQUE_NSTAR_BIT.md`** — Theory–implementation gap, two-perspective tension
- **`FINDINGS.md`** — Proven findings from epoch runs (grounding problem, stratification, adoption decisions)

The autogenesis path today is LM-authoritative for concepts, relations, evidence, active_focus, next_probes. Deterministic logic remains in schema validation, persistence, hashing, graph merge, and consequence gates (no placeholder concepts, archived cascades, alias validation).
