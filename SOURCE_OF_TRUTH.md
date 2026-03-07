# Source Of Truth

This file defines what to trust when files in `nstar-bit` disagree.

## Trust Order

1. Runtime truth
   - `src/bin/canonical.rs`
   - `src/canonical/core.rs`
   - `src/canonical/graph.rs`
   - `src/canonical/invariants.rs`
   - `src/canonical/types.rs`
   - `src/bin/replay.rs`
2. Observed runtime artifacts
   - `canonical_receipts.jsonl`
   - `nstar_canonical_state.json`
   - `epoch1_summary.tsv`
   - experiment output summaries and logs
3. Current planning and reality-check docs
   - `CRITIQUE_NSTAR_BIT.md`
   - `DIVERGENCE.md`
   - `MILESTONES.md`
   - `PARALLEL_SESSIONS.md`
4. Architecture/spec intent
   - `PROJECT_MANAGEMENT_SPEC_CANONICAL_SYSTEM.md`
   - `REPORT_ARCHITECTURE_EVIDENCE_AND_THREAD_V3.md`
5. Theory and prompt framing
   - `THEORY.md`
   - `PROTOCOL.md`
6. Historical implementation
   - `_legacy/*`

## Canonical Path

For implementation work, the canonical execution lane is:

- `src/canonical/*`
- `src/bin/canonical.rs`
- `src/bin/replay.rs`

If a document conflicts with this code, the code wins.

## Current State

The repository is in an experimental validation phase, not a finished or promotion-ready phase.

What is true now:

- The canonical Rust core exists and runs.
- The graph-first turn pipeline, receipts, replay, and OVM rule path are implemented.
- Epoch-style evaluation scripts and artifacts exist.
- The central claim is still under test: whether LM-authored scoring rules beat the old fixed regime.

## Files To Treat Carefully

- `README.md`
  - Useful overview, but stale in places.
- `PROJECT_MANAGEMENT_SPEC_CANONICAL_SYSTEM.md`
  - Good goal document, but not always current runtime truth.
- `invalidation_review.md.resolved`
  - Too optimistic; do not use as final status.
- `THEORY.md`
  - Motivation and framing, not acceptance criteria.
- `PROTOCOL.md`
  - Prompt-level intent, not executable system behavior.
- `experiments/README.md` and older experiment harness files
  - Useful context, but mixed with older assumptions and path drift.
- `_legacy/*`
  - Historical only.

## Immediate Blockers

These are the main issues still blocking a trustworthy evaluation loop:

1. OVM violations are applied after decision selection in `src/canonical/core.rs`.
   - A turn can still `Commit` even if the final OVM path makes invariants fail.
2. Replay verification does not fully flow back into saved canonical state.
   - `src/bin/replay.rs` rewrites receipt JSONL, but promotion checks `self.state.receipts`.
3. Bootstrap OVM behavior is inconsistent.
   - The prompt says the scoring rule may be absent, but runtime treats an empty rule as a violation.
4. Test coverage is too thin for the new path.
   - There are not enough tests around OVM, replay-state sync, and promotion eligibility.
5. Docs need synchronization.
   - Top-level status docs do not consistently reflect current runtime behavior.

## Practical Rule

When making decisions:

- build against `src/canonical/*`
- verify against receipts and saved state
- use `CRITIQUE_NSTAR_BIT.md` and `DIVERGENCE.md` as the best prose description of present reality
- treat theory documents as motivation, not proof
