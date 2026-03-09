# Task: Audit TAXONOMY.yaml Against Full Project History

## Objective

`TAXONOMY.yaml` contains a 10-section classification of the N★ Bit project's design space. Each leaf node has a `status` field (LIVE, CONCEPT, PLANNED, EXTERNAL, ASPIRATIONAL, PROVEN_UNPORTED) and an `evidence` field.

**The current audit was static** — it only checked `src/` source files and top-level markdown docs. It did NOT scan:

1. **73 epoch summary TSVs** (`epoch_logs/epoch*_summary.tsv`)
2. **The 52KB thread log** (`epoch_logs/thread.md`)
3. **7 subagent reports** (`subagent_runs/01-07_*.md`)
4. **39 subagent transcripts** (`subagent_runs/transcripts/`)
5. **Experiment outputs** (`experiments/output/`)
6. **The marvin framework** (`marvin/` — 324 files, may contain relevant abstractions)
7. **Conversation exports** (top-level `*.md` files like `Building Epistemic API.md`, `Declarative Architecture Refinement.md`, `Refining Graph Epistemics.md`, `cursor_casual_greeting.md`)
8. **Git commit messages and diffs** (only 12 commits but with detailed bodies)

Any of these sources could contain work, experiments, or decisions that should **change the status** of a taxonomy node.

## Work Instructions

### Phase 1: Scan epoch summaries for taxonomy-relevant evidence (fastest)

```bash
# Read ALL epoch summary TSVs — they are small (1-2KB each)
for f in epoch_logs/epoch*_summary.tsv; do echo "=== $f ==="; cat "$f"; done
```

For each summary, check if it describes:
- A concept or mechanism that maps to a taxonomy node
- A proven finding, adoption, or rejection
- An empirical measurement that would change a status from ASPIRATIONAL to CONCEPT/PLANNED/LIVE

### Phase 2: Scan the thread log

```bash
cat epoch_logs/thread.md
```

This is the running narrative of 73 epochs. Look for:
- Pivots where a taxonomy node was tried and failed
- Discoveries that prove or disprove a theoretical claim
- Architectural decisions that moved something from planned to implemented

### Phase 3: Scan subagent reports

```bash
for f in subagent_runs/0*.md; do echo "=== $f ==="; cat "$f"; done
```

These are targeted investigations. particularly:
- `05_architecture_search_synthesis.md` — may contain VSA/HDC/hypergraph research
- `03_meta3_branch_investigation.md` — may reference negentropy/VSA concepts
- `06_ethos_and_insertion_points.md` — may clarify theoretical grounding

### Phase 4: Scan conversation exports

```bash
head -100 "Building Epistemic API.md"
head -100 "Declarative Architecture Refinement.md"
head -100 "Refining Graph Epistemics.md"
```

These are full conversation logs with architectural decisions. Look for delineation changes.

### Phase 5: Check the marvin framework

```bash
cat marvin/CLAUDE.md
cat marvin/src/marvin/instructions.md
cat marvin/src/marvin/engine/CLAUDE.md
```

Marvin is a separate framework bundled in this repo. Check if it implements any taxonomy nodes that `src/canonical/` doesn't.

## Output Protocol

For each taxonomy node whose status should change, update `TAXONOMY.yaml` in-place:

1. Change the `status` field to the correct value
2. Update the `evidence` field with the specific file/line/epoch that proves the new status
3. Set `last_audited` to `"2026-03-09"`
4. Add a `notes` field explaining what you found

If you discover work that doesn't fit any existing taxonomy node, add a new node at the appropriate level with:
- A descriptive key and `label`
- The correct `status`
- Evidence with source file/epoch
- `notes: "Discovered during audit — not in original taxonomy"`

## Status Definitions

| Status | Meaning |
|--------|---------|
| **LIVE** | Code compiles, runs, produces verifiable output. Cite file:line. |
| **CONCEPT** | Exists as a discovered concept in the autogenesis graph (epoch_logs/epoch*_fork.json) but not as executable code. |
| **PLANNED** | Has a named checklist item, milestone, or design doc. No code yet. |
| **EXTERNAL** | Citing published research. Not measured on this system. |
| **ASPIRATIONAL** | No code, no plan, no external benchmark. Theoretical target. |
| **PROVEN_UNPORTED** | Empirically proven in Thread 1 (autogenesis, 40+ epochs via run_all_epochs.py) but not ported to Thread 2 (canonical Rust in src/canonical/). |

## Priority Order

1. Epoch summaries + thread.md (highest density of status-changing evidence)
2. Subagent reports (targeted investigations may have researched theoretical layers)
3. Conversation exports (architectural decisions)
4. Marvin/experiments (lowest priority — likely tangential)

## Done Condition

The task is done when:
- Every `last_audited` field in TAXONOMY.yaml is `"2026-03-09"` with accurate status
- Any new taxonomy nodes discovered from the history are added
- A summary of all status changes is printed to stdout
