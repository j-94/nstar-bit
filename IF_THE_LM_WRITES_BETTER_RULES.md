# If The LM Writes Better Scoring Rules Than Humans

**What it opens up, and how to get there fast.**

---

## I. What a Positive Result Actually Means

Say you run 20 turns. The LM proposes a scoring rule at turn 1. It mutates it at turns 5, 12, and 17. By turn 20, hypothesis precision is 0.72 and the old EMI+Beta regime scores 0.55 on the same count data. The LM won.

What just happened is not "a better formula was found." What happened is that the system closed a loop that has been open in every production LLM deployment: the loop between *what the model knows implicitly* and *what the system can act on explicitly*.

Every LLM already has latent structure about what matters in a given domain. When you prompt GPT-4 or Claude about software architecture, the model "knows" that coupling is more important than naming conventions for system stability. But that knowledge lives in weights. It cannot steer the system's own attention, selection, or resource allocation — because the scoring function is external to the model, written by a human, and static.

If the LM can write a scoring rule that outperforms the human-authored one, it means the model's latent domain knowledge has been *externalized into an executable, inspectable, falsifiable artifact*. The implicit became explicit. And because the artifact is a string expression evaluated against raw counts, it is:

- **Readable.** You can look at it and see what the model thinks matters.
- **Diffable.** You can compare turn 5's rule to turn 17's rule and see how the model's theory of relevance changed.
- **Transferable.** You can take the rule from one domain run and seed another.
- **Falsifiable.** You can replay the receipt chain and prove the rule works or doesn't.

No other system in the current LLM ecosystem does this. Fine-tuning changes weights (opaque). RLHF changes policy (opaque). Prompt engineering changes input (brittle). This changes the *selection function itself* — and it does so legibly.

---

## II. What It Opens Up Theoretically

### A. The Fitness Landscape Is Learnable From Within

This is the big one. In evolutionary systems, the fitness function is always external — defined by the environment, defined by the researcher, defined by the reward model. The organism does not write its own fitness function. If your LM does, you have crossed a boundary that most self-improving system proposals wave at but never reach.

What this means concretely: the system does not just learn *which hypotheses are good*. It learns *what "good" means for this domain*. EMI says "good = non-random co-occurrence." That is a universal prior. The LM might discover that in a software engineering context, "good = high c11 with low c01 when the total observation count is above 50" — a domain-specific selection criterion that EMI would never surface because EMI does not know what software is.

The theoretical implication is that the scoring rule becomes a *compressed representation of domain epistemology*. It encodes what counts as evidence in this domain. Different domains will produce different rules, and those rules are themselves data about what the domain is like.

### B. Adaptive Selection Pressure

EMI+Beta applies the same selection pressure at turn 1 and turn 1000. The prior is static. The LM-authored rule can change. This is not a minor difference — it means selection pressure can adapt to the phase of exploration.

Early turns: the system has few observations. A good early-phase rule might favor breadth — promote hypotheses with high variance, low confidence, explore the space. This is what the old Beta `pv` field was supposed to do, but it was hardcoded to a fixed exploration schedule.

Late turns: the system has many observations. A good late-phase rule might favor precision — promote only hypotheses with high c11 and low c01, prune aggressively. The LM can discover this phase transition on its own, because it sees the count data growing and can reason about when to shift from exploration to exploitation.

No fixed formula can do this without a human designing the phase schedule. The LM can discover the schedule from the data.

### C. Meta-Scoring (Rules About Rules)

If the LM can write a scoring rule and the system can evaluate whether that rule was good (via the eight metrics from the critique), then the system has a meta-level signal: *which rules produce good epochs*.

This opens a second-order loop:

- Turn-level: the LM writes a scoring rule for hypotheses.
- Epoch-level: the system evaluates whether that scoring rule improved performance.
- Cross-epoch: the LM uses the history of (rule, performance) pairs to write better rules.

This is where the "meta-space construction" in THEORY.md stops being speculative and starts being concrete. The meta-space is literally the space of scoring rules, parameterized by epoch performance. The system navigates that space by proposing rules, measuring results, and revising. The multi-scale recursion becomes real: Token scale is hypothesis evaluation, Turn scale is rule application, Session scale is rule mutation, Project scale is rule-trajectory analysis.

But — and this is important — you only get here if epoch 1 shows the rule *changes at all and the change helps*. If the LM proposes the same rule 20 times, this entire branch collapses.

### D. Transfer and Domain Fingerprinting

If different domains produce different scoring rules, then the scoring rule itself becomes a domain fingerprint. You can characterize a domain by the rule the system converges to when operating in it.

Software engineering might converge to a rule that weighs coupling metrics heavily. Medical diagnosis might converge to a rule that penalizes false negatives exponentially. Financial analysis might converge to a rule that discounts observations older than N turns.

These domain fingerprints are useful artifacts in their own right:

- **Cold start:** Seed a new session with the converged rule from a similar domain instead of starting from scratch.
- **Domain detection:** Run the first 5 turns with no seed, look at the rule the LM proposes, and classify what domain you are in.
- **Anomaly detection:** If the rule suddenly changes character mid-session, something about the domain shifted. This is a free signal.

### E. Legible Disagreement

Here is something no current LLM system can do: *tell you exactly where and why two models disagree about what matters*.

If you run the same task set through two different LMs (or two different prompting regimes for the same LM), they will produce different scoring rules. You can diff those rules. The diff tells you, in an executable expression, what the two systems weight differently. This is not interpretability in the "look at attention heads" sense. It is interpretability in the "here is a readable formula, and here is how it differs from the other readable formula" sense.

This is probably the lowest-effort, highest-novelty claim the project can make. Nobody else has this. It falls directly out of the architecture you already built.

---

## III. How to Get at the Low-Hanging Fruit Fast

The theoretical landscape above is large. Here is what you can reach in the next 2–4 weeks with the infrastructure you already have, ordered by effort-to-insight ratio.

### Fruit 1: The A/B Baseline (This Week)

**What:** Run the same 20-turn fixed task set twice. Once with EMI+Beta hardcoded as the scoring rule (freeze it, do not let the LM mutate it). Once with the LM free to propose and mutate.

**Why this is the fastest path to a publishable result:** You already have both systems. The old EMI formula is in your documents. Hardcoding it as a frozen `graph.scoring_rule` is trivial — you just skip the `DefineScoringRule` operation. The LM-free run gives you a baseline. The LM-authored run gives you the candidate. Compare hypothesis precision, false positive rate, and contradiction slope.

**Architecture needed:** Almost nothing. You need a flag (`--frozen-rule "expr"`) on the canonical binary that ignores `DefineScoringRule` ops and uses the provided expression. One CLI argument. One `if` statement in `core.rs`.

**What you learn:** Whether the LM adds value at all. This is the gate for everything else.

### Fruit 2: Rule Trajectory Logging (This Week)

**What:** Every time the LM proposes a `DefineScoringRule`, log the old rule and the new rule to a `rule_trajectory.jsonl` file with the turn number, the hypothesis count distribution, and the reason the LM gave for changing it (extract from the UTIR proposal text).

**Why:** This is free data. You are already processing `DefineScoringRule` ops. Adding a log line costs nothing. But the trajectory is the single most interesting artifact the system produces — it is a readable record of how the LM's theory of relevance evolves.

**Architecture needed:** One struct, one append to a JSONL file, in the `DefineScoringRule` match arm of `core.rs`.

**What you learn:** Whether the rule converges, oscillates, or drifts. Whether the LM's stated reason for changing maps to actual performance improvement. Whether different task sets produce different trajectories (domain fingerprinting).

### Fruit 3: Rule Regression Testing (Week 2)

**What:** At the end of a 20-turn epoch, take the final scoring rule and retroactively evaluate it against the count data from every previous turn. Compare to the rule that was active at each turn.

**Why:** This answers the question "was the final rule the best rule, or was an intermediate version better?" If an intermediate rule scores higher on late-turn data than the final rule, the LM overfit or drifted. This is cheap to compute — you already have the counts in the receipt chain, and `evalexpr` evaluation is microseconds.

**Architecture needed:** A post-epoch script (Rust or Python) that loads the receipt chain, extracts count snapshots and active rules per turn, and cross-evaluates.

**What you learn:** Whether the LM's rule mutations are monotonically improving or noisy. If noisy, you need a selection mechanism on rules themselves (keep the best rule seen so far, not the most recent). This is the seed of meta-scoring.

### Fruit 4: Seed Queue Closure (Week 2–3)

**What:** Take the hypotheses that survived the epoch (edge weight above selection predicate after turn 20), extract the top-uncertainty ones (highest `c01 + c10` relative to `c11`), and automatically inject them as the context for the next turn's LM prompt.

**Why:** This closes the exploration loop. Right now, the system waits for a human to type a prompt. With seed queue closure, the system says "I am most uncertain about the relationship between X and Y — my next turn will investigate that." This is where the system becomes autonomous rather than reactive.

**Architecture needed:** A function in `core.rs` that, after `apply_operator`, selects the top-K highest-uncertainty hypothesis edges and formats them as a prompt prefix. Wire this into the LM call in `canonical.rs`.

**What you learn:** Whether autonomous exploration discovers different (and better) hypotheses than human-directed exploration. This is the "zero predicates, discover from failures" promise made concrete.

### Fruit 5: The Legible Disagreement Demo (Week 3–4)

**What:** Run two epochs with different LMs (or different system prompts for the same LM) on the same fixed task set. Diff the converged scoring rules. Write a short document showing the diff and interpreting what it means.

**Why:** This is the demo. This is the thing you show people. "Here is what Claude thinks matters in software engineering. Here is what GPT-4 thinks matters. Here is the exact formula where they disagree. Here is which one was right, verified by deterministic replay."

Nobody else can produce this artifact. It is a direct consequence of the architecture, and it requires only infrastructure you already have plus the A/B harness from Fruit 1.

**Architecture needed:** The frozen-rule flag from Fruit 1, two LM endpoints, and a diff script.

---

## IV. What to Deliberately Not Build Yet

The theory opens doors to multi-scale recursive meta-scoring, VSM channel mapping, Ruliad coordinate systems, and cross-domain transfer learning. All of these are interesting. None of them are worth building until epoch 1 produces numbers.

Specifically:

- **Do not wire up Session/Project scale coordinates** until Token/Turn scale proves the operator evolves. Multi-scale is meaningless if single-scale does not work.

- **Do not build a meta-scoring loop** (rules about rules) until you have at least 5 epochs of rule trajectories to analyze. You need data about rule quality before you can learn from it.

- **Do not implement the VSM mapping** (algedonic channels, System 3* audits) until you have a concrete case where it would have changed a decision. Right now it is interpretive, not operational.

- **Do not refactor macro-hard promotion gates** beyond the minimum needed to read canonical state instead of Python state. The promotion system is downstream of the question "does the operator improve." Answer the question first.

- **Do not write a paper or a theory update** until `epoch_001_metrics.json` exists and you have looked at the numbers.

---

## V. The 30-Day Path

| Week | Focus | Deliverable | Gate |
|------|-------|-------------|------|
| 1 | Run epoch 1 (20 turns, fixed task set). Add frozen-rule flag. Add rule trajectory logging. | `epoch_001_metrics.json`, `rule_trajectory.jsonl`, baseline vs candidate comparison | Hypothesis precision is measurable (even if low) |
| 2 | Run epoch 2 (same tasks, improved prompt or seed). Build rule regression testing. Close seed queue. | `epoch_002_metrics.json`, regression analysis, first autonomous-exploration turns | Precision delta between epoch 1 and 2 is positive |
| 3 | Run epoch 3 with seed queue driving exploration. Start legible-disagreement demo with second LM. | `epoch_003_metrics.json`, rule diff between two LMs | Autonomous exploration produces at least one hypothesis human prompts missed |
| 4 | Consolidate. Write up findings. Decide whether multi-scale and meta-scoring are warranted by evidence. | Summary report with hard numbers. Go/no-go on theory expansion. | The hard question has an answer: yes or no |

The point of this timeline is not speed. It is focus. Every week produces a numbered artifact with metrics. No week produces theory without data. If the numbers say the paradigm works, you will have four weeks of evidence to build on. If they say it does not, you will know exactly where it broke and can revise the architecture rather than the narrative.

---

## VI. The Honest Summary

If the LM writes better scoring rules than humans, what opens up is a new kind of machine learning where the learned artifact is not weights or policies but *readable, diffable, falsifiable selection criteria*. The system learns what counts as evidence, not just what the evidence says. That is genuinely novel and genuinely useful.

The fastest path to proving it is not more infrastructure. It is 20 turns, 8 metrics, and an honest look at the numbers. Everything you need to run that test exists right now except a frozen-rule CLI flag and a trajectory log file. Both are afternoon work.

The low-hanging fruit — A/B baseline, rule trajectory, legible disagreement — are all demos that fall out of the architecture for minimal additional engineering. They are also the artifacts that would make other people understand why this project matters.

Build the fruit. Skip the theory. The theory earns its keep when the numbers land.
