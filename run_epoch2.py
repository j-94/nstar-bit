#!/usr/bin/env python3
"""Epoch 2 autogenesis runner — pruning_impact_audit.

Usage:
  python3 run_epoch2.py                        # run all tasks from start
  python3 run_epoch2.py --resume               # resume from last completed turn
  python3 run_epoch2.py --state-file FILE      # custom state path
  python3 run_epoch2.py --tasks-file FILE      # custom task list
  python3 run_epoch2.py --summary-file FILE    # custom summary output
"""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--resume", action="store_true")
parser.add_argument("--state-file", default="nstar-autogenesis/epoch2_fork.json")
parser.add_argument("--tasks-file", default="epoch2_tasks.txt")
parser.add_argument("--summary-file", default="epoch2_summary.tsv")
args = parser.parse_args()

STATE_FILE   = Path(args.state_file)
TASKS_FILE   = Path(args.tasks_file)
SUMMARY_FILE = Path(args.summary_file)

tasks = [l.strip() for l in TASKS_FILE.read_text().splitlines() if l.strip()]

BIN = "target/debug/autogenesis"
SEP = "━" * 65


def autogenesis_cmd(*extra: str) -> list[str]:
    return [BIN, "--state", str(STATE_FILE)] + list(extra)


def state_turn() -> int:
    try:
        return json.loads(STATE_FILE.read_text()).get("turn", 0)
    except Exception:
        return 0


def state_metrics() -> dict:
    try:
        s = json.loads(STATE_FILE.read_text())
        return {
            "turn":      s.get("turn", 0),
            "concepts":  len(s.get("concepts", {})),
            "relations": len(s.get("relations", {})),
            "evidence":  len(s.get("evidence_log", [])),
            "focus":     s.get("active_focus", []),
            "gate":      s.get("latest_gate", {}).get("reason", ""),
            "run_id":    s.get("run_lineage", {}).get("run_id", ""),
            "status":    s.get("run_lineage", {}).get("status", ""),
            "tensions":  s.get("unresolved_tensions", []),
            "seeds":     len(s.get("seed_queue", [])),
        }
    except Exception:
        return {}


def run_turn(prompt: str, max_retries: int = 5, base_wait: int = 20) -> bool:
    before = state_turn()
    for attempt in range(max_retries):
        result = subprocess.run(autogenesis_cmd("turn", prompt))
        if state_turn() > before:
            return True
        if attempt < max_retries - 1:
            wait = min(base_wait * (2 ** attempt), 240)
            print(f"  !! turn did not advance — retrying in {wait}s "
                  f"(attempt {attempt + 1}/{max_retries - 1})")
            time.sleep(wait)
    return False


# ── Determine start point ──────────────────────────────────────────────────────

baseline_turn = state_turn()

if args.resume:
    # Each task advances turn by 1; completed = turns run since epoch start
    # We don't track an external receipt file, so derive from state turn.
    # Epoch 2 started at whatever turn the fork was at when the epoch began.
    # We store that in the summary header line.
    if SUMMARY_FILE.exists():
        lines = [l for l in SUMMARY_FILE.read_text().splitlines() if l.strip()]
        if len(lines) >= 2:
            try:
                epoch_start = int(lines[1].split("\t")[0])
                completed = state_turn() - epoch_start
                start = completed + 1
                print(f"Resuming: epoch started at turn {epoch_start}, "
                      f"{completed} tasks done, starting at task {start}")
            except Exception:
                start = 1
        else:
            start = 1
    else:
        start = 1
else:
    start = 1
    SUMMARY_FILE.write_text(
        "epoch_start_turn\ttask\tresult\tturn\tconcepts\trelations\tevidence"
        "\tfocus\tseeds\ttensions\tgate\n"
    )
    print(f"Starting epoch 2 from state turn={baseline_turn}  run={state_metrics().get('run_id','?')}")

epoch_start_turn = state_turn()

# ── Main loop ─────────────────────────────────────────────────────────────────

for i, prompt in enumerate(tasks, 1):
    if i < start:
        continue

    m_before = state_metrics()
    print(f"\n{SEP}")
    print(f"TASK {i}/{len(tasks)}  (state turn {m_before['turn']} → {m_before['turn']+1})")
    print(f"PROMPT: {prompt[:120]}{'…' if len(prompt) > 120 else ''}")
    print(SEP)

    ok = run_turn(prompt)
    result = "ok" if ok else "failed"

    m = state_metrics()
    focus_str   = ",".join(m.get("focus", []))[:60]
    tensions    = len(m.get("tensions", []))
    gate_str    = m.get("gate", "")[:80]

    print(f"\n  turn={m['turn']}  concepts={m['concepts']}  "
          f"relations={m['relations']}  evidence={m['evidence']}")
    print(f"  focus={focus_str or '—'}")
    print(f"  gate={gate_str or '—'}")
    if tensions:
        print(f"  tensions={tensions}")

    with SUMMARY_FILE.open("a") as f:
        f.write(
            f"{epoch_start_turn}\t{i}\t{result}\t{m['turn']}\t{m['concepts']}\t"
            f"{m['relations']}\t{m['evidence']}\t{focus_str}\t"
            f"{m['seeds']}\t{tensions}\t{gate_str}\n"
        )

# ── Post-epoch summary ─────────────────────────────────────────────────────────

print(f"\n{SEP}")
print("ALL TASKS DONE — final state")
print(SEP)
subprocess.run(autogenesis_cmd("show"))

print(f"\n{SEP}")
print("SUMMARY TABLE")
print(SEP)
rows = [line.split("\t") for line in SUMMARY_FILE.read_text().splitlines() if line.strip()]
if rows:
    widths = [max(len(r[c]) for r in rows if c < len(r)) for c in range(len(rows[0]))]
    for row in rows:
        print("  ".join(cell.ljust(widths[c]) for c, cell in enumerate(row)))

# ── LM-authored fork proposal for epoch 3 ─────────────────────────────────────

print(f"\n{SEP}")
print("EPOCH 3 FORK PROPOSAL (LM-authored)")
print(SEP)
subprocess.run(autogenesis_cmd(
    "propose",
    "--fork-output", "nstar-autogenesis/epoch3_fork.json",
))
