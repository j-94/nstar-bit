#!/usr/bin/env python3
"""Held-out prediction evaluator for scoring rules.

Splits the receipt chain in half by turn. Recomputes c11/c10/c01/c00
from the first half only. Ranks edges under each rule. Measures
precision@K and recall@K against co-occurrences observed in the second half.

Usage:
  python3 scripts/heldout_eval.py "c11 / (c10 + c01 + 1)"
  python3 scripts/heldout_eval.py rule_a rule_b rule_c ...
  python3 scripts/heldout_eval.py --receipts baseline_receipts.jsonl --k 20 "rule"
  python3 scripts/heldout_eval.py --split 0.6 "rule"   # use 60% for training
"""

import argparse
import itertools
import json
import math
from collections import defaultdict
from pathlib import Path


def score_edge(rule: str, c11, c10, c01, c00) -> float:
    t = max(c11 + c10 + c01 + c00, 1)
    try:
        return float(eval(rule, {"__builtins__": {}}, {
            "c11": float(c11), "c10": float(c10),
            "c01": float(c01), "c00": float(c00), "t": float(t),
            "log": math.log, "sqrt": math.sqrt,
            "abs": abs, "max": max, "min": min,
        }))
    except Exception:
        return 0.0


def extract_active_nodes(receipt: dict) -> set:
    """Extract Token-scale named nodes only (not numeric primes, not Turn/Session/Project)."""
    nodes = set()
    for coord in receipt.get("coordinates", []):
        if coord.get("scale") != "Token":
            continue
        for name in coord.get("active_nodes", []):
            s = str(name)
            # Skip bare numbers (coordinate primes)
            if s.isdigit():
                continue
            nodes.add(s)
    return nodes


def build_counts(turns: list[dict]) -> dict:
    """Build per-pair contingency counts from a list of receipts.

    For each pair (A, B) where A < B lexicographically:
      c11: both active
      c10: A active, B not
      c01: A not, B active
      c00: neither active
    """
    # Collect all nodes seen across these turns
    all_nodes = set()
    per_turn_active = []
    for r in turns:
        active = extract_active_nodes(r)
        per_turn_active.append(active)
        all_nodes.update(active)

    all_nodes = sorted(all_nodes)
    counts = defaultdict(lambda: {"c11": 0, "c10": 0, "c01": 0, "c00": 0})

    for active in per_turn_active:
        active_set = set(active)
        for i, a in enumerate(all_nodes):
            for b in all_nodes[i + 1:]:
                a_on = a in active_set
                b_on = b in active_set
                key = (a, b)
                if a_on and b_on:
                    counts[key]["c11"] += 1
                elif a_on:
                    counts[key]["c10"] += 1
                elif b_on:
                    counts[key]["c01"] += 1
                else:
                    counts[key]["c00"] += 1

    return counts


def precision_recall_at_k(train_counts: dict, test_counts: dict,
                           rule: str, k: int) -> tuple[float, float, int]:
    """
    Score edges using train counts, rank, then measure against test counts.
    A pair is 'positive' in test if c11 > 0 in test turns.
    Returns (precision@k, recall@k, total_test_positives).
    """
    # Only score pairs with any signal in training
    candidates = {pair: c for pair, c in train_counts.items() if c["c11"] > 0}
    if not candidates:
        return 0.0, 0.0, 0

    scored = []
    for pair, c in candidates.items():
        s = score_edge(rule, c["c11"], c["c10"], c["c01"], c["c00"])
        scored.append((s, pair))
    scored.sort(key=lambda x: -x[0])

    test_positives = {pair for pair, c in test_counts.items() if c["c11"] > 0}
    total_positives = len(test_positives)

    top_k = [pair for _, pair in scored[:k]]
    hits = sum(1 for pair in top_k if pair in test_positives)

    precision = hits / k if k > 0 else 0.0
    recall = hits / total_positives if total_positives > 0 else 0.0

    return precision, recall, total_positives


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rules", nargs="+")
    p.add_argument("--receipts", default="baseline_receipts.jsonl")
    p.add_argument("--k", type=int, default=10, help="Top-K for precision/recall")
    p.add_argument("--split", type=float, default=0.5, help="Train fraction")
    p.add_argument("--min-train-c11", type=int, default=1)
    args = p.parse_args()

    receipts_path = Path(args.receipts)
    if not receipts_path.exists():
        # fallback
        receipts_path = Path("canonical_receipts.jsonl")
    if not receipts_path.exists():
        print("No receipts file found.")
        return

    receipts = [json.loads(l) for l in receipts_path.read_text().splitlines() if l.strip()]
    receipts.sort(key=lambda r: r["turn"])
    n = len(receipts)
    split_at = max(1, int(n * args.split))

    train = receipts[:split_at]
    test = receipts[split_at:]

    print(f"Receipts: {n} total  |  train={len(train)} turns  test={len(test)} turns  (split={args.split})")
    print(f"Building co-occurrence counts from {len(train)} training turns...")
    train_counts = build_counts(train)
    print(f"Building co-occurrence counts from {len(test)} test turns...")
    test_counts = build_counts(test)

    train_pos = sum(1 for c in train_counts.values() if c["c11"] >= args.min_train_c11)
    test_pos = sum(1 for c in test_counts.values() if c["c11"] > 0)
    print(f"Train pairs with c11>={args.min_train_c11}: {train_pos}")
    print(f"Test positives (c11>0 in test): {test_pos}")
    print()

    print(f"{'Rule':<55} {'P@'+str(args.k):>6} {'R@'+str(args.k):>6}  {'F1':>6}")
    print("-" * 78)

    results = []
    for rule in args.rules:
        p_at_k, r_at_k, _ = precision_recall_at_k(train_counts, test_counts, rule, args.k)
        f1 = 2 * p_at_k * r_at_k / (p_at_k + r_at_k) if (p_at_k + r_at_k) > 0 else 0.0
        results.append((f1, p_at_k, r_at_k, rule))
        label = rule if len(rule) <= 55 else rule[:52] + "..."
        print(f"{label:<55} {p_at_k:>6.3f} {r_at_k:>6.3f}  {f1:>6.3f}")

    results.sort(reverse=True)
    print()
    print(f"Winner: {results[0][3]}")
    print(f"  P@{args.k}={results[0][1]:.3f}  R@{args.k}={results[0][2]:.3f}  F1={results[0][0]:.3f}")


if __name__ == "__main__":
    main()
