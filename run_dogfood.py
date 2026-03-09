#!/usr/bin/env python3
"""
Dogfood run — feed the project's own knowledge artifacts through the epistemic API.

The system eats its own findings, epoch history, taxonomy, and conversation exports.
Output: a belief graph of what nstar-bit knows about itself.

Usage:
    python3 run_dogfood.py                  # start fresh
    python3 run_dogfood.py --resume         # skip already-processed files
    python3 run_dogfood.py --dry-run        # print signals, don't post
    python3 run_dogfood.py --state path     # use specific state file

The serve binary must be running:
    NSTAR_STATE=epoch_logs/epoch73_fork.json ./target/debug/serve
"""

import argparse
import json
import time
import sys
from pathlib import Path
import urllib.request
import urllib.error

BASE_URL = "http://localhost:3000"
PROGRESS_FILE = Path("epoch_logs/dogfood_progress.json")

# ─────────────────────────────────────────────────────────────────────────────
# Signal corpus — ordered by epistemic weight (high-signal first)
# ─────────────────────────────────────────────────────────────────────────────

SIGNALS = [
    # Tier 1: Permanent findings — highest epistemic weight
    {
        "path": "FINDINGS.md",
        "domain": "technical",
        "source": "findings",
        "note": "Permanent adopted conclusions — 73 epochs of adversarial self-testing",
    },
    {
        "path": "INTERPRETATION.md",
        "domain": "technical",
        "source": "interpretation",
        "note": "What the system actually is and what it has proven",
    },
    {
        "path": "STATUS.md",
        "domain": "technical",
        "source": "status",
        "note": "Current build state — what works, what is missing",
    },
    {
        "path": "GROUNDING_AUDIT_DELTA.md",
        "domain": "technical",
        "source": "audit",
        "note": "Full history scan delta — what the static audit missed",
    },

    # Tier 2: Architecture decisions
    {
        "path": "PURGE_HARDCODED.md",
        "domain": "technical",
        "source": "architecture",
        "note": "Roadmap to strip all hardcoded control logic",
    },
    {
        "path": "CHECKLIST.md",
        "domain": "technical",
        "source": "architecture",
        "note": "Build milestones and completion status",
    },

    # Tier 3: Epoch summaries — each epoch is one signal
    # These are processed in order: the narrative arc of the system's evolution
    *[
        {
            "path": f"epoch_logs/epoch{n}_summary.tsv",
            "domain": "technical",
            "source": "epoch_summary",
            "note": f"Epoch {n} — adversarial turn results, focus concepts, open tensions",
        }
        for n in range(1, 74)
    ],

    # Tier 4: Subagent investigations
    {
        "path": "subagent_runs/05_architecture_search_synthesis.md",
        "domain": "technical",
        "source": "subagent",
        "note": "VSA/HDC/hypergraph research synthesis",
    },
    {
        "path": "subagent_runs/06_ethos_and_insertion_points.md",
        "domain": "technical",
        "source": "subagent",
        "note": "Theoretical grounding and insertion points",
    },
    {
        "path": "subagent_runs/07_meta_runner_comparison_interpretation.md",
        "domain": "technical",
        "source": "subagent",
        "note": "Consequence enforcement proof — -11 concepts under no-placeholder rule",
    },
    {
        "path": "subagent_runs/03_meta3_branch_investigation.md",
        "domain": "technical",
        "source": "subagent",
        "note": "Meta3 branch — negentropy and VSA concepts",
    },

    # Tier 5: Conversation exports — architectural decisions
    {
        "path": "Building Epistemic API.md",
        "domain": "technical",
        "source": "conversation",
        "note": "Support set architecture — evidence-backed confidence",
    },
    {
        "path": "Declarative Architecture Refinement.md",
        "domain": "technical",
        "source": "conversation",
        "note": "Declarative architecture decisions",
    },
    {
        "path": "Refining Graph Epistemics.md",
        "domain": "technical",
        "source": "conversation",
        "note": "Graph epistemics refinement — causal_anchor discovery",
    },

    # Tier 6: Thread narrative (chunked — thread.md is 52KB)
    {
        "path": "epoch_logs/thread.md",
        "domain": "technical",
        "source": "thread",
        "note": "Full autogenesis narrative — 73 epochs of self-investigation",
        "chunk_lines": 100,  # split into 100-line chunks
    },
]


def load_progress() -> set:
    if PROGRESS_FILE.exists():
        return set(json.loads(PROGRESS_FILE.read_text()).get("done", []))
    return set()


def save_progress(done: set):
    PROGRESS_FILE.parent.mkdir(exist_ok=True)
    PROGRESS_FILE.write_text(json.dumps({"done": sorted(done)}, indent=2))


def post_turn(text: str, domain: str, source: str, dry_run: bool) -> bool:
    payload = json.dumps({
        "text": text,
        "domain": domain,
        "source": source,
    }).encode()

    if dry_run:
        preview = text[:120].replace("\n", " ")
        print(f"  [dry-run] POST /turn [{domain}/{source}] {preview}...")
        return True

    try:
        req = urllib.request.Request(
            f"{BASE_URL}/turn",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=60) as resp:
            result = json.loads(resp.read())
            new_c = result.get("concepts_after", 0) - result.get("concepts_before", 0)
            new_r = result.get("relations_after", 0) - result.get("relations_before", 0)
            ts = result.get("receipt_ts", 0)
            print(f"  turn={result.get('turn')} +concepts={new_c} +relations={new_r} receipt={ts}")
            return True
    except urllib.error.URLError as e:
        print(f"  ERROR: {e} — is serve running on port 3000?")
        return False


def check_serve_running() -> bool:
    try:
        with urllib.request.urlopen(f"{BASE_URL}/manifest", timeout=3) as resp:
            data = json.loads(resp.read())
            print(f"[serve] live — turn={data.get('turn')} concepts={data.get('summary', {}).get('concepts')} relations={data.get('summary', {}).get('relations')}")
            return True
    except Exception:
        return False


def process_signal(sig: dict, done: set, dry_run: bool, resume: bool) -> bool:
    path = Path(sig["path"])
    key = sig["path"]

    if resume and key in done:
        print(f"  [skip] {key}")
        return True

    if not path.exists():
        print(f"  [missing] {key}")
        return True

    domain = sig["domain"]
    source = sig["source"]
    note = sig.get("note", "")
    chunk_lines = sig.get("chunk_lines")

    content = path.read_text(errors="replace")

    if chunk_lines:
        lines = content.splitlines()
        chunks = [lines[i:i+chunk_lines] for i in range(0, len(lines), chunk_lines)]
        print(f"[signal] {key} ({len(chunks)} chunks, {len(lines)} lines)")
        for i, chunk in enumerate(chunks):
            chunk_key = f"{key}:chunk{i}"
            if resume and chunk_key in done:
                continue
            text = f"[source: {key}] [part {i+1}/{len(chunks)}] [{note}]\n\n" + "\n".join(chunk)
            ok = post_turn(text, domain, source, dry_run)
            if not ok:
                return False
            done.add(chunk_key)
            save_progress(done)
            if not dry_run:
                time.sleep(2)
    else:
        # Truncate at 4000 chars — enough context without flooding
        truncated = content[:4000]
        if len(content) > 4000:
            truncated += f"\n\n[truncated — {len(content)} total chars]"
        text = f"[source: {key}] [{note}]\n\n{truncated}"
        print(f"[signal] {key} ({len(content)} chars)")
        ok = post_turn(text, domain, source, dry_run)
        if not ok:
            return False

    done.add(key)
    save_progress(done)
    if not dry_run:
        time.sleep(3)
    return True


def print_manifest():
    try:
        with urllib.request.urlopen(f"{BASE_URL}/manifest", timeout=5) as resp:
            data = json.loads(resp.read())
            summary = data.get("summary", {})
            print(f"\n[manifest] turn={data.get('turn')} concepts={summary.get('concepts')} relations={summary.get('relations')} evidence={summary.get('evidence')}")
            print(f"[manifest] active_focus={data.get('active_focus', [])}")
            top = data.get("live_concepts", [])[:8]
            print("[manifest] top concepts:")
            for c in top:
                print(f"  {c['mention_count']:4d}x  {c['id']}")
    except Exception as e:
        print(f"[manifest] error: {e}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--state", default="epoch_logs/epoch73_fork.json")
    args = parser.parse_args()

    if not args.dry_run:
        if not check_serve_running():
            print("\nServe is not running. Start it with:")
            print(f"  NSTAR_STATE={args.state} ./target/debug/serve")
            print("\nThen re-run this script.")
            sys.exit(1)

    done = load_progress() if args.resume else set()
    total = len(SIGNALS)
    skipped = sum(1 for s in SIGNALS if s["path"] in done) if args.resume else 0

    print(f"\n[dogfood] {total} signals | {skipped} already done | resume={args.resume} | dry_run={args.dry_run}")
    print("[dogfood] feeding the system its own knowledge...\n")

    for i, sig in enumerate(SIGNALS):
        print(f"[{i+1}/{total}]", end=" ")
        ok = process_signal(sig, done, args.dry_run, args.resume)
        if not ok:
            print("\n[abort] POST failed — stopping.")
            break

    print("\n[dogfood] done")
    if not args.dry_run:
        print_manifest()


if __name__ == "__main__":
    main()
