#!/usr/bin/env python3
"""Offline rule evaluator — no LM calls, no API, no credits.

Load saved graph state, apply any evalexpr-compatible rule to all hypothesis
edges, rank and diff. Milliseconds.

Usage:
  python3 scripts/eval_rule.py "c11 / (c10 + c01 + 1)"
  python3 scripts/eval_rule.py "c11 / (c10 + c01 + 1)" "(c11*c11) / ((c11+c10)*(c11+c01)+1)"
  python3 scripts/eval_rule.py --state baseline_state.json "c11 / (c10+c01+1)"
  python3 scripts/eval_rule.py --top 20 "c11 / (c10 + c01 + 1)"
"""

import argparse
import json
import math
import sys
from pathlib import Path


def score_edge(rule: str, e: dict) -> float:
    c11 = float(e.get("c11", 0))
    c10 = float(e.get("c10", 0))
    c01 = float(e.get("c01", 0))
    c00 = float(e.get("c00", 0))
    t   = c11 + c10 + c01 + c00
    score = 0.0
    try:
        score = eval(rule, {"__builtins__": {}}, {
            "c11": c11, "c10": c10, "c01": c01, "c00": c00, "t": max(t, 1),
            "log": math.log, "sqrt": math.sqrt, "abs": abs, "max": max, "min": min,
        })
    except Exception:
        pass
    return float(score)


def rank(edges: list, rule: str) -> list:
    scored = [(score_edge(rule, e), e) for e in edges]
    scored.sort(key=lambda x: -x[0])
    return scored


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rules", nargs="+", help="One or two rule expressions to evaluate/compare")
    p.add_argument("--state", default="baseline_state.json", help="Graph state JSON file")
    p.add_argument("--top", type=int, default=15, help="Top N edges to show")
    p.add_argument("--min-c11", type=int, default=1, help="Min c11 to include edge")
    args = p.parse_args()

    state_path = Path(args.state)
    if not state_path.exists():
        # try lm state
        state_path = Path("nstar_canonical_state.json")
    if not state_path.exists():
        print("No state file found. Run a session first to accumulate counts.", file=sys.stderr)
        sys.exit(1)

    state = json.loads(state_path.read_text())
    all_edges = state["graph"]["edges"]
    hyp = [e for e in all_edges if e.get("kind") == "Hypothesis" and e.get("c11", 0) >= args.min_c11]

    print(f"State: {state_path}  |  {len(hyp)} hypothesis edges (c11 >= {args.min_c11})")
    print(f"Turns: {state.get('turn_count', '?')}")
    print()

    rule_a = args.rules[0]
    rule_b = args.rules[1] if len(args.rules) > 1 else None

    ranked_a = rank(hyp, rule_a)

    if rule_b is None:
        # Single rule: show top N
        print(f"Rule: {rule_a}")
        print(f"{'Score':>10}  {'c11':>4} {'c10':>4} {'c01':>4} {'c00':>4}  Edge")
        print("-" * 72)
        for score, e in ranked_a[:args.top]:
            print(f"{score:>10.4f}  {e.get('c11',0):>4} {e.get('c10',0):>4} {e.get('c01',0):>4} {e.get('c00',0):>4}  {e['from']} → {e['to']}")
    else:
        # Two rules: diff
        ranked_b = rank(hyp, rule_b)
        rank_a = {e["from"] + "→" + e["to"]: i for i, (_, e) in enumerate(ranked_a)}
        rank_b = {e["from"] + "→" + e["to"]: i for i, (_, e) in enumerate(ranked_b)}

        # Edges where ranking changed most
        diffs = []
        for key, ra in rank_a.items():
            rb = rank_b.get(key, ra)
            diffs.append((rb - ra, key))  # positive = moved up under B
        diffs.sort(key=lambda x: -abs(x[0]))

        print(f"Rule A: {rule_a}")
        print(f"Rule B: {rule_b}")
        print()
        print(f"Biggest ranking changes (+ = moved up under B, - = moved down):")
        print(f"{'Δrank':>8}  {'rA':>6}  {'rB':>6}  {'c11':>4} {'c10':>4} {'c01':>4}  Edge")
        print("-" * 80)
        for delta, key in diffs[:args.top]:
            ra = rank_a[key]
            rb = rank_b.get(key, ra)
            frm, to = key.split("→", 1)
            edge = next(e for e in hyp if e["from"] == frm and e["to"] == to)
            score_a = score_edge(rule_a, edge)
            score_b = score_edge(rule_b, edge)
            print(f"{delta:>+8}  {ra:>6}  {rb:>6}  {edge.get('c11',0):>4} {edge.get('c10',0):>4} {edge.get('c01',0):>4}  {key}  [{score_a:.3f} → {score_b:.3f}]")

        # Summary stats
        print()
        scores_a = [s for s, _ in ranked_a]
        scores_b = [s for s, _ in ranked_b]
        mean_a = sum(scores_a) / len(scores_a)
        mean_b = sum(scores_b) / len(scores_b)
        top10_a = set(e["from"]+"→"+e["to"] for _, e in ranked_a[:10])
        top10_b = set(e["from"]+"→"+e["to"] for _, e in ranked_b[:10])
        print(f"Mean score      A={mean_a:.4f}  B={mean_b:.4f}")
        print(f"Top-10 overlap  {len(top10_a & top10_b)}/10 edges in common")
        print(f"Top-10 A only:  {top10_a - top10_b}")
        print(f"Top-10 B only:  {top10_b - top10_a}")


if __name__ == "__main__":
    main()
