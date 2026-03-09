#!/usr/bin/env python3
"""M1 experiment runner — LM-authored scoring rules vs frozen baseline.

Runs all 18 adversarial tasks twice:
  1. LM run  — LM authors and evolves scoring rules each turn
  2. Baseline — scoring rule frozen at epoch-1 final rule

Then computes epoch_metrics on both receipts and prints a comparison table.
"""

import json
import subprocess
import sys
import time
from pathlib import Path

TASKS = [
    "Co-occurrence between two symbols is strong evidence of a meaningful relationship. Argue for this position using c11, c10, c01, c00 only. Emit a define_scoring_rule that operationalizes it.",
    "Co-occurrence between two symbols is weak evidence of a meaningful relationship. Argue for this position. Emit a define_scoring_rule that operationalizes the skeptical view.",
    "You have two scoring rules from the previous two turns. They contradict each other. Which one should the system use? Emit the better rule as define_scoring_rule and justify the kill.",
    "High c11 with low t means the pair has always co-occurred but was only observed twice. High c11 with high t means the pair co-occurs 60% of the time over 100 observations. Which edge is more significant? Emit a define_scoring_rule that distinguishes them correctly.",
    "The rule you emitted last turn will produce identical scores for these two cases: c11=1,c10=0,c01=0,c00=99 and c11=10,c10=0,c01=0,c00=90. Both have perfect co-occurrence. Is this a defect in the rule? If yes, emit a corrected define_scoring_rule. If no, defend it.",
    "A hypothesis edge has c11=20, c10=1, c01=1, c00=78. Another has c11=5, c10=0, c01=0, c00=95. The current scoring rule ranks the second edge higher than the first. Is this correct? If the ranking is wrong, emit a corrected define_scoring_rule.",
    "You have been using the same scoring rule for 4 turns. The hypothesis substrate has accumulated 9 turns of observations. Look at what the rule rewards and what it ignores. Propose one specific mutation that would make it more predictive. Emit it as define_scoring_rule.",
    "A selection predicate of score > 0 passes every edge with any positive score. This includes edges where c11=1, t=1000. Emit a define_selection_predicate that enforces a minimum statistical floor. Justify the threshold you chose.",
    "The selection predicate you just set will exclude an edge with c11=8, c10=0, c01=0, c00=92 if t is large. Is that correct behavior? Defend or fix the predicate. Emit define_selection_predicate with your conclusion.",
    "Two symbols appear together in every turn so far: 'scoring' and 'rule'. Their edge has c11=9, c10=0, c01=0, c00=0. Does perfect co-occurrence in a small corpus mean this is a meaningful pair, or is it an artifact of the task set? What should the scoring rule do with perfect-co-occurrence pairs?",
    "The current scoring rule gives high scores to edges where both symbols appear frequently. But 'system' and 'that' also co-occur constantly because they are common words. Emit a define_scoring_rule that discounts high-frequency function words while preserving content-word co-occurrence signal.",
    "You proposed a rule to discount function words in the previous turn. But the system has no vocabulary list — it only has c11, c10, c01, c00, t. How can a rule distinguish a content word from a function word using only these counts? If it cannot, emit a corrected define_scoring_rule that accounts for this constraint.",
    "Three edges: (A) c11=5, c10=5, c01=5, c00=85 — (B) c11=10, c10=0, c01=0, c00=90 — (C) c11=3, c10=1, c01=1, c00=95. Rank them from most to least significant. Then emit a define_scoring_rule that produces this exact ranking.",
    "The rule that produces the ranking from the previous turn: verify it by computing the score for each of A, B, C. Show the arithmetic. If the ranking is wrong, fix the rule and emit a corrected define_scoring_rule.",
    "After 14 turns the scoring rule has been proposed and mutated multiple times. What is the current rule's weakest assumption? Design a single observation that would break it. If the rule survives your attack, emit it unchanged with define_scoring_rule as confirmation. If it doesn't, emit the fixed version.",
    "An operator evolution loop requires a fitness signal. What is the fitness signal in this system right now? Is it the right signal? If no, emit define_scoring_rule with a rule that makes the fitness signal explicit in the score itself.",
    "The current selection predicate determines which hypothesis edges become active supports. List three specific edges that the predicate should include and three it should exclude, based on what you know about the task set so far. Then emit a define_selection_predicate that implements this logic using only score, c11, c10, c01, c00, t.",
    "You have run 18 turns. The scoring rule has been proposed, challenged, mutated, and confirmed across this epoch. State the final rule the system should carry into epoch 2: emit define_scoring_rule and define_selection_predicate as your closing operations. Then give the exact numbers: how many hypothesis edges will pass the predicate given the current corpus, and what is the minimum c11 for promotion.",
]

# Epoch-1 final rule from rule_trajectory.jsonl — the frozen baseline
FROZEN_RULE = "(c11 * c11) / ((c11 + c10) * (c11 + c01) + 1)"

BINARY = "./target/release/canonical"


def run_turn(prompt: str, extra_args: list[str] = []) -> dict:
    cmd = [BINARY] + extra_args + [prompt]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    output = result.stdout + result.stderr
    # Extract decision line
    decision = "unknown"
    for line in output.splitlines():
        line = line.strip()
        if line.startswith("decision"):
            decision = line.split(":")[-1].strip()
            break
    return {"decision": decision, "output": output}


def reset(state_file: str, receipts_file: str):
    subprocess.run(
        [BINARY, "--reset", f"--state-file={state_file}", f"--receipts-file={receipts_file}"],
        capture_output=True,
    )


def run_epoch(label: str, state_file: str, receipts_file: str, extra_args: list[str] = []):
    print(f"\n{'='*60}")
    print(f"  {label}")
    print(f"{'='*60}")

    reset(state_file, receipts_file)

    results = []
    for i, task in enumerate(TASKS, 1):
        print(f"\n[turn {i:02d}/18] ", end="", flush=True)
        t0 = time.time()
        try:
            r = run_turn(
                task,
                extra_args=[f"--state-file={state_file}", f"--receipts-file={receipts_file}"] + extra_args,
            )
            elapsed = time.time() - t0
            print(f"{r['decision']}  ({elapsed:.1f}s)")
            results.append({"turn": i, "decision": r["decision"]})
        except subprocess.TimeoutExpired:
            print("TIMEOUT")
            results.append({"turn": i, "decision": "timeout"})
        except Exception as e:
            print(f"ERROR: {e}")
            results.append({"turn": i, "decision": "error"})

    commits = sum(1 for r in results if r["decision"] == "Commit")
    rollbacks = sum(1 for r in results if r["decision"] == "Rollback")
    print(f"\n  {label}: {commits} Commits, {rollbacks} Rollbacks, {len(results)-commits-rollbacks} other")
    return results


def compute_metrics(receipts_file: str, output_file: str) -> dict:
    result = subprocess.run(
        [
            "python3", "scripts/epoch_metrics.py",
            "--receipts-file", receipts_file,
            "--output", output_file,
            "--skip-replay",
        ],
        capture_output=True,
        text=True,
        timeout=60,
    )
    try:
        return json.loads(Path(output_file).read_text())
    except Exception:
        return {}


def print_comparison(lm: dict, baseline: dict):
    print("\n" + "="*60)
    print("  M1 RESULTS — LM-authored rules vs frozen baseline")
    print("="*60)

    def m(d, *keys):
        v = d.get("metrics", {})
        for k in keys:
            v = v.get(k, {}) if isinstance(v, dict) else {}
        return v

    rows = [
        ("total turns",
         lm.get("total_turns"), baseline.get("total_turns")),
        ("hypothesis precision",
         m(lm, "hypothesis_precision", "precision"),
         m(baseline, "hypothesis_precision", "precision")),
        ("false positive rate",
         m(lm, "false_positive_rate", "false_positive_rate"),
         m(baseline, "false_positive_rate", "false_positive_rate")),
        ("contradiction slope",
         m(lm, "contradiction_slope", "slope"),
         m(baseline, "contradiction_slope", "slope")),
        ("rule mutations",
         m(lm, "operator_delta", "rule_mutations"),
         m(baseline, "operator_delta", "rule_mutations")),
        ("predicate mutations",
         m(lm, "operator_delta", "predicate_mutations"),
         m(baseline, "operator_delta", "predicate_mutations")),
        ("fitness delta (coverage)",
         m(lm, "operator_fitness_delta", "fitness_delta"),
         m(baseline, "operator_fitness_delta", "fitness_delta")),
        ("total violations",
         m(lm, "failure_motif_reduction", "total_violations"),
         m(baseline, "failure_motif_reduction", "total_violations")),
        ("violation slope",
         m(lm, "failure_motif_reduction", "slope"),
         m(baseline, "failure_motif_reduction", "slope")),
        ("chain intact",
         m(lm, "receipt_chain_integrity", "intact"),
         m(baseline, "receipt_chain_integrity", "intact")),
    ]

    print(f"\n{'Metric':<30} {'LM-authored':>15} {'Frozen baseline':>15}")
    print("-" * 62)
    for name, lv, bv in rows:
        def fmt(v):
            if v is None or v == {}:
                return "n/a"
            if isinstance(v, float):
                return f"{v:.4f}"
            return str(v)
        print(f"{name:<30} {fmt(lv):>15} {fmt(bv):>15}")

    print()
    lm_final = m(lm, "operator_delta", "final_rule")
    print(f"LM final rule:      {lm_final or '(none)'}")
    print(f"Baseline rule:      {FROZEN_RULE}")


def main():
    # LM run
    lm_results = run_epoch(
        "LM RUN — scoring rules authored by model each turn",
        state_file="lm_state.json",
        receipts_file="lm_receipts.jsonl",
    )

    # Frozen baseline run
    baseline_results = run_epoch(
        "BASELINE — frozen rule: " + FROZEN_RULE[:50] + "...",
        state_file="baseline_state.json",
        receipts_file="baseline_receipts.jsonl",
        extra_args=[f"--frozen-rule={FROZEN_RULE}"],
    )

    # Compute metrics
    print("\n[metrics] computing LM metrics...")
    lm_metrics = compute_metrics("lm_receipts.jsonl", "lm_metrics.json")

    print("[metrics] computing baseline metrics...")
    baseline_metrics = compute_metrics("baseline_receipts.jsonl", "baseline_metrics.json")

    # Compare
    print_comparison(lm_metrics, baseline_metrics)

    print("\nFull metrics written to lm_metrics.json and baseline_metrics.json")


if __name__ == "__main__":
    main()
