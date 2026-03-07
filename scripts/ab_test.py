#!/usr/bin/env python3
"""A/B test: frozen human baseline vs LM-authored scoring rules.

Runs the same epoch twice — once with a fixed human-written rule, once letting
the LM author its own rules — then computes the 8 canonical metrics for each
condition and prints a side-by-side comparison table.

Usage:
  python3 scripts/ab_test.py
  python3 scripts/ab_test.py --baseline-rule "c11 / (c10 + c01 + 1)"
  python3 scripts/ab_test.py --tasks-file epoch_tasks_adversarial.txt
  python3 scripts/ab_test.py --skip-replay
  python3 scripts/ab_test.py --compare-only   # skip runs, compare existing files
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

# ── File paths ─────────────────────────────────────────────────────────────────

BASELINE_STATE    = Path("baseline_state.json")
BASELINE_RECEIPTS = Path("baseline_receipts.jsonl")
BASELINE_SUMMARY  = Path("baseline_summary.tsv")
BASELINE_METRICS  = Path("baseline_metrics.json")

LM_STATE    = Path("nstar_canonical_state.json")
LM_RECEIPTS = Path("canonical_receipts.jsonl")
LM_SUMMARY  = Path("epoch1_summary.tsv")
LM_METRICS  = Path("lm_metrics.json")

DEFAULT_BASELINE_RULE = "c11 / (c10 + c01 + 1)"

SEP = "━" * 72


# ── Helpers ───────────────────────────────────────────────────────────────────

def run(cmd: list[str], label: str) -> None:
    print(f"\n{SEP}")
    print(f"  {label}")
    print(SEP)
    result = subprocess.run(cmd)
    if result.returncode != 0:
        print(f"  WARNING: process exited {result.returncode}", file=sys.stderr)


def load_metrics(path: Path) -> dict:
    if not path.exists():
        return {}
    return json.loads(path.read_text())


def fmt(val, precision: int = 4) -> str:
    if val is None:
        return "—"
    if isinstance(val, bool):
        return "✓" if val else "✗"
    if isinstance(val, float):
        return f"{val:.{precision}f}"
    return str(val)


# ── Comparison logic ──────────────────────────────────────────────────────────

MetricSpec = tuple[str, str, bool]  # (display_name, json_path, higher_is_better)

METRICS: list[MetricSpec] = [
    ("hypothesis_precision",  "hypothesis_precision.precision",          True),
    ("false_positive_rate",   "false_positive_rate.false_positive_rate", False),
    ("contradiction_slope",   "contradiction_slope.slope",               False),
    ("rule_mutations",        "operator_delta.rule_mutations",           True),   # LM > baseline expected
    ("predicate_mutations",   "operator_delta.predicate_mutations",      True),   # LM > baseline expected
    ("fitness_delta",         "operator_fitness_delta.fitness_delta",    True),
    ("late_quality",          "operator_fitness_delta.late_mean_quality",True),
    ("chain_intact",          "receipt_chain_integrity.intact",          True),
]


def dig(obj: dict, path: str):
    """Dot-path lookup into nested dict."""
    cur = obj
    for key in path.split("."):
        if not isinstance(cur, dict):
            return None
        cur = cur.get(key)
    return cur


def winner(bval, lval, higher_is_better) -> str:
    if higher_is_better is None:
        return ""
    try:
        b, l = float(bval), float(lval)
        if abs(b - l) < 1e-6:
            return "tie"
        if higher_is_better:
            return "baseline" if b > l else "lm"
        else:
            return "baseline" if b < l else "lm"
    except (ValueError, TypeError):
        if bval == "✓" and lval != "✓":
            return "baseline"
        if lval == "✓" and bval != "✓":
            return "lm"
        return ""


def compare(baseline: dict, lm: dict, baseline_rule: str) -> None:
    bm = baseline.get("metrics", {})
    lm_ = lm.get("metrics", {})
    bt = baseline.get("total_turns", "?")
    lt = lm.get("total_turns", "?")

    rows: list[tuple[str, str, str, str]] = [
        ("metric", "baseline (frozen)", "lm-authored", "winner"),
        ("-" * 24, "-" * 17, "-" * 17, "-" * 8),
        ("turns", str(bt), str(lt), ""),
    ]

    wins = {"baseline": 0, "lm": 0}
    for name, path, hib in METRICS:
        bval_raw = dig(bm, path)
        lval_raw = dig(lm_, path)
        bval = fmt(bval_raw)
        lval = fmt(lval_raw)
        w = winner(bval, lval, hib)
        if w in wins:
            wins[w] += 1
        rows.append((name, bval, lval, w))

    widths = [max(len(r[c]) for r in rows) for c in range(4)]
    for row in rows:
        print("  ".join(cell.ljust(widths[c]) for c, cell in enumerate(row)))

    total_scored = wins["baseline"] + wins["lm"]
    print()
    print(f"  baseline rule : {baseline_rule}")
    print(f"  LM wins       : {wins['lm']} / {total_scored}")
    print(f"  baseline wins : {wins['baseline']} / {total_scored}")

    if wins["lm"] > wins["baseline"]:
        edge = wins["lm"] - wins["baseline"]
        print(f"\n  → LM-authored rules win by {edge}. Run show_disagreement.py to see where they differ.")
    elif wins["baseline"] > wins["lm"]:
        edge = wins["baseline"] - wins["lm"]
        print(f"\n  → Baseline rule wins by {edge}. Check trajectory for why LM rules underperformed.")
    else:
        print("\n  → Tied. Neither condition clearly dominates.")


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-rule", default=DEFAULT_BASELINE_RULE,
                        help="Frozen scoring rule for baseline condition")
    parser.add_argument("--tasks-file", default="epoch1_tasks.txt",
                        help="Task file to use for both conditions")
    parser.add_argument("--skip-replay", action="store_true",
                        help="Skip replay determinism metric (faster)")
    parser.add_argument("--compare-only", action="store_true",
                        help="Skip epoch runs; compare existing metric files")
    args = parser.parse_args()

    skip_replay = ["--skip-replay"] if args.skip_replay else []

    if not args.compare_only:
        # ── Condition A: frozen baseline rule ─────────────────────────────────
        run([
            "python3", "run_epoch1.py",
            "--frozen-rule", args.baseline_rule,
            "--state-file",    str(BASELINE_STATE),
            "--receipts-file", str(BASELINE_RECEIPTS),
            "--summary-file",  str(BASELINE_SUMMARY),
            "--tasks-file",    args.tasks_file,
        ], f"CONDITION A — baseline frozen rule  [{args.baseline_rule}]")

        run([
            "python3", "scripts/epoch_metrics.py",
            "--receipts-file", str(BASELINE_RECEIPTS),
            "--output", str(BASELINE_METRICS),
            "--trajectory-file", "baseline_rule_trajectory.jsonl",
        ] + skip_replay, "Computing baseline metrics (8 metrics)")

        # ── Condition B: LM-authored rules ────────────────────────────────────
        run([
            "python3", "run_epoch1.py",
            "--tasks-file", args.tasks_file,
        ], "CONDITION B — LM-authored scoring rules")

        run([
            "python3", "scripts/epoch_metrics.py",
            "--receipts-file", str(LM_RECEIPTS),
            "--output", str(LM_METRICS),
            "--trajectory-file", "rule_trajectory.jsonl",
        ] + skip_replay, "Computing LM metrics (8 metrics)")

    # ── Comparison ────────────────────────────────────────────────────────────
    baseline_metrics = load_metrics(BASELINE_METRICS)
    lm_metrics = load_metrics(LM_METRICS)

    if not baseline_metrics or not lm_metrics:
        print("ERROR: metric files not found. Run without --compare-only first.",
              file=sys.stderr)
        sys.exit(1)

    print(f"\n{SEP}")
    print("  A/B COMPARISON — baseline (frozen) vs LM-authored scoring rules")
    print(SEP)
    compare(baseline_metrics, lm_metrics, args.baseline_rule)
    print()


if __name__ == "__main__":
    main()
