# Task: Clean Commit of Current Work

You are a git-savvy subagent. Your job is to make a clean, well-structured commit of all
current work in this repo. Do not ask questions. Read, decide, act.

## Context You Must Understand First

This repo (`nstar-bit`) is a Rust + Python system with two live processes running:
- `epoch_logs/epoch73_fork.json` — a large autogenesis state (DO NOT TOUCH)
- `nstar_canonical_state.json` — canonical OVM state (DO NOT TOUCH)
- `run_canonical_corpus.py` — corpus run script still executing

Do NOT interrupt any running processes. Do NOT reset state files. Only do git operations.

## Step 1: Read the current gitignore

```bash
cat .gitignore
```

## Step 2: Update .gitignore to exclude noise before staging

Add the following lines to `.gitignore` if they are not already present:

```
# Large framework (tracked separately)
marvin/

# Python cache
__pycache__/
scripts/__pycache__/

# Large log files (ephemeral)
epoch_logs/*.log
epoch_logs/canonical_*.log
epoch_logs/corpus_*.log
epoch_logs/runner.log

# Untracked conversation exports (too noisy)
*.md.resolved
rawchat-*.md

# Baseline/ab test logs
ab_adversarial.log
baseline_run.log
baseline_summary.tsv
epoch1_run.log
```

Keep these EXISTING rules:
- `target/` — Rust build artifacts
- `*.json` — large state snapshots
- `*.jsonl` — receipt chains
- `nstar-autogenesis/` — legacy python engine
- The subagent transcripts exception

## Step 3: Stage everything meaningful

Run:
```bash
git add .gitignore
git add -A
git status --short
```

Review what's staged. The expected staged files should be roughly:

**Core Rust source (modified):**
- `src/canonical/core.rs`
- `src/canonical/graph.rs`
- `src/canonical/invariants.rs`
- `src/canonical/mod.rs`
- `src/canonical/types.rs`
- `src/canonical/promotion.rs` (new)
- `src/canonical/schema.rs` (new)
- `src/bin/canonical.rs`
- `src/lib.rs`
- `src/lm.rs`
- `src/autogenesis.rs` (new)
- `src/bin/autogenesis.rs` (new)
- `src/bin/serve.rs` (new)
- `src/manifest.rs` (new)
- `Cargo.toml`, `Cargo.lock`

**Epoch data (summaries + tasks only — NOT fork JSONs):**
- `epoch_logs/epoch*_summary.tsv`
- `epoch_logs/epoch*_tasks.txt`
- `epoch_logs/thread.md`
- `epoch_logs/thread_summary_2026-03-08.md`
- `epoch_logs/policy.json` (if policy.json is small and important)

**Scripts:**
- `scripts/corpus_agent.py`
- `scripts/decay_bootstrap.py`
- `scripts/ingest_corpus.py`
- `run_all_epochs.py`
- `run_canonical_corpus.py`
- `run_corpus_assimilation.py`
- `run_m1.py`, `run_m1_epoch2.py`
- `run_epoch2.py`, `run_epoch3.py`, `run_epoch4.py`

**Docs:**
- `FINDINGS.md`
- `STATUS.md`
- `RUNNING_STATES.md`
- `INTENT_REPO_ASSIMILATION.md`
- `MEMORATUM_CORPUS_AGENT.md`
- `ARCHIVE_CANONICAL_STACK.md`
- `TAXONOMY.yaml`
- `TAXONOMY_AUDIT_TASK.md`
- Conversation exports: `Building Epistemic API.md`, `Declarative Architecture Refinement.md`, `Refining Graph Epistemics.md`, `Epoch 21 Bootstrap and Gate.md`

**Subagent runs:**
- `subagent_runs/*.md`
- `subagent_runs/transcripts/` (all)

**Experiments:**
- `experiments/output/` (all)
- `episteme/` (all)

**Do NOT stage:**
- `marvin/` (71MB — excluded by gitignore)
- `epoch_logs/*.log` (ephemeral noise)
- `epoch_logs/epoch*_fork.json` (excluded by *.json)
- `nstar_canonical_state.json` (excluded by *.json)
- `canonical_receipts.jsonl` (excluded by *.jsonl)
- `__pycache__/` (excluded)
- `cursor_casual_greeting.md` (not project work)
- `ab_adversarial.log`, `baseline_run.log` etc (ephemeral)

## Step 4: Verify staged file count is reasonable

```bash
git diff --staged --stat | tail -5
```

If more than 200 files are staged, something went wrong. Stop and check.

## Step 5: Write the commit message

The commit should capture the full scope of work since the last commit (`275be98`).

Use this message:

```
snapshot: corpus run + autogenesis + taxonomy + audit docs

What this commit contains:

Rust (canonical OVM):
- Autogenesis engine ported to Rust (src/autogenesis.rs, src/bin/autogenesis.rs)
- HTTP serve endpoints: /turn, /synthesize, /manifest (src/bin/serve.rs)
- Manifest dispatch stub (src/manifest.rs)
- OVM promotion module (src/canonical/promotion.rs)
- OVM schema module (src/canonical/schema.rs)
- B1 fix: null_to_default_f32 on all f32 fields (schema.rs)
- B2 fix: OVM ops count as evidence in invariants.rs
- B3 fix: exponential backoff on rate limits in lm.rs
- Behavioral substrate: node: prefix filtering, c11/c10/c01/c00 on hypothesis edges
- Held-out rule evaluator: P@K, R@K scorecard injected into LM prompt
- Frozen-rule A/B flag for M1 experiment
- Rule trajectory logger: rule_trajectory.jsonl
- evaluate_rule_heldout() every 5 turns
- Seed queue + investigated_pairs on GraphState

M1 Experiment Results:
- Epoch 1: LM rule P@10=0.200, frozen P@10=0.200 (tied, small corpus)
- Epoch 2 (adversarial): LM rule P@10=0.100, frozen P@10=0.000
- LM rule added t-dependence under adversarial pressure; frozen degraded entirely
- Hard question answered: LM-authored rules outperform frozen baselines

Corpus runs:
- run_canonical_corpus.py: feeds project READMEs through canonical engine
- run_all_epochs.py: autogenesis epoch runner (73 epochs completed)
- scripts/corpus_agent.py: rewired to call autogenesis binary (not write JSON)
- scripts/ingest_corpus.py: meta6/7 abstraction ingestion

Epoch data:
- epoch_logs/epoch*_summary.tsv (73 epochs)
- epoch_logs/epoch*_tasks.txt (73 epoch task sets)
- epoch_logs/thread.md (52KB running narrative)
- epoch_logs/thread_summary_2026-03-08.md

Docs:
- FINDINGS.md: permanent findings (3 proven principles)
- STATUS.md: M1 results confirmed
- RUNNING_STATES.md: two-thread catalog
- INTENT_REPO_ASSIMILATION.md
- MEMORATUM_CORPUS_AGENT.md
- TAXONOMY.yaml: machine-readable 10-section design-space taxonomy
- TAXONOMY_AUDIT_TASK.md: agent task to audit taxonomy against history

Subagent investigations:
- 7 targeted research reports
- 39 conversation transcripts

Open blockers:
- no_evidence_at_declaration gate not ported from Thread 1 to canonical
- Seed queue still open-loop (M2)
- PURGE_HARDCODED.md execution pending (~8h work)
- macro-hard gate still reading vestigial Python fields
```

## Step 6: Commit

```bash
git commit -m "<paste the message above>"
```

## Step 7: Confirm

```bash
git log --oneline -3
git status --short | head -5
```

Report: total files committed, the commit hash, and any files still untracked that seem important.
