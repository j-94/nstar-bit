#!/usr/bin/env python3
"""Feed the README corpus through the canonical Rust OVM.

Each README = one turn. The canonical LM discovers what survived across
all past work. No manual rubrics. No hardcoded schema. The scoring rule
decides what lives.

Usage:
  python3 run_canonical_corpus.py
  python3 run_canonical_corpus.py --reset     # wipe state, start fresh
  python3 run_canonical_corpus.py --resume    # skip already-receipted turns
"""

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

BIN          = "./target/debug/canonical"
STATE_FILE   = "nstar_canonical_state.json"
RECEIPTS     = "canonical_receipts.jsonl"
SYNOPSIS     = Path("README_CORPUS_SYNOPSIS.md")
LOG          = Path("epoch_logs/canonical_corpus.log")
PROGRESS     = Path("epoch_logs/canonical_corpus_progress.json")
MAX_CHARS    = 3000

parser = argparse.ArgumentParser(description=__doc__,
    formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument("--reset",  action="store_true", help="Reset canonical state before running")
parser.add_argument("--resume", action="store_true", help="Skip READMEs already in receipt chain")
parser.add_argument("--dry-run",action="store_true", help="Print paths without running")
parser.add_argument("--limit",  type=int, default=0,  help="Stop after N READMEs (0=all)")
args = parser.parse_args()


def extract_paths(synopsis: Path) -> list[str]:
    paths, in_loc = [], False
    for line in synopsis.read_text().splitlines():
        if re.search(r'## .*(location|path|file)', line, re.I):
            in_loc = True; continue
        if in_loc:
            s = line.strip()
            for pat in [r'- `(/.+)`', r'- (/.+\.md)', r'^\| (/.+) \|']:
                m = re.match(pat, s)
                if m: paths.append(m.group(1)); break
            if s.startswith('#') and paths: break
    return paths


def read_readme(path: str) -> str:
    p = Path(path)
    if not p.exists(): return ""
    try:
        text = p.read_text(errors="replace")
        return text[:MAX_CHARS] + ("\n...[truncated]" if len(text) > MAX_CHARS else "")
    except Exception:
        return ""


def already_receipted() -> set[str]:
    """Return set of source URIs already in the receipt chain."""
    done = set()
    rp = Path(RECEIPTS)
    if not rp.exists(): return done
    for line in rp.read_text().splitlines():
        try:
            r = json.loads(line)
            uri = r.get("source_uri", "") or r.get("prompt", "")
            if uri: done.add(uri)
        except Exception:
            pass
    return done


def load_progress() -> dict:
    if PROGRESS.exists():
        try: return json.loads(PROGRESS.read_text())
        except Exception: pass
    return {"completed": [], "failed": []}


def save_progress(p: dict):
    PROGRESS.write_text(json.dumps(p, indent=2))


def canonical_turn(prompt: str) -> tuple[bool, str]:
    """Run one canonical turn. Returns (success, output)."""
    result = subprocess.run(
        [BIN, "--state-file", STATE_FILE, "--receipts-file", RECEIPTS, prompt],
        capture_output=True, text=True, timeout=180
    )
    out = (result.stdout + result.stderr).strip()
    ok  = result.returncode == 0 and ("Commit" in out or "Halt" in out or "decision" in out.lower())
    return ok, out


def state_snapshot() -> dict:
    try:
        raw = json.loads(Path(STATE_FILE).read_text())
        g = raw.get("graph", {})
        return {
            "turn":         raw.get("turn_count", 0),
            "nodes":        len(g.get("nodes", {})),
            "edges":        len(g.get("edges", {})),
            "patterns":     len(g.get("patterns", {})),
            "scoring_rule": g.get("scoring_rule", "(none)"),
            "selection_predicate": g.get("selection_predicate", "(none)"),
            "seed_queue":   g.get("seed_queue", []),
        }
    except Exception:
        return {}


def pop_seed_from_state(seed: str):
    """Remove a seed from the state file after running it."""
    try:
        raw = json.loads(Path(STATE_FILE).read_text())
        q = raw.get("graph", {}).get("seed_queue", [])
        if seed in q:
            q.remove(seed)
            raw["graph"]["seed_queue"] = q
            Path(STATE_FILE).write_text(json.dumps(raw))
    except Exception:
        pass


def drain_seed_queue(log) -> int:
    """Run any self-generated investigation prompts before the next README.
    Returns number of seeds drained."""
    drained = 0
    while True:
        s = state_snapshot()
        seeds = s.get("seed_queue", [])
        if not seeds:
            break
        seed = seeds[0]
        print(f"  [seed] {seed[:100]}...")
        # Pop it first so re-runs don't re-queue the same seed
        pop_seed_from_state(seed)
        t0 = time.time()
        try:
            ok, out = canonical_turn(seed)
        except subprocess.TimeoutExpired:
            ok, out = False, "TIMEOUT"
        elapsed = time.time() - t0
        log.write(f"\n--- [SEED] ({elapsed:.0f}s) ---\n{out[:300]}\n")
        if ok:
            snap = state_snapshot()
            print(f"    ok ({elapsed:.0f}s)  turn={snap.get('turn','?')}  "
                  f"nodes={snap.get('nodes',0)}  edges={snap.get('edges',0)}  "
                  f"seeds_remaining={len(snap.get('seed_queue',[]))}")
        else:
            print(f"    FAILED ({elapsed:.0f}s): {out[:80]}")
        drained += 1
        if drained >= 3:  # cap per README to avoid runaway self-investigation
            break
    return drained


# ── Setup ──────────────────────────────────────────────────────────────────────

LOG.parent.mkdir(exist_ok=True)

if args.reset:
    print("[corpus] --reset: wiping canonical state")
    subprocess.run([BIN, "--reset", "--state-file", STATE_FILE,
                    "--receipts-file", RECEIPTS, "init"], capture_output=True)

# ── Path collection ────────────────────────────────────────────────────────────

paths = [p for p in extract_paths(SYNOPSIS) if Path(p).exists()]
print(f"[corpus] {len(paths)} READMEs on disk")

receipted = already_receipted() if args.resume else set()
progress  = load_progress()     if args.resume else {"completed": [], "failed": []}
completed = set(progress["completed"])
failed    = set(progress["failed"])

pending = [p for p in paths if p not in completed]
print(f"[corpus] {len(pending)} pending  ({len(completed)} done  {len(failed)} failed)")

if args.limit:
    pending = pending[:args.limit]
    print(f"[corpus] --limit {args.limit} applied")

if args.dry_run:
    for p in pending[:10]: print(f"  would run: {p}")
    print("  ...")
    sys.exit(0)

# ── Run ────────────────────────────────────────────────────────────────────────

s0 = state_snapshot()
print(f"[corpus] start  turn={s0.get('turn',0)}  "
      f"nodes={s0.get('nodes',0)}  edges={s0.get('edges',0)}")
print(f"[corpus] scoring_rule: {s0.get('scoring_rule','(none)')[:80]}")
print()

with open(LOG, "a") as log:
    log.write(f"\n{'#'*60}\n# Canonical corpus run — {time.strftime('%Y-%m-%d %H:%M')}\n"
              f"# pending: {len(pending)}\n{'#'*60}\n")

    for i, readme_path in enumerate(pending, 1):
        repo = Path(readme_path).parent.name
        print(f"[{i}/{len(pending)}] {repo}")

        content = read_readme(readme_path)
        if not content:
            print(f"  skip: unreadable")
            failed.add(readme_path); progress["failed"] = list(failed)
            save_progress(progress); continue

        # The prompt: what did this system try to do? How does it relate to the lineage?
        prompt = (
            f"CORPUS TURN — {repo}\n\n"
            f"README:\n---\n{content}\n---\n\n"
            f"What was this system's core intent? "
            f"What patterns does it share with: signal_driven_execution, "
            f"receipt_based_validation, dynamic_predicate_discovery, "
            f"graph_first_architecture, operator_control_plane? "
            f"Was it abandoned, evolved, or still active? "
            f"Emit only what the README directly evidences."
        )

        t0 = time.time()
        try:
            ok, out = canonical_turn(prompt)
        except subprocess.TimeoutExpired:
            ok, out = False, "TIMEOUT"
        elapsed = time.time() - t0

        log.write(f"\n--- {readme_path} ({elapsed:.0f}s) ---\n{out[:500]}\n")

        if ok:
            completed.add(readme_path); progress["completed"] = list(completed)
            s = state_snapshot()
            seeds_pending = len(s.get("seed_queue", []))
            print(f"  ok ({elapsed:.0f}s)  turn={s.get('turn','?')}  "
                  f"nodes={s.get('nodes',0)}  edges={s.get('edges',0)}  "
                  f"seeds={seeds_pending}")
            # Drain self-generated investigation prompts before next README
            if seeds_pending:
                drained = drain_seed_queue(log)
                if drained:
                    print(f"  [seeds] drained {drained} self-investigation turns")
        else:
            failed.add(readme_path); progress["failed"] = list(failed)
            print(f"  FAILED ({elapsed:.0f}s): {out[:120]}")

        save_progress(progress)

# ── Summary ────────────────────────────────────────────────────────────────────

sf = state_snapshot()
print(f"\n[corpus] done.")
print(f"  completed={len(completed)}  failed={len(failed)}")
print(f"  final turn={sf.get('turn',0)}  "
      f"nodes={sf.get('nodes',0)}  edges={sf.get('edges',0)}  patterns={sf.get('patterns',0)}")
print(f"  scoring_rule: {sf.get('scoring_rule','(none)')[:100]}")
