#!/usr/bin/env python3
"""Compute epoch-level metrics from canonical_receipts.jsonl.

Input:  canonical_receipts.jsonl  (default, or --receipts-file PATH)
Output: epoch_001_metrics.json    (default, or --output PATH)

Eight metrics:
  1. hypothesis_precision      promoted nodes confirmed by later turns
  2. false_positive_rate       promoted nodes that die within 5 turns
  3. contradiction_slope       linear regression over contradiction_score
  4. operator_delta            did the scoring rule / criteria change?
  5. operator_fitness_delta    late-epoch coverage minus early-epoch coverage
  6. replay_determinism        cargo run --bin replay result
  7. receipt_chain_integrity   prev_hash chain unbroken?
  8. failure_motif_reduction   repeated violation patterns decreasing?
"""

from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from collections import defaultdict
from pathlib import Path


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def load_receipts(path: Path) -> list[dict]:
    receipts = []
    with path.open() as fh:
        for lineno, line in enumerate(fh, 1):
            line = line.strip()
            if not line:
                continue
            try:
                receipts.append(json.loads(line))
            except json.JSONDecodeError as exc:
                print(f"WARNING: line {lineno} parse error: {exc}", file=sys.stderr)
    receipts.sort(key=lambda r: r.get("turn", 0))
    return receipts


def turn_scale_active(receipt: dict) -> set[str]:
    """Return active_nodes from the Turn-scale coordinate of a receipt."""
    for coord in receipt.get("coordinates", []):
        if coord.get("scale") == "Turn":
            return set(coord.get("active_nodes", []))
    return set()


def linreg(xs: list[float], ys: list[float]) -> dict:
    """Simple OLS linear regression. Returns slope, intercept, r2."""
    n = len(xs)
    if n < 2:
        return {"slope": None, "intercept": None, "r2": None, "n": n}
    sx = sum(xs)
    sy = sum(ys)
    sxy = sum(x * y for x, y in zip(xs, ys))
    sxx = sum(x * x for x in xs)
    syy = sum(y * y for y in ys)
    denom = n * sxx - sx * sx
    if denom == 0:
        return {"slope": 0.0, "intercept": sy / n, "r2": None, "n": n}
    slope = (n * sxy - sx * sy) / denom
    intercept = (sy - slope * sx) / n
    # R²
    ss_tot = syy - sy * sy / n
    ss_res = syy - slope * sxy - intercept * sy
    r2 = 1.0 - ss_res / ss_tot if ss_tot != 0 else None
    return {"slope": round(slope, 6), "intercept": round(intercept, 6),
            "r2": round(r2, 4) if r2 is not None else None, "n": n}


# ---------------------------------------------------------------------------
# Metric 1: Hypothesis precision
# "promoted edges confirmed by later turns"
# A node discovered at turn T is 'confirmed' if it appears as a Turn-scale
# active node in any subsequent turn.
# ---------------------------------------------------------------------------

def metric_hypothesis_precision(receipts: list[dict]) -> dict:
    # Map: node_id -> first turn it was discovered
    discovered: dict[str, int] = {}
    for r in receipts:
        t = r.get("turn", 0)
        for node in r.get("discovered_nodes", []):
            if node not in discovered:
                discovered[node] = t

    # Build set of (node, turn) active events
    active_by_turn: dict[int, set[str]] = {}
    for r in receipts:
        active_by_turn[r.get("turn", 0)] = turn_scale_active(r)

    total = len(discovered)
    if total == 0:
        return {"precision": None, "confirmed": 0, "total_discovered": 0,
                "note": "no discovered nodes"}

    confirmed = 0
    for node, disc_turn in discovered.items():
        for r in receipts:
            t = r.get("turn", 0)
            if t > disc_turn and node in active_by_turn.get(t, set()):
                confirmed += 1
                break

    precision = confirmed / total
    return {
        "precision": round(precision, 4),
        "confirmed": confirmed,
        "total_discovered": total,
    }


# ---------------------------------------------------------------------------
# Metric 2: False positive rate
# "promoted edges that die within 5 turns"
# A node is 'promoted' when it first appears as Turn-scale active.
# It 'dies within 5 turns' if in turns [T+1 .. T+5] it never reappears.
# ---------------------------------------------------------------------------

def metric_false_positive_rate(receipts: list[dict]) -> dict:
    active_by_turn: dict[int, set[str]] = {}
    for r in receipts:
        active_by_turn[r.get("turn", 0)] = turn_scale_active(r)

    all_turns = sorted(active_by_turn.keys())

    # First turn each node appears as active
    first_active: dict[str, int] = {}
    for t in all_turns:
        for node in active_by_turn[t]:
            if node not in first_active:
                first_active[node] = t

    total_promoted = len(first_active)
    if total_promoted == 0:
        return {"false_positive_rate": None, "died_within_5": 0,
                "total_promoted": 0, "note": "no promoted nodes"}

    died = 0
    for node, promo_turn in first_active.items():
        # Turns within the next 5 after promotion
        window = [t for t in all_turns if promo_turn < t <= promo_turn + 5]
        if not window:
            # No subsequent turns — cannot assess; skip
            continue
        reappeared = any(node in active_by_turn.get(t, set()) for t in window)
        if not reappeared:
            died += 1

    fpr = died / total_promoted if total_promoted > 0 else None
    return {
        "false_positive_rate": round(fpr, 4) if fpr is not None else None,
        "died_within_5": died,
        "total_promoted": total_promoted,
    }


# ---------------------------------------------------------------------------
# Metric 3: Contradiction slope
# Linear regression: turn -> contradiction_score
# ---------------------------------------------------------------------------

def metric_contradiction_slope(receipts: list[dict]) -> dict:
    xs = [float(r.get("turn", 0)) for r in receipts]
    ys = [float(r.get("contradiction_score", 0.0)) for r in receipts]
    reg = linreg(xs, ys)
    return {
        "slope": reg["slope"],
        "intercept": reg["intercept"],
        "r2": reg["r2"],
        "n_turns": reg["n"],
        "mean_contradiction": round(sum(ys) / len(ys), 6) if ys else None,
    }


# ---------------------------------------------------------------------------
# Metric 4: Operator delta
# Did the scoring rule change across the epoch?
# Primary: count turns where recorded_proposal.operations contains
#          a define_scoring_rule or define_selection_predicate op.
# Secondary: compare criteria_before/after for structural changes.
# ---------------------------------------------------------------------------

def _extract_ovm_ops(receipt: dict) -> list[str]:
    """Return list of OVM operation names emitted in this turn."""
    proposal = receipt.get("recorded_proposal", {})
    # ovm_ops is a list of dicts like {"operation": "define_scoring_rule", "rule": "..."}
    ops = proposal.get("ovm_ops", []) or proposal.get("operations", []) or []
    names = []
    for op in ops:
        if isinstance(op, dict):
            n = op.get("operation") or op.get("type") or op.get("name") or ""
            if n:
                names.append(str(n))
    return names


def metric_operator_delta(receipts: list[dict]) -> dict:
    if not receipts:
        return {"changed": None, "note": "no receipts"}

    rule_mutations = 0
    predicate_mutations = 0
    turns_with_any_mutation = 0

    for r in receipts:
        ops = _extract_ovm_ops(r)
        had_rule = any("scoring_rule" in o or "define_scoring" in o for o in ops)
        had_pred = any("selection_predicate" in o or "define_selection" in o for o in ops)
        if had_rule:
            rule_mutations += 1
        if had_pred:
            predicate_mutations += 1
        if had_rule or had_pred:
            turns_with_any_mutation += 1

    # Also check structural criteria drift
    first_cb = receipts[0].get("criteria_before", {})
    last_ca = receipts[-1].get("criteria_after", {})
    criteria_drifted = first_cb != last_ca if (first_cb and last_ca) else None

    # Final rule from last receipt's ovm_ops
    final_rule = ""
    for r in reversed(receipts):
        proposal = r.get("recorded_proposal", {})
        ops = proposal.get("ovm_ops", []) or proposal.get("operations", []) or []
        for op in reversed(ops):
            if isinstance(op, dict) and "scoring_rule" in op.get("operation", op.get("type", "")):
                final_rule = op.get("rule") or op.get("expression") or op.get("value") or ""
                break
        if final_rule:
            break

    return {
        "changed": turns_with_any_mutation > 0,
        "rule_mutations": rule_mutations,
        "predicate_mutations": predicate_mutations,
        "turns_with_mutation": turns_with_any_mutation,
        "criteria_drifted": criteria_drifted,
        "final_rule": final_rule,
        "total_turns": len(receipts),
    }


# ---------------------------------------------------------------------------
# Metric 5: Operator fitness delta
# late-epoch precision minus early-epoch precision
# Proxy: evidence_coverage (most semantically direct)
# ---------------------------------------------------------------------------

def metric_operator_fitness_delta(receipts: list[dict]) -> dict:
    n = len(receipts)
    if n < 2:
        return {"fitness_delta": None, "note": "insufficient turns"}

    mid = n // 2
    early = receipts[:mid]
    late = receipts[mid:]

    def mean_coverage(rs: list[dict]) -> float:
        vals = [r.get("evidence_coverage", 0.0) for r in rs]
        return sum(vals) / len(vals) if vals else 0.0

    early_cov = mean_coverage(early)
    late_cov = mean_coverage(late)
    delta = late_cov - early_cov

    # Also report proposal_quality delta as secondary
    def mean_quality(rs: list[dict]) -> float:
        vals = [r.get("proposal_quality", 0.0) for r in rs]
        return sum(vals) / len(vals) if vals else 0.0

    return {
        "fitness_delta": round(delta, 4),
        "early_mean_coverage": round(early_cov, 4),
        "late_mean_coverage": round(late_cov, 4),
        "early_mean_quality": round(mean_quality(early), 4),
        "late_mean_quality": round(mean_quality(late), 4),
        "early_turns": len(early),
        "late_turns": len(late),
    }


# ---------------------------------------------------------------------------
# Metric 6: Replay determinism
# cargo run --bin replay --receipts-file <path>
# Parse stdout for PASS/FAIL and match counts.
# ---------------------------------------------------------------------------

def metric_replay_determinism(receipts_path: Path) -> dict:
    cmd = [
        "cargo", "run", "--bin", "replay", "--",
        "--receipts-file", str(receipts_path),
    ]
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=120,
            cwd=receipts_path.parent,  # run from same dir so relative paths work
        )
        stdout = result.stdout
        stderr = result.stderr

        passed = "PASS:" in stdout
        failed = "FAIL:" in stdout

        total = matches = None
        for line in stdout.splitlines():
            line = line.strip()
            if line.startswith("Total Receipts:"):
                try:
                    total = int(line.split(":")[1].strip())
                except ValueError:
                    pass
            elif line.startswith("Matches:"):
                try:
                    matches = int(line.split(":")[1].strip())
                except ValueError:
                    pass

        determinism_rate = (matches / total) if (total and total > 0) else None

        return {
            "pass": passed and not failed,
            "total": total,
            "matches": matches,
            "determinism_rate": round(determinism_rate, 4) if determinism_rate is not None else None,
            "exit_code": result.returncode,
            "stderr_snippet": stderr.strip()[-500:] if stderr.strip() else None,
        }
    except subprocess.TimeoutExpired:
        return {"pass": False, "error": "timeout after 120s"}
    except FileNotFoundError:
        return {"pass": False, "error": "cargo not found in PATH"}
    except Exception as exc:
        return {"pass": False, "error": str(exc)}


# ---------------------------------------------------------------------------
# Metric 7: Receipt chain integrity
# Each receipt's prev_hash must equal the previous receipt's hash.
# First receipt must have prev_hash == "genesis".
# ---------------------------------------------------------------------------

def metric_receipt_chain_integrity(receipts: list[dict]) -> dict:
    if not receipts:
        return {"intact": None, "note": "no receipts"}

    broken_links: list[dict] = []

    if receipts[0].get("prev_hash") != "genesis":
        broken_links.append({
            "turn": receipts[0].get("turn"),
            "expected_prev": "genesis",
            "actual_prev": receipts[0].get("prev_hash"),
        })

    for i in range(1, len(receipts)):
        prev = receipts[i - 1]
        curr = receipts[i]
        expected = prev.get("hash")
        actual = curr.get("prev_hash")
        if expected != actual:
            broken_links.append({
                "turn": curr.get("turn"),
                "expected_prev": expected,
                "actual_prev": actual,
            })

    return {
        "intact": len(broken_links) == 0,
        "broken_links": broken_links,
        "total_links_checked": len(receipts),
    }


# ---------------------------------------------------------------------------
# Metric 8: Failure motif reduction
# Count violation patterns per turn; compare early vs late halves.
# Also detect repeated motifs (same violation string appearing multiple times).
# ---------------------------------------------------------------------------

def metric_failure_motif_reduction(receipts: list[dict]) -> dict:
    n = len(receipts)
    if n == 0:
        return {"reduction": None, "note": "no receipts"}

    # Normalise violation strings to motif keys (strip numeric suffixes)
    def to_motif(v: str) -> str:
        # e.g. "insufficient_evidence_coverage:0.00" -> "insufficient_evidence_coverage"
        return v.split(":")[0]

    motif_counts: dict[str, int] = defaultdict(int)
    violations_per_turn = []

    for r in receipts:
        vs = r.get("violations", [])
        violations_per_turn.append(len(vs))
        for v in vs:
            motif_counts[to_motif(v)] += 1

    mid = n // 2
    early_rate = sum(violations_per_turn[:mid]) / mid if mid > 0 else 0.0
    late_rate = sum(violations_per_turn[mid:]) / (n - mid) if (n - mid) > 0 else 0.0
    reduction = early_rate - late_rate  # positive = fewer failures late

    # Linear regression over violations per turn
    turns = [r.get("turn", i + 1) for i, r in enumerate(receipts)]
    reg = linreg([float(t) for t in turns], [float(v) for v in violations_per_turn])

    repeated_motifs = {k: v for k, v in motif_counts.items() if v > 1}

    return {
        "reduction": round(reduction, 4),
        "early_violation_rate": round(early_rate, 4),
        "late_violation_rate": round(late_rate, 4),
        "total_violations": sum(violations_per_turn),
        "unique_motifs": len(motif_counts),
        "repeated_motifs": repeated_motifs,
        "slope": reg["slope"],   # negative = decreasing over time (good)
        "n_turns": n,
    }


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--receipts-file",
        default="canonical_receipts.jsonl",
        help="Path to canonical_receipts.jsonl",
    )
    parser.add_argument(
        "--output",
        default="epoch_001_metrics.json",
        help="Output path for metrics JSON",
    )
    parser.add_argument(
        "--trajectory-file",
        default=None,
        help="Path to rule_trajectory.jsonl (overrides receipt-based mutation count)",
    )
    parser.add_argument(
        "--skip-replay",
        action="store_true",
        help="Skip the cargo replay step (faster, offline)",
    )
    args = parser.parse_args()

    receipts_path = Path(args.receipts_file)
    if not receipts_path.exists():
        print(f"ERROR: receipts file not found: {receipts_path}", file=sys.stderr)
        sys.exit(1)

    receipts = load_receipts(receipts_path)
    print(f"Loaded {len(receipts)} receipts from {receipts_path}", file=sys.stderr)

    metrics: dict = {}

    print("Computing metric 1: hypothesis_precision ...", file=sys.stderr)
    metrics["hypothesis_precision"] = metric_hypothesis_precision(receipts)

    print("Computing metric 2: false_positive_rate ...", file=sys.stderr)
    metrics["false_positive_rate"] = metric_false_positive_rate(receipts)

    print("Computing metric 3: contradiction_slope ...", file=sys.stderr)
    metrics["contradiction_slope"] = metric_contradiction_slope(receipts)

    print("Computing metric 4: operator_delta ...", file=sys.stderr)
    m4 = metric_operator_delta(receipts)
    # Override mutation counts with trajectory file if provided (applied mutations only)
    if args.trajectory_file:
        traj_path = Path(args.trajectory_file)
        if traj_path.exists():
            traj_lines = [l for l in traj_path.read_text().splitlines() if l.strip()]
            m4["rule_mutations"] = len(traj_lines)
            m4["changed"] = len(traj_lines) > 0
        else:
            m4["rule_mutations"] = 0
            m4["changed"] = False
    metrics["operator_delta"] = m4

    print("Computing metric 5: operator_fitness_delta ...", file=sys.stderr)
    metrics["operator_fitness_delta"] = metric_operator_fitness_delta(receipts)

    if args.skip_replay:
        print("Skipping metric 6: replay_determinism (--skip-replay)", file=sys.stderr)
        metrics["replay_determinism"] = {"pass": None, "note": "skipped"}
    else:
        print("Computing metric 6: replay_determinism (cargo run --bin replay) ...", file=sys.stderr)
        metrics["replay_determinism"] = metric_replay_determinism(receipts_path)

    print("Computing metric 7: receipt_chain_integrity ...", file=sys.stderr)
    metrics["receipt_chain_integrity"] = metric_receipt_chain_integrity(receipts)

    print("Computing metric 8: failure_motif_reduction ...", file=sys.stderr)
    metrics["failure_motif_reduction"] = metric_failure_motif_reduction(receipts)

    output: dict = {
        "epoch": "001",
        "receipts_file": str(receipts_path.resolve()),
        "total_turns": len(receipts),
        "metrics": metrics,
    }

    output_path = Path(args.output)
    output_path.write_text(json.dumps(output, indent=2) + "\n")
    print(f"Wrote {output_path}", file=sys.stderr)

    # Summary to stdout
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
