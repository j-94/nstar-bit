#!/usr/bin/env python3
"""Show per-edge scoring disagreements between two rules.

Reads hypothesis edges from the canonical state, evaluates each edge under
both rules, and prints a table showing where the rules disagree — different
relative ranking, or one promotes while the other demotes.

Variables available in scoring expressions:
  c11  — co-activation AND reinforcement
  c10  — co-activation WITHOUT reinforcement
  c01  — no co-activation WITH reinforcement
  c00  — neither
  t    — total turns observed

Usage:
  python3 scripts/show_disagreement.py
  python3 scripts/show_disagreement.py --state-file nstar_canonical_state.json
  python3 scripts/show_disagreement.py \
      --rule-a "c11 / (c10 + c01 + 1)" \
      --rule-b "c11 / (c10 + c01 + 1) * (1 - c00 / (t + 1))"
  python3 scripts/show_disagreement.py --all-edges   # include non-hypothesis edges
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path


# ── Safe expression evaluator ─────────────────────────────────────────────────

_SAFE_NAMES = {
    "log": math.log,
    "log2": math.log2,
    "log10": math.log10,
    "exp": math.exp,
    "sqrt": math.sqrt,
    "abs": abs,
    "min": min,
    "max": max,
}


def eval_rule(expr: str, c11: float, c10: float, c01: float, c00: float, t: float) -> float | None:
    """Evaluate an evalexpr-style scoring rule with the given co-occurrence counts."""
    if not expr or not expr.strip():
        return None
    env = {"c11": c11, "c10": c10, "c01": c01, "c00": c00, "t": t, **_SAFE_NAMES}
    try:
        result = eval(compile(expr, "<rule>", "eval"), {"__builtins__": {}}, env)  # noqa: S307
        return float(result)
    except ZeroDivisionError:
        return None
    except Exception:
        return None


# ── Load state ────────────────────────────────────────────────────────────────

def load_edges(state_path: Path, hypothesis_only: bool = True) -> list[dict]:
    data = json.loads(state_path.read_text())
    edges = data.get("graph", {}).get("edges", [])
    if hypothesis_only:
        edges = [e for e in edges if e.get("kind") == "Hypothesis"]
    # Normalise: c11/c10/c01/c00 may be top-level or nested under "hypothesis"
    normalised = []
    for e in edges:
        h = e.get("hypothesis", {})
        c11 = float(e.get("c11", h.get("c11", 0)))
        c10 = float(e.get("c10", h.get("c10", 0)))
        c01 = float(e.get("c01", h.get("c01", 0)))
        c00 = float(e.get("c00", h.get("c00", 0)))
        t   = float(e.get("t",   h.get("t", c11 + c10 + c01 + c00)))
        eid = e.get("id") or f"{e.get('from','?')}→{e.get('to','?')}"
        normalised.append({
            "id": eid,
            "c11": c11, "c10": c10, "c01": c01, "c00": c00, "t": t,
        })
    return normalised


def lm_rule_from_state(state_path: Path) -> str:
    data = json.loads(state_path.read_text())
    return data.get("graph", {}).get("scoring_rule", "")


# ── Formatting ────────────────────────────────────────────────────────────────

def fmt_score(v: float | None) -> str:
    if v is None:
        return "  —  "
    return f"{v:+.4f}"


def rank_by_score(edges: list[dict], rule: str) -> dict[str, int]:
    scored = []
    for e in edges:
        s = eval_rule(rule, e["c11"], e["c10"], e["c01"], e["c00"], e["t"])
        scored.append((e["id"], s))
    scored.sort(key=lambda x: (x[1] is None, -(x[1] or 0)))
    return {eid: rank + 1 for rank, (eid, _) in enumerate(scored)}


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-file", default="nstar_canonical_state.json")
    parser.add_argument("--rule-a", default="c11 / (c10 + c01 + 1)",
                        help="Rule A (baseline / human-authored)")
    parser.add_argument("--rule-b", default=None,
                        help="Rule B (LM-authored; defaults to current rule in state)")
    parser.add_argument("--all-edges", action="store_true",
                        help="Include non-hypothesis edges")
    parser.add_argument("--min-rank-diff", type=int, default=1,
                        help="Only show edges whose rank differs by at least N")
    args = parser.parse_args()

    state_path = Path(args.state_file)
    if not state_path.exists():
        print(f"State file not found: {state_path}", file=sys.stderr)
        sys.exit(1)

    rule_a = args.rule_a
    rule_b = args.rule_b or lm_rule_from_state(state_path)

    if not rule_b:
        print("No LM-authored rule found in state. Run an epoch first "
              "or pass --rule-b explicitly.", file=sys.stderr)
        sys.exit(1)

    edges = load_edges(state_path, hypothesis_only=not args.all_edges)
    if not edges:
        print("No hypothesis edges found in state file.", file=sys.stderr)
        sys.exit(1)

    SEP = "─" * 78

    print(f"\nDisagreement report — {len(edges)} hypothesis edge(s)")
    print(f"  rule A (baseline) : {rule_a}")
    print(f"  rule B (lm)       : {rule_b}")
    print(SEP)

    ranks_a = rank_by_score(edges, rule_a)
    ranks_b = rank_by_score(edges, rule_b)

    rows = []
    for e in edges:
        eid  = e["id"]
        c11  = e["c11"]
        c10  = e["c10"]
        c01  = e["c01"]
        c00  = e["c00"]
        t    = e["t"]

        sa = eval_rule(rule_a, c11, c10, c01, c00, t)
        sb = eval_rule(rule_b, c11, c10, c01, c00, t)
        ra = ranks_a.get(eid, 0)
        rb = ranks_b.get(eid, 0)
        diff = abs(ra - rb)

        rows.append({
            "id": eid,
            "c11": c11, "c10": c10, "c01": c01, "c00": c00, "t": t,
            "score_a": sa, "score_b": sb,
            "rank_a": ra, "rank_b": rb,
            "rank_diff": diff,
        })

    rows.sort(key=lambda r: -r["rank_diff"])

    header = (
        f"{'edge-id':<28}  "
        f"{'c11':>4} {'c10':>4} {'c01':>4} {'t':>4}  "
        f"{'score_A':>8}  {'score_B':>8}  "
        f"{'rA':>3} {'rB':>3} {'Δ':>3}"
    )
    print(header)
    print(SEP)

    shown = 0
    agreed = 0
    for r in rows:
        if r["rank_diff"] < args.min_rank_diff:
            agreed += 1
            continue
        shown += 1
        flag = "▲" if r["rank_b"] < r["rank_a"] else ("▼" if r["rank_b"] > r["rank_a"] else " ")
        print(
            f"{r['id']:<28}  "
            f"{int(r['c11']):>4} {int(r['c10']):>4} {int(r['c01']):>4} {int(r['t']):>4}  "
            f"{fmt_score(r['score_a']):>8}  {fmt_score(r['score_b']):>8}  "
            f"{r['rank_a']:>3} {r['rank_b']:>3} {r['rank_diff']:>3} {flag}"
        )

    print(SEP)
    print(f"  Showing {shown} edge(s) with rank diff ≥ {args.min_rank_diff}.  "
          f"{agreed} edge(s) agreed.")

    if shown == 0:
        print("  Rules agree on all edge rankings. Try --min-rank-diff 0 to see full table.")
    else:
        a_wins = sum(1 for r in rows if r["rank_diff"] >= args.min_rank_diff and r["rank_a"] < r["rank_b"])
        b_wins = sum(1 for r in rows if r["rank_diff"] >= args.min_rank_diff and r["rank_b"] < r["rank_a"])
        print(f"  Rule A ranks higher: {a_wins}  |  Rule B (LM) ranks higher: {b_wins}")

    print()


if __name__ == "__main__":
    main()
