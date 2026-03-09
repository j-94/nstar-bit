#!/usr/bin/env python3
"""M1 Epoch 2 — stress-test the evolved rule and check if it holds out.

Starts from the LM's epoch-1 final state. Runs 18 new adversarial tasks
that specifically probe the evolved rule's weaknesses, then re-runs
held-out eval comparing: epoch-1 frozen rule vs epoch-2 evolved rule.
"""

import json
import subprocess
import sys
import time
from pathlib import Path

# Epoch-1 final rules
LM_RULE_E1    = "(c11 * c11) / (c11 + 2 * (c10 + c01) + (1 / (c11 + 1)))"
FROZEN_RULE   = "(c11 * c11) / ((c11 + c10) * (c11 + c01) + 1)"

BINARY = "./target/release/canonical"

# Epoch 2 tasks: attack the evolved rule's specific weaknesses
# 1. c11=0 edge case: denominator = 2*(c10+c01) + 1 — what score does a never-co-occurring pair get?
# 2. The 1/(c11+1) regularizer shrinks as c11 grows — is that useful or noise?
# 3. 2*(c10+c01) vs multiplicative (c10+c01)^2 — which penalizes asymmetry better?
# 4. Large-t behavior — does the rule degrade when the corpus grows?
# 5. PMI comparison — is the evolved rule better than log(c11*t / (c10*c01))?
# 6. Robustness: two rules that rank identically on small corpus diverge on large
# 7. The selection predicate needs tightening once the rule is validated
TASKS = [
    "The current rule is (c11*c11)/(c11 + 2*(c10+c01) + 1/(c11+1)). Compute the score for a pair with c11=0, c10=5, c01=5. What does this score mean semantically? Is it correct that a never-co-occurring pair gets a non-zero score? If this is a defect, emit a corrected define_scoring_rule.",

    "The 1/(c11+1) term in the denominator adds a small value that shrinks as c11 grows: 1.0 when c11=0, 0.5 when c11=1, 0.1 when c11=9. What is the intended effect of this term? Does it actually change any ranking decisions compared to removing it? If it is purely cosmetic, simplify the rule and emit define_scoring_rule.",

    "Compare these two denominator designs: (A) 2*(c10+c01) — current rule. (B) (c10+1)*(c01+1) — multiplicative. For the pair c11=10, c10=8, c01=1: A gives denominator=18, B gives denominator=18. For c11=10, c10=4, c01=4: A gives 16, B gives 25. Which penalizes asymmetric marginals more consistently? Emit a define_scoring_rule with the better design.",

    "The current rule has no dependence on t (total turns observed). This means a pair seen in t=5 turns and a pair seen in t=500 turns get the same score if their c11, c10, c01 are identical. Is this a problem for hypothesis selection quality? Emit a define_scoring_rule that incorporates t if and only if it improves the semantics.",

    "Pointwise Mutual Information: PMI(a,b) = log(P(a,b) / (P(a)*P(b))) = log(c11*t / ((c11+c10)*(c11+c01))). Compare PMI to the current rule on: (A) c11=10, c10=0, c01=0, t=10. (B) c11=10, c10=9, c01=9, t=100. Show the arithmetic for both. Which rule produces a more meaningful ranking? Emit define_scoring_rule with your verdict.",

    "The current rule rewards c11^2 in the numerator. This means doubling c11 quadruples the score. Is superlinear reward for co-occurrence count correct, or does it over-weight high-frequency pairs? Design a case where c11^2 produces a wrong ranking. If you find one, emit a corrected define_scoring_rule. If c11^2 is correct, defend it and emit it unchanged.",

    "The epoch-1 corpus had 18 turns. Imagine the corpus grows to 500 turns. A pair (A,B) maintains c11/t=0.6 throughout — at t=18 it has c11=11, at t=500 it has c11=300. The current rule scores both at roughly (c11^2)/(c11). Does the score grow linearly with c11? Is that the right behavior for a growing corpus? Emit a define_scoring_rule that is robust to corpus growth.",

    "A selection predicate that passes score > 0 will pass nearly every pair with any c11. Design a predicate that: (1) requires minimum c11>=3 for statistical credibility, (2) requires precision floor c11/(c11+c10) > 0.5, (3) requires recall floor c11/(c11+c01) > 0.5. Emit define_selection_predicate with all three conditions.",

    "The predicate from the previous turn has three conditions. A pair with c11=3, c10=2, c01=0 passes conditions 1 and 3 but fails condition 2 (precision=0.6 > 0.5 — actually passes). Verify: does this pair pass all three conditions? Show the arithmetic. Then test the hardest case: c11=3, c10=3, c01=3. Does it pass? Should it? Adjust and emit define_selection_predicate.",

    "You have now run 9 turns in epoch 2. The scoring rule has evolved across two epochs. State what the rule is now optimizing for in plain language — not math, not code. What is the intuition? Then: is that intuition correct given the adversarial tasks you have seen? If the intuition is wrong, emit a corrected define_scoring_rule that matches the intended semantics.",

    "Two rules: (A) current evolved rule. (B) (c11 / (c11 + c10 + 1)) * (c11 / (c11 + c01 + 1)) — the product of precision and recall. Rule B is F1-squared. Compare on: c11=5, c10=5, c01=0 and c11=5, c10=0, c01=5. Which rule treats these two cases more symmetrically? Emit define_scoring_rule with the more principled design.",

    "The scoring rule you emit gets applied to every edge in the graph at selection time. But not all edges are hypothesis edges — some are structural anchors with very high c11 because they appear in every turn by construction. Should the scoring rule treat structural anchors differently? If yes, how? Emit define_scoring_rule that handles this case.",

    "Run an adversarial attack on the current selection predicate: construct a pair that scores high under the rule but should NOT be selected as a meaningful hypothesis — something that is a spurious artifact of the task set. Describe the pair and why it is spurious. Then emit define_selection_predicate that would reject it.",

    "The scoring rule now spans two epochs of evolution. Compare the epoch-1 starting rule (c11*c11)/((c11+c10)*(c11+c01)+1) to the current rule side by side. For which specific pair configurations does the current rule produce a different ranking? Give a concrete numerical example where they diverge. Emit define_scoring_rule confirming the current rule if it is better, or reverting if it is not.",

    "Fitness signal audit: in this system, what is the actual signal that causes the rule to improve? Is it the gate passing/failing? The graph nodes being confirmed? The adversarial tasks? If the fitness signal is implicit or vague, emit define_scoring_rule with a rule that makes the fitness signal explicit — embedding it in the score formula itself.",

    "The selection predicate currently operates on score, c11, c10, c01. It does not use t. A pair observed only twice (t=2, c11=2, c10=0) has perfect co-occurrence but near-zero statistical power. Emit define_selection_predicate that adds a minimum-t floor, justified by what t you chose and why.",

    "Across two epochs, the rule has mutated multiple times. What is the trajectory? Did each mutation make the rule strictly better, or were some sideways moves? Given the trajectory, what is the predicted form of the rule after a third epoch of adversarial evolution? Emit define_scoring_rule as the best current candidate to enter epoch 3.",

    "Final epoch-2 summary. Emit define_scoring_rule and define_selection_predicate as your closing operations for epoch 2. Then state: (1) what the held-out P@10 of this rule should be vs the epoch-1 frozen rule — give a specific number, not a range. (2) what the single weakest remaining assumption in the rule is. (3) the seed tension for epoch 3.",
]


def run_turn(prompt: str, extra_args: list[str] = []) -> dict:
    cmd = [BINARY] + extra_args + [prompt]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    output = result.stdout + result.stderr
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
    print(f"\n  {label}: {commits} Commits, {rollbacks} Rollbacks")
    return results


def compute_metrics(receipts_file: str, output_file: str) -> dict:
    result = subprocess.run(
        ["python3", "scripts/epoch_metrics.py",
         "--receipts-file", receipts_file,
         "--output", output_file,
         "--skip-replay"],
        capture_output=True, text=True, timeout=60,
    )
    try:
        return json.loads(Path(output_file).read_text())
    except Exception:
        return {}


def held_out_eval(receipts_file: str, *rules: str) -> list[dict]:
    result = subprocess.run(
        ["python3", "scripts/heldout_eval.py",
         "--receipts", receipts_file,
         "--k", "10", "--split", "0.6",
         *rules],
        capture_output=True, text=True, timeout=60,
    )
    print(result.stdout)
    return result.stdout


def extract_final_rule(receipts_file: str) -> str:
    """Get the last emitted scoring rule from a receipts file."""
    try:
        receipts = [json.loads(l) for l in Path(receipts_file).read_text().splitlines() if l.strip()]
        for r in reversed(receipts):
            ops = r.get("recorded_proposal", {}).get("ovm_ops", [])
            for op in reversed(ops):
                if isinstance(op, dict) and "scoring_rule" in op.get("operation", ""):
                    return op.get("rule", "")
    except Exception:
        pass
    return ""


def main():
    # Run epoch 2 with the LM starting from a clean state (same task set, evolved rule as context)
    run_epoch(
        "EPOCH 2 LM — evolved rule under adversarial pressure",
        state_file="lm_e2_state.json",
        receipts_file="lm_e2_receipts.jsonl",
    )

    # Run epoch 2 baseline — frozen rule against same tasks
    run_epoch(
        "EPOCH 2 BASELINE — frozen rule against same tasks",
        state_file="baseline_e2_state.json",
        receipts_file="baseline_e2_receipts.jsonl",
        extra_args=[f"--frozen-rule={FROZEN_RULE}"],
    )

    # Compute metrics
    print("\n[metrics] computing epoch 2 metrics...")
    lm_e2 = compute_metrics("lm_e2_receipts.jsonl", "lm_e2_metrics.json")
    base_e2 = compute_metrics("baseline_e2_receipts.jsonl", "baseline_e2_metrics.json")

    # Extract evolved rule from epoch 2
    lm_rule_e2 = extract_final_rule("lm_e2_receipts.jsonl") or LM_RULE_E1

    print(f"\nEpoch-2 LM final rule: {lm_rule_e2}")

    # Held-out eval comparing all three rules on combined receipts
    # Use epoch 2 data (larger corpus) for the evaluation
    print("\n=== Held-out eval on epoch 2 data ===")
    held_out_eval(
        "lm_e2_receipts.jsonl",
        lm_rule_e2,
        LM_RULE_E1,
        FROZEN_RULE,
    )

    # Cross-epoch comparison table
    print("\n" + "="*65)
    print("  EPOCH 2 COMPARISON — LM-evolved vs frozen baseline")
    print("="*65)

    def m(d, *keys):
        v = d.get("metrics", {})
        for k in keys:
            v = v.get(k, {}) if isinstance(v, dict) else {}
        return v

    rows = [
        ("total turns",
         lm_e2.get("total_turns"), base_e2.get("total_turns")),
        ("hypothesis precision",
         m(lm_e2, "hypothesis_precision", "precision"),
         m(base_e2, "hypothesis_precision", "precision")),
        ("false positive rate",
         m(lm_e2, "false_positive_rate", "false_positive_rate"),
         m(base_e2, "false_positive_rate", "false_positive_rate")),
        ("contradiction slope",
         m(lm_e2, "contradiction_slope", "slope"),
         m(base_e2, "contradiction_slope", "slope")),
        ("rule mutations",
         m(lm_e2, "operator_delta", "rule_mutations"),
         m(base_e2, "operator_delta", "rule_mutations")),
        ("total violations",
         m(lm_e2, "failure_motif_reduction", "total_violations"),
         m(base_e2, "failure_motif_reduction", "total_violations")),
        ("chain intact",
         m(lm_e2, "receipt_chain_integrity", "intact"),
         m(base_e2, "receipt_chain_integrity", "intact")),
    ]

    print(f"\n{'Metric':<30} {'LM epoch 2':>15} {'Frozen epoch 2':>15}")
    print("-" * 62)
    for name, lv, bv in rows:
        def fmt(v):
            if v is None or v == {}:
                return "n/a"
            if isinstance(v, float):
                return f"{v:.4f}"
            return str(v)
        print(f"{name:<30} {fmt(lv):>15} {fmt(bv):>15}")

    print(f"\nEpoch-1 frozen rule:   {FROZEN_RULE}")
    print(f"Epoch-1 LM rule:       {LM_RULE_E1}")
    print(f"Epoch-2 LM final rule: {lm_rule_e2}")


if __name__ == "__main__":
    main()
