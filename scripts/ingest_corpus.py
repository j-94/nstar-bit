#!/usr/bin/env python3
"""Corpus ingestion epoch — assimilate past repos into the autogenesis graph.

Each README in the live corpus becomes one turn in the autogenesis engine.
The engine extracts concepts, relations, and evidence via its existing
author_autogenesis_turn mechanism. No new format. No grepping.
The graph IS the index after this runs.

Usage:
    python3 scripts/ingest_corpus.py
    python3 scripts/ingest_corpus.py --state epoch_logs/epoch72_fork.json
    python3 scripts/ingest_corpus.py --resume   # skip already-ingested paths
    python3 scripts/ingest_corpus.py --dry-run  # print prompts, don't run
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

BIN = "./target/debug/autogenesis"
CORPUS_SOURCE = Path("README_CORPUS_SYNOPSIS.md")
LOG_FILE = Path("epoch_logs/corpus_ingestion.log")
STATE_OUT = Path("epoch_logs/corpus_ingested_state.json")
PROGRESS_FILE = Path("epoch_logs/corpus_ingestion_progress.json")

# Repos to prioritise — run these first regardless of order in corpus
PRIORITY_PATHS = [
    "meta2-engine",
    "meta3-graph-core",
    "meta3-causal-kernel",
    "anthropic-proxy",
    "agentic-os",
    "dreaming-kernel",
    "graph-kernel",
    "donkey",
    "prime_dsl",
    "agentic-deep-graph-reasoning",
    "agentic-network-effects-lab",
    "meta5-omni-engine",
    "meta5-causal-vault",
    "meta5-runtime-assertions",
    "one-engine",
    "nstar",
]

TURN_PROMPT_TEMPLATE = """\
CORPUS INGESTION — {repo_name}

You are reading the README of a past system built by the same developer as nstar-bit. \
Your task: extract what this system was trying to do, what concepts it implements, \
and how it relates to the nstar project and its graph/evidence/scoring architecture.

Source path: {path}

README content:
---
{content}
---

Focus on:
1. What core problem was this system solving?
2. What architectural concepts appear — receipts, gates, graphs, scoring, evidence, \
   determinism, metacognition, orchestration, verification, etc.?
3. What is this system's relationship to: nstar loop, graph substrate, evidence gates, \
   scoring rules, receipt chains, operator control?
4. What survived across implementations — what ideas kept recurring?

Extract concepts and relations from the text. Be specific. \
Do not invent — only extract what is clearly present. \
This turn contributes to building the canonical index of past work.\
"""


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--state", default=None,
                   help="Starting state file (default: latest epoch fork)")
    p.add_argument("--resume", action="store_true",
                   help="Skip README paths already recorded in progress file")
    p.add_argument("--dry-run", action="store_true",
                   help="Print prompts without running the engine")
    p.add_argument("--max", type=int, default=0,
                   help="Stop after this many turns (0 = all)")
    return p.parse_args()


def latest_fork() -> Path:
    import re
    forks = list(Path("epoch_logs").glob("epoch*_fork.json"))
    if not forks:
        raise FileNotFoundError("No epoch fork files found in epoch_logs/")
    forks.sort(key=lambda p: int(re.search(r"epoch(\d+)", p.name).group(1)))
    return forks[-1]


def extract_readme_paths(source: Path) -> list[Path]:
    """Extract /Users/jobs/.../README.md paths from the corpus synopsis."""
    import re
    pattern = re.compile(r"^- `(/Users/jobs/.+/[Rr][Ee][Aa][Dd][Mm][Ee]\.md)`")
    paths = []
    seen = set()
    for line in source.read_text(encoding="utf-8", errors="ignore").splitlines():
        m = pattern.match(line.strip())
        if m and m.group(1) not in seen:
            seen.add(m.group(1))
            paths.append(Path(m.group(1)))
    return paths


def prioritize(paths: list[Path]) -> list[Path]:
    """Put priority repos first, rest after."""
    priority, rest = [], []
    for p in paths:
        key = p.as_posix().lower()
        if any(name.lower() in key for name in PRIORITY_PATHS):
            priority.append(p)
        else:
            rest.append(p)
    return priority + rest


def repo_name(path: Path) -> str:
    """Derive a short repo name from the README path."""
    parts = path.parts
    # Walk up until we find the repo root name (parent of README.md)
    if path.name.lower() == "readme.md":
        candidate = path.parent.name
        # If it's a boring directory name, go one level higher
        if candidate.lower() in {"src", "docs", "lib", "examples", "test"}:
            candidate = path.parent.parent.name
        return candidate
    return path.stem


def load_progress() -> set:
    if PROGRESS_FILE.exists():
        try:
            return set(json.loads(PROGRESS_FILE.read_text()).get("done", []))
        except Exception:
            pass
    return set()


def save_progress(done: set):
    PROGRESS_FILE.write_text(json.dumps({"done": sorted(done)}, indent=2))


def run_turn(state_path: Path, prompt: str, log_fh) -> dict:
    cmd = [BIN, "--state", str(state_path), "turn", prompt]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
    stdout = result.stdout.strip()
    stderr = result.stderr.strip()
    log_fh.write(f"\n{'='*60}\n{prompt[:120]}...\n")
    log_fh.write(f"stdout: {stdout}\n")
    if stderr:
        log_fh.write(f"stderr: {stderr}\n")
    log_fh.flush()

    import re
    out = {}
    for line in stdout.splitlines():
        for m in re.finditer(r"(\w+)=(\d+)", line):
            key, val = m.group(1), int(m.group(2))
            if key in ("turn", "concepts", "relations", "discovered", "promoted", "archived"):
                out[key] = out.get(key, 0) + val
    return out


def main():
    args = parse_args()

    if not CORPUS_SOURCE.exists():
        print(f"ERROR: {CORPUS_SOURCE} not found. Run from repo root.", file=sys.stderr)
        sys.exit(1)

    # Resolve starting state
    state_path = Path(args.state) if args.state else latest_fork()
    if not state_path.exists():
        print(f"ERROR: state file {state_path} not found.", file=sys.stderr)
        sys.exit(1)

    # Copy to working state so original fork is preserved
    working_state = STATE_OUT
    if not args.resume or not working_state.exists():
        shutil.copy(state_path, working_state)
        print(f"Starting from: {state_path} → {working_state}")
    else:
        print(f"Resuming: {working_state}")

    # Collect paths
    all_paths = extract_readme_paths(CORPUS_SOURCE)
    existing = [p for p in all_paths if p.exists()]
    ordered = prioritize(existing)

    print(f"Corpus: {len(all_paths)} paths in synopsis, {len(existing)} exist on disk")
    print(f"Priority repos first: {len([p for p in ordered if any(n.lower() in p.as_posix().lower() for n in PRIORITY_PATHS)])}")

    done = load_progress() if args.resume else set()
    todo = [p for p in ordered if str(p) not in done]
    if args.max:
        todo = todo[:args.max]

    print(f"Turns to run: {len(todo)} (skipping {len(done)} already done)\n")

    if args.dry_run:
        for p in todo[:3]:
            name = repo_name(p)
            content = p.read_text(encoding="utf-8", errors="ignore")[:2000]
            prompt = TURN_PROMPT_TEMPLATE.format(repo_name=name, path=p, content=content)
            print(f"\n{'='*60}\n{prompt[:400]}\n...\n")
        print(f"[dry-run] would run {len(todo)} turns")
        return

    LOG_FILE.parent.mkdir(exist_ok=True)
    totals = {"concepts": 0, "relations": 0, "discovered": 0}

    with open(LOG_FILE, "a", encoding="utf-8") as log_fh:
        log_fh.write(f"\n\n{'#'*65}\n# Corpus ingestion — {time.strftime('%Y-%m-%d %H:%M')}\n# State: {working_state}\n{'#'*65}\n")

        for i, readme_path in enumerate(todo, 1):
            name = repo_name(readme_path)
            print(f"[{i:02d}/{len(todo)}] {name} ", end="", flush=True)

            try:
                content = readme_path.read_text(encoding="utf-8", errors="ignore")
                # Truncate very long READMEs — keep most informative part
                if len(content) > 4000:
                    content = content[:3800] + "\n\n[...truncated...]"

                prompt = TURN_PROMPT_TEMPLATE.format(
                    repo_name=name,
                    path=readme_path,
                    content=content,
                )

                t0 = time.time()
                out = run_turn(working_state, prompt, log_fh)
                elapsed = time.time() - t0

                c = out.get("concepts", 0)
                r = out.get("relations", 0)
                d = out.get("discovered", 0)
                totals["concepts"] += c
                totals["relations"] += r
                totals["discovered"] += d

                print(f"concepts={c} relations={r} discovered={d} ({elapsed:.0f}s)")
                done.add(str(readme_path))
                save_progress(done)

            except subprocess.TimeoutExpired:
                print("TIMEOUT")
                log_fh.write(f"TIMEOUT: {readme_path}\n")
            except Exception as e:
                print(f"ERROR: {e}")
                log_fh.write(f"ERROR: {readme_path}: {e}\n")

    print(f"\n{'='*60}")
    print(f"Corpus ingestion complete")
    print(f"  Turns run:      {len(done)}")
    print(f"  Concepts seen:  {totals['concepts']}")
    print(f"  Relations seen: {totals['relations']}")
    print(f"  Discovered:     {totals['discovered']}")
    print(f"  State:          {working_state}")
    print(f"  Log:            {LOG_FILE}")
    print(f"\nNext: run an epoch against this state to score cross-repo relations.")
    print(f"  python3 run_all_epochs.py --state {working_state}")


if __name__ == "__main__":
    main()
