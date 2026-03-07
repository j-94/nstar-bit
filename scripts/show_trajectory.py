#!/usr/bin/env python3
"""Pretty-print rule_trajectory.jsonl — shows how the scoring rule evolved.

Each entry is one LM-authored rule mutation: old rule, new rule, turn number,
hypothesis edge count, mean weight, and the reason the LM gave for the change.

Usage:
  python3 scripts/show_trajectory.py
  python3 scripts/show_trajectory.py --file rule_trajectory.jsonl
  python3 scripts/show_trajectory.py --json     # raw JSON output
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def load(path: Path) -> list[dict]:
    entries = []
    for i, line in enumerate(path.read_text().splitlines(), 1):
        line = line.strip()
        if not line:
            continue
        try:
            entries.append(json.loads(line))
        except json.JSONDecodeError as exc:
            print(f"WARNING: line {i}: {exc}", file=sys.stderr)
    return entries


def diff_tokens(a: str, b: str) -> str:
    """Highlight tokens that changed between two expressions."""
    ta = set(a.split())
    tb = set(b.split())
    added   = tb - ta
    removed = ta - tb
    parts = []
    if removed:
        parts.append(f"  removed : {' '.join(sorted(removed))}")
    if added:
        parts.append(f"  added   : {' '.join(sorted(added))}")
    return "\n".join(parts) if parts else "  (structural rewrite)"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--file", default="rule_trajectory.jsonl",
                        help="Path to rule_trajectory.jsonl")
    parser.add_argument("--json", action="store_true",
                        help="Output raw JSON instead of formatted text")
    args = parser.parse_args()

    path = Path(args.file)
    if not path.exists():
        print(f"Not found: {path}")
        print("rule_trajectory.jsonl is written whenever the LM emits a "
              "define_scoring_rule operation. Run an epoch first.")
        return

    entries = load(path)
    if not entries:
        print("No trajectory entries found.")
        return

    if args.json:
        print(json.dumps(entries, indent=2))
        return

    SEP = "─" * 70

    print(f"Rule trajectory — {len(entries)} mutation(s)  [{path}]")
    print("=" * 70)

    for idx, e in enumerate(entries, 1):
        turn   = e.get("turn", "?")
        old    = e.get("old_rule") or "(none)"
        new    = e.get("new_rule", "")
        n      = e.get("hypothesis_count", 0)
        mw     = e.get("mean_weight", 0.0)
        reason = e.get("reason", "")

        print(f"\n#{idx:>3}  turn={turn}  edges={n}  mean_weight={mw:.3f}")
        print(SEP)
        print(f"  old : {old}")
        print(f"  new : {new}")
        if old != "(none)":
            print(diff_tokens(old, new))
        print(f"  why : {reason[:120]}")

    print(f"\n{SEP}")
    last = entries[-1]
    print(f"Final rule : {last.get('new_rule', '')}")
    print(f"Mutations  : {len(entries)}")

    # Stability analysis: how many turns between mutations?
    if len(entries) >= 2:
        gaps = []
        for i in range(1, len(entries)):
            t0 = entries[i - 1].get("turn", 0)
            t1 = entries[i].get("turn", 0)
            if isinstance(t0, int) and isinstance(t1, int):
                gaps.append(t1 - t0)
        if gaps:
            avg = sum(gaps) / len(gaps)
            print(f"Mean turns between mutations: {avg:.1f}")
            print(f"Min / Max gap: {min(gaps)} / {max(gaps)}")


if __name__ == "__main__":
    main()
