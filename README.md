# N★ Bit

**The causal collapse of things.**

A protocol that makes LLMs domain-adaptable by running a metacognitive pass at every turn. No training. No fine-tuning. No infrastructure. Just a prompt protocol and a state file.

## What It Does

At every LLM turn, three passes run:

1. **Meta Pass** (before acting): What predicates exist? Which are active? Do any gates fire?
2. **Task Pass** (the actual work): Gated by the meta pass.
3. **Reflection Pass** (after acting): Did this turn reveal a failure mode not covered by current predicates? If yes, a new predicate emerges.

Predicates start at **zero**. The system discovers them from the pattern of its own successes and failures. After N turns, a domain-specific metacognitive model has emerged — without anyone designing it.

## Quick Start

```bash
# Run the nstar-bit protocol on any task
python3 nstar.py "Debug why auth.py returns 403 on valid tokens"

# Run in interactive mode
python3 nstar.py --interactive

# View the current state (accumulated predicates and collapses)
python3 nstar.py --state

# Reset state (start fresh)
python3 nstar.py --reset
```

## How It Works

```
Turn 1:  predicates = []           → LLM responds → reflection discovers "Uncertainty"
Turn 5:  predicates = [U, A, Δ]   → gates fire    → LLM verifies before acting
Turn 20: predicates = [U, A, Δ, Coupling, Temporal_Pressure, ...]
                                    → domain-specific metacognition emerged
```

The predicates aren't designed. They're **discovered**.

## Files

```
nstar.py          ← The protocol (one file, ~200 lines)
nstar_state.json  ← Accumulated predicates, gates, collapses (starts empty)
PROTOCOL.md       ← System prompt addendum (copy-paste into any LLM)
THEORY.md         ← Full theory document
```

## The Theory (One Paragraph)

The nstar bit is a function `n★(ins, outs) → collapsed_state` that takes what went into a computation and what came out, and returns where that computation sits in the space of all possible computations. It's dynamic in n (discovers its own dimensionality), node-state interchangeable (a metacognitive predicate and a domain concept are the same type of thing), and recursive (the collapse can collapse itself). The Viable System Model provides the universal structure: every viable domain has the same channels (algedonic/emergency, coordination, control, audit, intelligence, identity). The nstar bit discovers which channels are active in a given domain by observing ins and outs.

## Why This Exists

Previous attempts to build this involved 141MB mission graphs, 39K-line YAML manifests, 7+ distributed repos, and a Rust kernel with 40 binary targets. System health: `sick_score: 64.71`. 

This repo exists to test whether the **theory** works before building **infrastructure**. If after 20 turns the emergent predicates are useful, the theory holds. If not, no amount of Rust would save it.

## Origin

Discovered 2026-03-01 through a conversation that was itself an instance of the protocol.
