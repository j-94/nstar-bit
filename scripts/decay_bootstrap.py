#!/usr/bin/env python3
"""
decay_bootstrap.py — one-time migration to apply support_set-derived
confidence to all live relations in a state JSON.

Before this pass, relations enter the graph at the confidence the LM declared
(often 1.0) regardless of whether any evidence backs them. This creates the
"hardening pathology": beliefs that survive by not being challenged, not by
being supported.

After this pass, confidence reflects actual evidence:
  - evidence_for=0, evidence_against=0  →  confidence = 0.0  (unknown)
  - evidence_for>0                      →  raw ratio + 0.1 floor bonus
  - archived                            →  unchanged (already dead)

This is run ONCE against the epoch7 fork to produce a clean baseline.
All future relations are governed by the support_set primitive in autogenesis.rs.

Usage:
    python3 scripts/decay_bootstrap.py \\
        --input  epoch_logs/epoch7_fork.json \\
        --output epoch_logs/epoch8_bootstrap.json

    # dry-run (print stats, don't write):
    python3 scripts/decay_bootstrap.py \\
        --input  epoch_logs/epoch7_fork.json \\
        --dry-run
"""

import argparse
import json
import time


def now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def compute_confidence(ev_for: int, ev_against: int) -> float:
    """
    Derive confidence from evidence counts alone.
    Empty support (both zero) → 0.0, not 1.0.
    Non-empty support → raw Bayesian ratio + 0.1 floor bonus (matches autogenesis.rs).
    """
    if ev_for == 0 and ev_against == 0:
        return 0.0
    total = ev_for + ev_against
    raw = ev_for / total
    floor_bonus = 0.1 if ev_for > 0 else 0.0
    return min(raw + floor_bonus, 1.0)


def run_bootstrap(state: dict, dry_run: bool = False) -> dict:
    relations = state.get("relations", {})
    if not isinstance(relations, dict):
        raise ValueError("state.relations must be a dict")

    stats = {
        "total": 0,
        "archived_skipped": 0,
        "hardened_reset": 0,       # was c≥0.95, evidence_for=0
        "evidenced_recomputed": 0, # had evidence, confidence updated
        "unchanged": 0,            # already at 0.0 or consistent
        "confidence_delta_sum": 0.0,
    }

    for rel_id, rel in relations.items():
        stats["total"] += 1

        if rel.get("status") == "archived":
            stats["archived_skipped"] += 1
            continue

        ev_for = int(rel.get("evidence_for", 0))
        ev_against = int(rel.get("evidence_against", 0))
        old_conf = float(rel.get("confidence", 0.0))
        new_conf = compute_confidence(ev_for, ev_against)

        delta = abs(new_conf - old_conf)
        stats["confidence_delta_sum"] += delta

        if old_conf >= 0.95 and ev_for == 0:
            stats["hardened_reset"] += 1
        elif ev_for > 0:
            stats["evidenced_recomputed"] += 1
        else:
            stats["unchanged"] += 1

        if not dry_run:
            rel["confidence"] = round(new_conf, 4)
            # support_set already defaults to [] via serde; make it explicit
            if "support_set" not in rel:
                rel["support_set"] = []

    return stats


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--input",  required=True, help="Input state JSON path")
    parser.add_argument("--output", default=None,  help="Output state JSON path (omit for dry-run)")
    parser.add_argument("--dry-run", action="store_true", help="Print stats only, no write")
    args = parser.parse_args()

    dry_run = args.dry_run or args.output is None

    with open(args.input) as f:
        state = json.load(f)

    stats = run_bootstrap(state, dry_run=dry_run)

    live = stats["total"] - stats["archived_skipped"]
    avg_delta = stats["confidence_delta_sum"] / max(live, 1)

    print("── decay_bootstrap ──────────────────────────────────")
    print(f"  input:               {args.input}")
    print(f"  mode:                {'DRY RUN' if dry_run else 'WRITE → ' + args.output}")
    print()
    print(f"  total relations:     {stats['total']}")
    print(f"  archived (skipped):  {stats['archived_skipped']}")
    print(f"  live processed:      {live}")
    print()
    print(f"  hardened → reset:    {stats['hardened_reset']}")
    print(f"    (conf≥0.95, evidence_for=0, now → 0.0)")
    print(f"  evidenced recomputed:{stats['evidenced_recomputed']}")
    print(f"    (raw ratio + 0.1 floor, matches new autogenesis.rs)")
    print(f"  already at 0:        {stats['unchanged']}")
    print()
    print(f"  avg confidence Δ:    {avg_delta:.4f}  (across all live)")
    print(f"  total confidence Δ:  {stats['confidence_delta_sum']:.4f}")

    if not dry_run:
        state["updated_at"] = now_iso()
        with open(args.output, "w") as f:
            json.dump(state, f, indent=2, ensure_ascii=True)
        print()
        print(f"  written:             {args.output}")

    print("─────────────────────────────────────────────────────")


if __name__ == "__main__":
    main()
