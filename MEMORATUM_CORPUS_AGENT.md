# MEMORATUM: Corpus Agent Run (corpus_agent.py)
*Isolated 2026-03-08 — do not remove, do not use as architecture*

## What This Was

A manual pipeline (`scripts/corpus_agent.py`) that read past repos and conversation vault,
extracted concepts and relations, and wrote them directly to a JSON graph state.

**This violated the original architecture spec (INTENT_REPO_ASSIMILATION.md).**
The system was not deciding what to gather — we hardcoded the schema, the sources, and the extraction format.
It was a freeform agent dressed as an autonomous system.

The run is preserved here as signal. The actual assimilation should go through the autogenesis engine.

---

## What It Actually Found (signal worth keeping)

### Core Lineage Map

| From | To | Why |
|------|----|-----|
| `meta_engine_series` | `agentic_os` | Hardcoded control logic → receipts + state dashboards |
| `agentic_os` | `dreaming_kernel` | Rigid structural rules failed; needed grounding over inertia |
| `dreaming_kernel` | `execution_kernel` | Self-mutation without verifier = instability |
| `execution_kernel` | `nstar_bit` | Backend rigor preserved; operator UX abandoned |

### Surviving Patterns (appeared across all lineages)
- `signal_driven_execution` — the thread from meta2 to nstar; control via state-change predicates, not hardcoded paths
- `receipt_pattern` — immutable log of state transitions, present in every generation
- `dynamic_predicate_discovery` — the core nstar-bit insight: control logic discovered at runtime
- `operator_control_plane` — externalized steering interface; every generation tried this differently
- `unified_execution_substrate` — the recurring unrealized goal across meta, agentic-os, dreaming-kernel

### Key Pivot Reasons (extracted from actual sources)
- `agentic_os_ui_failure`: "Abandoned due to excessive cognitive load on the operator vs. kernel-level control"
- `competitiveness_gap`: "Technical rigor (execution_kernel) outpaces usability (operator_ux)"
- `dreaming_kernel_instability`: "Inherent failure of unconstrained self-mutation without external verification"
- `pivot_to_state_mutation`: "Developer abandoned passive API-based knowledge retrieval in favor of active, self-mutating program graphs"
- `meta_search_api_dependency`: "Abandoned because it created non-deterministic execution paths that broke the verifier"

### Critical Contradictions Found
- `dynamic_predicate_discovery` **contradicts** `control_logic` — this is nstar-bit's founding tension
- `dreaming_kernel` self-mutation **contradicts** `receipt_based_validation` — why the kernel was superseded
- `audit_driven_inflation` **contradicts** `signal_driven_execution` — internal auditing creates fake stability

### Grounding Failures (what kept recurring and failing)
- `grounding_starvation`: strict verification rules prevent acquiring new valid knowledge
- `epistemic_drift`: agent's internal model diverges from reality under self-mutation
- `absolute_grounding_deficit`: relations exist without causal anchor in dialogue evidence

---

## State Files (preserved, not deleted)

- `epoch_logs/corpus_agent_state.json` — 1035 concepts, 1707 relations (raw, unscored)
- `epoch_logs/corpus_agent.log` — full turn-by-turn log
- `epoch_logs/corpus_agent_run.log` — stdout from last run
- `scripts/corpus_agent.py` — the script itself (isolated, not used going forward)

---

## Why This Approach Was Wrong

From `INTENT_REPO_ASSIMILATION.md`:
> - Not grepping files
> - Not building a manual pipeline
> - Not manually specifying metrics or rubrics
> - The autogenesis engine already exists. Run the engine against the corpus.
> - LM makes the rubrics (not manually specified)
> - The graph IS the index

corpus_agent.py did all the things we said not to do. The concepts/relations it found are real signal
but they bypassed the scoring rule — nothing was filtered, nothing earned its place. Everything
is in the graph regardless of whether it appeared across multiple sources.

---

## What To Do Instead

Feed `README_CORPUS_SYNOPSIS.md` (281 repo paths) to `run_all_epochs.py` as tasks.
One turn per repo. LM emits OVM ops. Scoring rule filters.
The epoch_logs graph becomes the queryable index — no separate corpus_agent needed.
