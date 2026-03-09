# Intent: Graph-Native Repo Assimilation
*Branch: intent/repo-assimilation — 2026-03-08*

---

## The Problem

There are 70+ repos of past work (meta2, meta3-graph-core, anthropic-proxy, donkey/prime_dsl, meta5-*, agentic-os, dreaming-kernel, graph-kernel, nix.codecli, one-engine(n), .mjs nstar system, etc.). Each repo is a snapshot of intent at the time of building. The treasure is the conceptual residue — what problem kept recurring across all of them.

Currently: no way to query across them. Knowledge is locked in static files.

## What We Are NOT Doing

- Not grepping files
- Not building a manual pipeline
- Not manually specifying metrics or rubrics
- Not creating a new ingestion system from scratch

## What We ARE Doing

The autogenesis engine already exists. It already has:
- Evidence-gated node/relation creation
- LM-authored scoring rules that evolve under adversarial pressure (M1 proven)
- OVM ops for emitting structured knowledge
- A receipt chain that proves what happened

The README corpus already exists:
- `README_CORPUS_SYNOPSIS.md` — 281 unique READMEs from across the machine, manually curated
- `scripts/build_readme_repo_summaries.py` — path collection + per-README prose summarization
- Paths span: Desktop archives, Workspace, kernel_sandbox, CascadeProjects, Developer

**The graph IS the index. Run the engine against the corpus.**

## The Architecture

```
README_CORPUS_SYNOPSIS.md (281 repo paths)
    ↓
Each repo = one turn in the autogenesis engine
LM reads the README → emits OVM ops:
  - node: what this repo was trying to do
  - relation: how it connects to other repos
  - evidence: the specific language/concepts that appear
    ↓
Scoring rule evaluates which relations survive
(c11 = concept appears in multiple repos = strong relation)
    ↓
Graph = queryable index of your development intent across all past work
    ↓
Orchestrator with dynamic prompts queries the graph
"what does nix.codecli share with nstar-bit?" → graph answers, no grep
```

## The Orchestrator Interface

Instruct it like a terminal agent. It runs the system in background.
LM makes the rubrics (not manually specified).
Dynamic prompts are generated from graph context.

Think: early meta2 work — but with the index first.
meta2 was trying to be the orchestrator. It had nothing to query.
Now we have the index. Now meta2 makes sense.

## The Seed File

`README_CORPUS_SYNOPSIS.md` is the manually curated seed. Key repo families:
- `meta2-engine/` — the orchestrator (needs the index we're building)
- `meta3-graph-core/` — hypergraph, UTIR, receipts, LeJIT verification
- `meta5-{causal,runtime,graph-viz,hot-reload,symbiotic-ui,...}` — next-gen layer
- `anthropic-proxy/` — the proxy (sits in front of all LM calls)
- `agentic-os/` — receipts/state/dashboards
- `donkey/prime_dsl` — prime encoding
- `dreaming-kernel/` + `graph-kernel/` — kernel work

Your manually-added synthesis (from the file) already identified:
> "Multiple parallel architectural vocabularies (kernel/graph/meta/agent) without one canonical contract."
> "The actionable implication is to force one canonical engine contract and treat all other forms as derived artifacts."

**That canonical engine contract = the autogenesis engine + its scoring rule.**

## What "Assimilation" Means

Not porting code. Not migrating systems.
Each repo's README becomes evidence for graph nodes.
The scoring rule discovers which concepts survived across all repos.
Those are your stable intentions — the ideas that kept returning regardless of implementation.

## Next Steps (in order)

1. Verify paths in `README_CORPUS_SYNOPSIS.md` still exist on disk
2. Write the ingestion turn prompt: "Read this README. What was this system trying to do? Emit define_node and define_relation OVM ops."
3. Run epoch: one turn per repo in the corpus
4. After epoch: graph contains concept map of all past work
5. Build orchestrator query interface on top

## What This Branch Does NOT Touch

- The M1 canonical Rust OVM (complete — see STATUS.md)
- The autogenesis engine runner (complete — see run_all_epochs.py)
- FINDINGS.md (already updated with M1 results)

---

*Confirmed intent from prompts 11–15 of session 2026-03-08.*
*Pending: user confirmation that interpretation is correct before implementation.*
