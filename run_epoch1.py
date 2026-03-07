#!/usr/bin/env python3
"""Epoch 1 fixed task runner.

Usage:
  python3 run_epoch1.py                                    # clean reset, run all tasks
  python3 run_epoch1.py --resume                           # resume from last receipt
  python3 run_epoch1.py --frozen-rule "c11/(c10+c01+1)"   # freeze scoring rule
  python3 run_epoch1.py --state-file FILE                  # custom state path
  python3 run_epoch1.py --receipts-file FILE               # custom receipts path
  python3 run_epoch1.py --summary-file FILE                # custom summary path
  python3 run_epoch1.py --tasks-file FILE                  # custom task list
"""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--resume", action="store_true",
                    help="Resume from next unrun task instead of resetting")
parser.add_argument("--frozen-rule", default=None,
                    help="Fix scoring rule for the entire epoch (baseline mode)")
parser.add_argument("--state-file", default="nstar_canonical_state.json")
parser.add_argument("--receipts-file", default="canonical_receipts.jsonl")
parser.add_argument("--summary-file", default="epoch1_summary.tsv")
parser.add_argument("--tasks-file", default="epoch1_tasks.txt")
args = parser.parse_args()

TASKS_FILE   = Path(args.tasks_file)
SUMMARY_FILE = Path(args.summary_file)
STATE_FILE   = Path(args.state_file)
RECEIPTS     = Path(args.receipts_file)

tasks = [l.strip() for l in TASKS_FILE.read_text().splitlines() if l.strip()]


# ── Build canonical command ────────────────────────────────────────────────────

def canonical_cmd(*extra: str) -> list[str]:
    cmd = [
        "cargo", "run", "--bin", "canonical", "--",
        "--state-file", str(STATE_FILE),
        "--receipts-file", str(RECEIPTS),
    ]
    if args.frozen_rule:
        cmd += ["--frozen-rule", args.frozen_rule]
    cmd += list(extra)
    return cmd


# ── Reset or resume ────────────────────────────────────────────────────────────

if not args.resume:
    label = "frozen" if args.frozen_rule else "lm"
    print(f"Resetting canonical state for clean epoch ({label} mode)…")
    subprocess.run(canonical_cmd("--reset"), capture_output=True)
    SUMMARY_FILE.write_text(
        "turn\ttask\tdecision\tinv_pass\tcov\tcon\tviolations\tdiscovered\tovm_rule\thash\n"
    )
    start = 1
else:
    try:
        lines = [l for l in RECEIPTS.read_text().splitlines() if l.strip()]
        completed = len(lines)
    except Exception:
        completed = 0
    start = completed + 1
    print(f"Resuming from task {start} (chain has {completed} receipts)")
    if not SUMMARY_FILE.exists():
        SUMMARY_FILE.write_text(
            "turn\ttask\tdecision\tinv_pass\tcov\tcon\tviolations\tdiscovered\tovm_rule\thash\n"
        )

SEP = "━" * 65


def current_turns() -> int:
    try:
        return json.loads(STATE_FILE.read_text()).get("turn_count", 0)
    except Exception:
        return 0


def ovm_state() -> str:
    try:
        g = json.loads(STATE_FILE.read_text())["graph"]
        rule = g.get("scoring_rule", "")
        pred = g.get("selection_predicate", "")
        parts = []
        if rule:
            parts.append(f"rule:{rule[:55]}")
        if pred:
            parts.append(f"pred:{pred[:35]}")
        return " | ".join(parts) if parts else "empty"
    except Exception:
        return "?"


def receipt_count() -> int:
    try:
        return sum(1 for l in RECEIPTS.read_text().splitlines() if l.strip())
    except Exception:
        return 0


def last_receipt() -> dict:
    try:
        lines = [l for l in RECEIPTS.read_text().splitlines() if l.strip()]
        return json.loads(lines[-1]) if lines else {}
    except Exception:
        return {}


def run_task(prompt: str, max_retries: int = 6, base_wait: int = 30) -> bool:
    """Run a single canonical turn, retrying if the LM is rate-limited.

    Returns True if a new receipt was written, False if all retries exhausted.
    """
    before = receipt_count()
    for attempt in range(max_retries):
        subprocess.run(canonical_cmd(prompt))
        if receipt_count() > before:
            return True
        if attempt < max_retries - 1:
            wait = min(base_wait * (2 ** attempt), 300)
            print(f"  ⚠  No receipt written — rate-limited? Retrying in {wait}s "
                  f"(attempt {attempt + 1}/{max_retries - 1})")
            time.sleep(wait)
    return False


# ── Main loop ─────────────────────────────────────────────────────────────────

for i, prompt in enumerate(tasks, 1):
    if i < start:
        continue

    chain_turn = current_turns() + 1
    print(f"\n{SEP}")
    print(f"TASK {i}/{len(tasks)}  (chain turn {chain_turn})")
    print(f"PROMPT: {prompt}")
    print(SEP)

    ok = run_task(prompt)
    if not ok:
        print(f"  ✗  Task {i} failed after all retries — skipping")

    rec       = last_receipt()
    decision  = str(rec.get("decision", ""))
    inv_pass  = str(rec.get("invariant_passed", ""))
    cov       = f"{rec.get('evidence_coverage', 0):.2f}"
    con       = f"{rec.get('contradiction_score', 0):.2f}"
    violations = ", ".join(rec.get("violations", [])) or "—"
    discovered = ", ".join(rec.get("discovered_nodes", [])) or "—"
    h         = rec.get("hash", "")
    actual    = rec.get("turn", current_turns())
    rule      = ovm_state()

    print(f"\n  → decision={decision}  inv={inv_pass}  cov={cov}  con={con}")
    if violations != "—":
        print(f"  → violations: {violations}")
    if discovered != "—":
        print(f"  → discovered: {discovered}")
    print(f"  → ovm: {rule}")
    print(f"  → hash: {h}")

    with SUMMARY_FILE.open("a") as f:
        f.write(f"{actual}\t{i}\t{decision}\t{inv_pass}\t{cov}\t{con}\t"
                f"{violations}\t{discovered}\t{rule}\t{h}\n")

# ── Post-epoch ────────────────────────────────────────────────────────────────

print(f"\n{SEP}")
print("ALL TASKS DONE — running replay verifier")
print(SEP)
subprocess.run([
    "cargo", "run", "--bin", "replay", "--",
    "--receipts-file", str(RECEIPTS),
])

print(f"\n{SEP}")
print("FINAL STATE")
print(SEP)
subprocess.run(canonical_cmd("--state"))

print(f"\n{SEP}")
print("SUMMARY TABLE")
print(SEP)
rows = [line.split("\t") for line in SUMMARY_FILE.read_text().splitlines()]
if rows:
    widths = [max(len(r[c]) for r in rows if c < len(r)) for c in range(len(rows[0]))]
    for row in rows:
        print("  ".join(cell.ljust(widths[c]) for c, cell in enumerate(row)))
