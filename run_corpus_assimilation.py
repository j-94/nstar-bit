#!/usr/bin/env python3
"""Corpus assimilation epoch — feeds the 281-repo README corpus into the autogenesis engine.

Each README becomes one turn. The LM decides what concepts/relations to emit.
The scoring rule in the state filters what survives. No manual rubrics.

Usage:
  python3 run_corpus_assimilation.py
  python3 run_corpus_assimilation.py --resume
  python3 run_corpus_assimilation.py --state epoch_logs/epoch72_fork.json
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

README_CORPUS = Path("README_CORPUS_SYNOPSIS.md")
BIN = "target/debug/autogenesis"
LOG_DIR = Path("epoch_logs")
PROGRESS_FILE = LOG_DIR / "corpus_assimilation_progress.json"
LOG_FILE = LOG_DIR / "corpus_assimilation.log"
MAX_README_CHARS = 3000  # truncate very long READMEs to keep turn messages tight


def latest_fork() -> Path:
    forks = list(LOG_DIR.glob("epoch*_fork.json"))
    if not forks:
        raise FileNotFoundError("No epoch forks found in epoch_logs/")
    forks.sort(key=lambda p: int(re.search(r"epoch(\d+)", p.name).group(1)))
    return forks[-1]


def load_state(path: str) -> dict:
    try:
        return json.loads(Path(path).read_text())
    except Exception:
        return {}


def state_turn(path: str) -> int:
    return load_state(path).get("turn", 0)


def autogenesis_cmd(state: str, *extra: str) -> list:
    return [BIN, "--state", state] + list(extra)


def run_turn(state: str, prompt: str, max_retries: int = 3, base_wait: int = 15) -> bool:
    before = state_turn(state)
    for attempt in range(max_retries):
        subprocess.run(autogenesis_cmd(state, "turn", prompt))
        if state_turn(state) > before:
            return True
        if attempt < max_retries - 1:
            wait = min(base_wait * (2 ** attempt), 120)
            print(f"  !! turn did not advance (attempt {attempt+1}) — retrying in {wait}s")
            time.sleep(wait)
    return False


def extract_readme_paths(synopsis: Path) -> list[str]:
    """Pull the 281 singleton README paths from README_CORPUS_SYNOPSIS.md."""
    paths = []
    in_locations = False
    for line in synopsis.read_text().splitlines():
        if line.startswith("## Locations"):
            in_locations = True
            continue
        if in_locations:
            stripped = line.strip()
            if stripped.startswith("- `") and stripped.endswith("`"):
                paths.append(stripped[3:-1])  # strip "- `" and trailing "`"
            elif stripped.startswith("- /"):
                paths.append(stripped[2:])
            elif stripped.startswith("#"):
                break
    return paths


def read_readme(path: str) -> str:
    p = Path(path)
    if not p.exists():
        return ""
    try:
        content = p.read_text(errors="replace")
        if len(content) > MAX_README_CHARS:
            content = content[:MAX_README_CHARS] + "\n...[truncated]"
        return content
    except Exception:
        return ""


def build_turn_message(readme_path: str, content: str) -> str:
    """Build the turn message. The LM will extract symbols and author graph transitions."""
    repo_name = Path(readme_path).parent.name
    return f"""CORPUS ASSIMILATION TURN

Repository: {repo_name}
Path: {readme_path}

README content:
---
{content}
---

Based on this README, contribute to the graph:
- What was this system trying to do at its core?
- What concepts does it implement or depend on?
- How does it relate to: signal_driven_execution, receipt_based_validation,
  dynamic_predicate_discovery, operator_control_plane, unified_execution_substrate,
  graph_first_architecture, deterministic_replay?
- Was this abandoned, evolved into something else, or still active?

Emit evidence-backed concepts and relations only. Do not assert without evidence from the README.
"""


def load_progress() -> dict:
    if PROGRESS_FILE.exists():
        try:
            return json.loads(PROGRESS_FILE.read_text())
        except Exception:
            pass
    return {"completed": [], "failed": []}


def save_progress(p: dict):
    PROGRESS_FILE.write_text(json.dumps(p, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--state", default="", help="Starting state file (default: latest fork)")
    parser.add_argument("--resume", action="store_true", help="Skip already-completed READMEs")
    parser.add_argument("--dry-run", action="store_true", help="Print tasks without running")
    args = parser.parse_args()

    # Resolve state file
    state_path = args.state or str(latest_fork())
    print(f"[corpus] state: {state_path}")
    print(f"[corpus] turn at start: {state_turn(state_path)}")

    # Extract README paths
    paths = extract_readme_paths(README_CORPUS)
    print(f"[corpus] {len(paths)} READMEs in corpus")

    # Load progress
    progress = load_progress() if args.resume else {"completed": [], "failed": []}
    completed = set(progress["completed"])
    failed = set(progress["failed"])

    pending = [p for p in paths if p not in completed]
    print(f"[corpus] {len(pending)} remaining ({len(completed)} done, {len(failed)} failed)")

    if args.dry_run:
        for p in pending[:10]:
            print(f"  would run: {p}")
        print("  ...")
        return

    LOG_DIR.mkdir(exist_ok=True)

    with open(LOG_FILE, "a") as log:
        log.write(f"\n\n{'#'*65}\n# Corpus assimilation — {time.strftime('%Y-%m-%d %H:%M')}\n"
                  f"# state: {state_path}\n# pending: {len(pending)}\n{'#'*65}\n")

        for i, readme_path in enumerate(pending, 1):
            print(f"\n[{i}/{len(pending)}] {Path(readme_path).parent.name} / "
                  f"{Path(readme_path).name}")

            content = read_readme(readme_path)
            if not content:
                print(f"  skip: unreadable or missing")
                failed.add(readme_path)
                progress["failed"] = list(failed)
                save_progress(progress)
                continue

            message = build_turn_message(readme_path, content)
            log.write(f"\n--- {readme_path} ---\nMESSAGE LENGTH: {len(message)}\n")

            t0 = time.time()
            ok = run_turn(state_path, message)
            elapsed = time.time() - t0

            if ok:
                completed.add(readme_path)
                progress["completed"] = list(completed)
                s = load_state(state_path)
                print(f"  ok ({elapsed:.0f}s) | "
                      f"turn={s.get('turn','?')} "
                      f"concepts={len(s.get('concepts',{}))} "
                      f"relations={len(s.get('relations',{}))}")
                log.write(f"RESULT: ok ({elapsed:.0f}s)\n")
            else:
                failed.add(readme_path)
                progress["failed"] = list(failed)
                print(f"  FAILED ({elapsed:.0f}s)")
                log.write(f"RESULT: FAILED\n")

            save_progress(progress)

    print(f"\n[corpus] done. completed={len(completed)} failed={len(failed)}")
    print(f"[corpus] final turn: {state_turn(state_path)}")
    print(f"[corpus] progress: {PROGRESS_FILE}")
    print(f"[corpus] log: {LOG_FILE}")


if __name__ == "__main__":
    main()
