#!/usr/bin/env python3
"""Autonomous corpus discovery agent — RLM loop.

The agent reads sources, reasons about them, writes to its scratchpad,
and commits discoveries to the autogenesis graph via turn emissions.
It cannot write outside the scratchpad. It runs until the graph converges
or a turn limit is reached.

Usage:
    python3 scripts/corpus_agent.py
    python3 scripts/corpus_agent.py --state epoch_logs/epoch72_fork.json
    python3 scripts/corpus_agent.py --turns 20 --verbose
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

# ── config ─────────────────────────────────────────────────────────────────────

BIN         = "./target/debug/autogenesis"
SCRATCHPAD  = Path("epoch_logs/agent_scratchpad")
LOG_FILE    = Path("epoch_logs/corpus_agent.log")

# Agent can READ from these roots only
ALLOWED_READ_ROOTS = [
    Path("/Users/jobs/Desktop/data_prep_stage"),
    Path("/Users/jobs/Desktop/tmp-meta3-engine-test"),
    Path("/Users/jobs/Desktop/Desktop(archive)/github/conversation-vault-projects"),
    Path("/Users/jobs/Desktop/Desktop(archive)/meta7-chatloop-negentropy"),
    Path("/Users/jobs/Desktop/mia-kernel"),
    Path("/Users/jobs/Desktop/consolidatation"),
    Path("/Users/jobs/Desktop/dreaming-kernel"),
    Path("/Users/jobs/Desktop/agentic-os"),
    Path("/Users/jobs/Desktop/core-clarity-dashboard"),
    Path("/Users/jobs/Desktop/graph-kernel"),
    Path("/Users/jobs/Desktop/macro-hard"),
    Path("/Users/jobs/Desktop/Foundational tools"),
    Path("/Users/jobs/Desktop/Desktop(archive)/github/meta2-engine"),
    Path("/Users/jobs/Desktop/Desktop(archive)/github/Donkeyv1-extracted"),
    Path("/Users/jobs/Desktop/Desktop(archive)/meta5-causal-vault"),
    Path("/Users/jobs/Desktop/Desktop(archive)/meta5-runtime-assertions"),
    Path("/Users/jobs/.codex/skills"),
    Path("/Users/jobs/.codex/archived_sessions"),
    Path("/Users/jobs/Developer/nstar-bit"),
    Path(SCRATCHPAD),
]

# Agent can only WRITE here
WRITE_ROOT = SCRATCHPAD.resolve()

# Model for orchestrator (meta) and agent (turn) calls
OPENROUTER_KEY_CMD = ["security", "find-generic-password", "-s", "OPENROUTER_API_KEY", "-w"]

# ── tool implementations ────────────────────────────────────────────────────────

def _allowed_read(path: Path) -> bool:
    resolved = path.resolve()
    return any(
        str(resolved).startswith(str(root.resolve()))
        for root in ALLOWED_READ_ROOTS
    )

def tool_read_file(path: str, max_chars: int = 6000) -> str:
    p = Path(path)
    if not _allowed_read(p):
        return f"BLOCKED: {path} is outside allowed read roots."
    if not p.exists():
        return f"NOT FOUND: {path}"
    try:
        content = p.read_text(encoding="utf-8", errors="ignore")
        if len(content) > max_chars:
            return content[:max_chars] + f"\n\n[...truncated at {max_chars} chars of {len(content)} total]"
        return content
    except Exception as e:
        return f"ERROR reading {path}: {e}"

def tool_list_dir(path: str, max_entries: int = 60) -> str:
    p = Path(path)
    if not _allowed_read(p):
        return f"BLOCKED: {path} is outside allowed read roots."
    if not p.exists():
        return f"NOT FOUND: {path}"
    try:
        entries = sorted(p.iterdir(), key=lambda x: (x.is_file(), x.name))
        lines = []
        for e in entries[:max_entries]:
            size = f"{e.stat().st_size:>10,}" if e.is_file() else "         -"
            lines.append(f"{'d' if e.is_dir() else 'f'}  {size}  {e.name}")
        if len(entries) > max_entries:
            lines.append(f"... and {len(entries)-max_entries} more")
        return "\n".join(lines)
    except Exception as e:
        return f"ERROR listing {path}: {e}"

def tool_search(path: str, term: str, max_lines: int = 30) -> str:
    p = Path(path)
    if not _allowed_read(p):
        return f"BLOCKED: {path} is outside allowed read roots."
    try:
        result = subprocess.run(
            ["grep", "-r", "-i", "-l", "--include=*.json", "--include=*.md",
             "--include=*.jsonl", "--include=*.txt", "--include=*.yaml",
             term, str(p)],
            capture_output=True, text=True, timeout=15
        )
        lines = result.stdout.strip().splitlines()[:max_lines]
        return "\n".join(lines) if lines else f"No matches for '{term}' in {path}"
    except Exception as e:
        return f"ERROR searching: {e}"

def tool_write_scratchpad(filename: str, content: str) -> str:
    # Strip any path traversal attempts
    safe_name = Path(filename).name
    out = WRITE_ROOT / safe_name
    try:
        WRITE_ROOT.mkdir(parents=True, exist_ok=True)
        out.write_text(content, encoding="utf-8")
        return f"Written: {out} ({len(content)} chars)"
    except Exception as e:
        return f"ERROR writing scratchpad: {e}"

def tool_graph_turn(text: str, state_path: Path) -> str:
    """Submit a discovery to the autogenesis engine as a proper turn.
    The engine's LM extracts symbols and authors the graph transition.
    The scoring rule decides what survives — nothing is hardcoded.
    """
    try:
        result = subprocess.run(
            [BIN, "--state", str(state_path), "turn", text],
            capture_output=True, text=True, timeout=120
        )
        output = (result.stdout + result.stderr).strip()
        return output if output else "turn submitted (no output)"
    except subprocess.TimeoutExpired:
        return "ERROR: autogenesis turn timed out"
    except Exception as e:
        return f"ERROR: {e}"

        # If no structured format, extract as a free-text concept
        if not added_concepts and not added_relations:
            # Store as a summary concept
            cid = f"corpus_finding_{turn}"
            state.setdefault("concepts", {})[cid] = {
                "id": cid, "label": f"Corpus finding (turn {turn})",
                "summary": text[:300], "aliases": [], "status": "known",
                "first_seen_turn": turn, "last_seen_turn": turn, "mention_count": 1
            }
            added_concepts.append(cid)

        state_path.write_text(json.dumps(state, indent=2), encoding="utf-8")
        return f"written turn={turn} concepts={len(added_concepts)} relations={len(added_relations)}: {added_concepts[:3]} {added_relations[:3]}"
    except Exception as e:
        return f"ERROR: {e}"

def tool_sample_json(path: str, field: str = "title", keyword: str = "", n: int = 30) -> str:
    """Sample entries from a large JSON array, optionally filtering by keyword in a field."""
    p = Path(path)
    if not _allowed_read(p):
        return f"BLOCKED: {path}"
    try:
        with open(p, encoding="utf-8", errors="ignore") as f:
            data = json.load(f)
        if not isinstance(data, list):
            return f"Not a JSON array. Keys: {list(data.keys())[:10]}"
        if keyword:
            hits = [item for item in data if keyword.lower() in str(item.get(field, "")).lower()]
        else:
            hits = data
        out = []
        for item in hits[:n]:
            ts = str(item.get("timestamp", item.get("create_time", "")))[:10]
            title = str(item.get(field, item.get("title", "")))[:80]
            msgs = item.get("message_count", "")
            platform = item.get("platform", "")
            out.append(f"[{ts}] [{platform}] msgs={msgs} | {title}")
        return f"Found {len(hits)} matches (showing {min(n,len(hits))}):\n" + "\n".join(out)
    except Exception as e:
        return f"ERROR: {e}"

def tool_read_json_entry(path: str, keyword: str, field: str = "title", max_msgs: int = 8) -> str:
    """Read actual message content from a matched conversation in the chat vault."""
    p = Path(path)
    if not _allowed_read(p):
        return f"BLOCKED: {path}"
    try:
        with open(p, encoding="utf-8", errors="ignore") as f:
            data = json.load(f)
        hits = [item for item in data if keyword.lower() in str(item.get(field, "")).lower()]
        if not hits:
            return f"No match for '{keyword}' in field '{field}'"
        c = hits[0]
        msgs = c.get("messages", [])[:max_msgs]
        out = [f"Title: {c.get('title','')}", f"Platform: {c.get('platform','')}",
               f"Messages ({len(c.get('messages',[]))} total, showing {len(msgs)}):"]
        for m in msgs:
            role = m.get("role", m.get("author", {}).get("role", "?"))
            content = str(m.get("content", m.get("text", "")))[:400]
            out.append(f"\n[{role}]: {content}")
        return "\n".join(out)
    except Exception as e:
        return f"ERROR: {e}"

def tool_git_log(repo: str = ".", n: int = 40) -> str:
    """Read git commit history from a repo (read-only)."""
    p = Path(repo)
    if not _allowed_read(p) and str(p.resolve()) != str(Path("./").resolve()):
        return f"BLOCKED: {repo}"
    try:
        result = subprocess.run(
            ["git", "-C", str(p), "log", "--oneline", f"-{n}"],
            capture_output=True, text=True, timeout=10
        )
        return result.stdout.strip() or "no git history"
    except Exception as e:
        return f"ERROR: {e}"

def tool_read_codex_session(session_path: str, max_chars: int = 5000) -> str:
    """Read a codex/claude session log, extracting key messages."""
    p = Path(session_path)
    if not _allowed_read(p):
        return f"BLOCKED: {session_path}"
    try:
        lines = p.read_text(encoding="utf-8", errors="ignore").splitlines()
        out = []
        chars = 0
        for line in lines:
            if not line.strip():
                continue
            try:
                entry = json.loads(line)
                role = entry.get("role", entry.get("type", ""))
                content = str(entry.get("content", entry.get("message", "")))[:300]
                if content and role in ("user", "assistant", "human"):
                    out.append(f"[{role}]: {content}")
                    chars += len(content)
                    if chars > max_chars:
                        break
            except Exception:
                pass
        return "\n".join(out) if out else f"Raw (first {max_chars} chars):\n" + p.read_text()[:max_chars]
    except Exception as e:
        return f"ERROR: {e}"

def tool_graph_state(state_path: Path) -> str:
    try:
        result = subprocess.run(
            [BIN, "--state", str(state_path), "monitor"],
            capture_output=True, text=True, timeout=30
        )
        return result.stdout.strip()[:2000] if result.stdout.strip() else "no monitor output"
    except Exception as e:
        # Fallback: read state json directly
        try:
            state = json.loads(state_path.read_text())
            concepts = len(state.get("concepts", {}))
            relations = len(state.get("relations", {}))
            turn = state.get("turn", 0)
            seeds = state.get("seed_queue", [])
            tensions = state.get("tensions", [])
            return (f"turn={turn} concepts={concepts} relations={relations} "
                    f"seeds={len(seeds)} tensions={len(tensions)}")
        except Exception as e2:
            return f"ERROR reading state: {e2}"

# ── LM call ────────────────────────────────────────────────────────────────────

def get_api_key() -> str:
    for cmd in [
        ["security", "find-generic-password", "-s", "OPENROUTER_API_KEY", "-w"],
        ["security", "find-generic-password", "-s", "OPENAI_API_KEY", "-w"],
    ]:
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
            key = r.stdout.strip()
            if key:
                return key
        except Exception:
            pass
    return os.environ.get("OPENROUTER_API_KEY") or os.environ.get("OPENAI_API_KEY", "")

def lm_call(system: str, user: str, model: str = "google/gemini-3.1-flash-lite-preview") -> str:
    import urllib.request
    key = get_api_key()
    payload = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "max_tokens": 2000,
        "temperature": 0.3,
    }).encode()
    req = urllib.request.Request(
        "https://openrouter.ai/api/v1/chat/completions",
        data=payload,
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read())
            return data["choices"][0]["message"]["content"]
    except Exception as e:
        return f"LM_ERROR: {e}"

# ── orchestrator prompt ─────────────────────────────────────────────────────────

AGENT_SYSTEM = """You are an autonomous discovery agent with terminal-level access to a developer's full body of work.

Your goal: build a deep understanding of what this developer was trying to create — not surface descriptions, but the real intent, the pivots, why things were abandoned, what kept recurring.

When you find something worth committing, call graph_turn with your synthesis in plain prose. The autogenesis engine handles structuring it — you do not format CONCEPT/RELATION lines. Just describe what you discovered: what the system was, what it connected to, what problem it was solving, why it was abandoned or evolved.

RULES:
- Primary source: the conversation vault (715MB of actual design conversations). Start there.
- Read actual messages, not titles. Use sample_json to find relevant conversations, then read_json_entry for full content.
- Use scratchpad to synthesize across sources before committing.
- Don't describe filenames — discover intent, tensions, pivots, patterns that survived.
- graph_turn submits your synthesis to the autogenesis engine. The scoring rule decides what survives.

TOOLS:
{"tool": "read_file", "path": "/abs/path"}
{"tool": "list_dir", "path": "/abs/path"}
{"tool": "search", "path": "/abs/path", "term": "keyword"}
{"tool": "sample_json", "path": "/abs/path", "field": "title", "keyword": "graph", "n": 20}
{"tool": "read_json_entry", "path": "/abs/path", "keyword": "meta3", "field": "title"}
{"tool": "git_log", "repo": "/abs/path/to/repo", "n": 50}
{"tool": "read_codex_session", "path": "/abs/path/to/session.jsonl"}
{"tool": "write_scratchpad", "filename": "notes.md", "content": "..."}
{"tool": "graph_turn", "text": "prose synthesis of what you discovered — the engine structures it"}
{"tool": "graph_state"}

Respond ONLY as JSON: {"thinking": "...", "calls": [...]}
"""

def build_orchestrator_prompt(graph_summary: str, history: list, turn: int) -> str:
    sources = """TARGETED SOURCES — meta6/7/8 architectural abstractions. Read ALL of these:

PRIORITY 1 — Meta6 core abstractions (read these first):
   /Users/jobs/Desktop/tmp-meta3-engine-test/docs/META6_ABSTRACTION.md
   /Users/jobs/Desktop/tmp-meta3-engine-test/docs/META6_VISION.md
   /Users/jobs/Desktop/tmp-meta3-engine-test/docs/META6_LOOPBACK_SIMULATION.md
   /Users/jobs/Desktop/tmp-meta3-engine-test/docs/META6_SUPERIORITY.md
   /Users/jobs/Desktop/tmp-meta3-engine-test/docs/BOK_KNOWLEDGE_ECONOMY.md

PRIORITY 2 — Meta7 negentropy engine (actual Rust implementation):
   /Users/jobs/Desktop/Desktop(archive)/meta7-chatloop-negentropy/src/main.rs

PRIORITY 3 — Prime logic implementation:
   /Users/jobs/Desktop/mia-kernel/src/kernel/primes.rs  (if exists)
   /Users/jobs/Desktop/consolidatation/artifacts/history_contract/v4/dedup_signature_counts.txt

PRIORITY 4 — Conversation vault (search for these specific concepts):
   /Users/jobs/Desktop/tmp-meta3-engine-test/research/sources/history_miner_folder/input/chat.json
   → sample_json field="title" keyword="negentropy" / "prime logic" / "hyper graph" / "fluid kernel" / "void score" / "manifest" / "reducible" / "omni"

KEY CONCEPTS TO SURFACE INTO THE GRAPH:
- hyper_graph_manifest: system definition lives in graph, not code. Engine is generic runner.
- prime_logic: node addresses are primes (2=Evidence,3=Structure,5=Behavior,7=Memory,11=Operator,13=Emergence,17=OMNI). Composed concepts = products.
- negentropy_gate: adoption criterion is entropy reduction, not just inflation score.
- void_score: per-concept ignorance metric = 1000/(evidence_count+1). High void → deep search signal.
- omni_node: LM as fallback only — fires when no deterministic edge matches.
- fluid_kernel: RAM-only, TTL on concepts, productive forgetting, no accumulation.
- value_formula: Value = (Entropy_Start - Entropy_End) × Volume × Utility_Constant

Commit each concept with its relations and rationale via graph_turn."""

    recent_detail = ""
    for entry in history[-4:]:
        recent_detail += f"\n---\n{entry}\n"

    return f"""Turn {turn} of the discovery loop.

Current graph state:
{graph_summary}

{sources}

Recent actions and their results:
{recent_detail if recent_detail.strip() else '(none yet)'}

DIRECTIVE: Go deep. Read actual conversation messages from the vault. Read codex sessions. Read git logs.
Find the pivots — when did the developer abandon one approach for another, and why?
Find what survived — what concepts appear across meta2, meta3, dreaming-kernel, agentic-os, nstar-bit?
Commit specific relational discoveries as graph_turns, not summaries.

JSON only: {{"thinking": "...", "calls": [...]}}"""

# ── main loop ───────────────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--state", default=None, help="Autogenesis state file")
    p.add_argument("--turns", type=int, default=40, help="Max RLM loop turns")
    p.add_argument("--verbose", action="store_true")
    return p.parse_args()

def latest_fork() -> Path:
    forks = list(Path("epoch_logs").glob("epoch*_fork.json"))
    if not forks:
        raise FileNotFoundError("No epoch forks found")
    forks.sort(key=lambda p: int(re.search(r"epoch(\d+)", p.name).group(1)))
    return forks[-1]

def main():
    args = parse_args()
    SCRATCHPAD.mkdir(parents=True, exist_ok=True)
    LOG_FILE.parent.mkdir(exist_ok=True)

    STATE_FILE = Path(args.state) if args.state else latest_fork()
    print(f"State: {STATE_FILE}")

    history = []

    tools = {
        "read_file":         lambda c: tool_read_file(c["path"]),
        "list_dir":          lambda c: tool_list_dir(c["path"]),
        "search":            lambda c: tool_search(c["path"], c["term"]),
        "sample_json":       lambda c: tool_sample_json(c["path"], c.get("field","title"), c.get("keyword",""), c.get("n",30)),
        "read_json_entry":   lambda c: tool_read_json_entry(c["path"], c["keyword"], c.get("field","title")),
        "git_log":           lambda c: tool_git_log(c.get("repo","."), c.get("n",40)),
        "read_codex_session":lambda c: tool_read_codex_session(c["path"]),
        "write_scratchpad":  lambda c: tool_write_scratchpad(c["filename"], c["content"]),
        "graph_turn":        lambda c: tool_graph_turn(c["text"], STATE_FILE),
        "graph_state":       lambda c: tool_graph_state(STATE_FILE),
    }

    with open(LOG_FILE, "a") as log:
        log.write(f"\n\n{'#'*65}\n# Corpus agent — {time.strftime('%Y-%m-%d %H:%M')}\n{'#'*65}\n")

        for turn in range(1, args.turns + 1):
            print(f"\n[turn {turn:02d}] ", end="", flush=True)

            graph_summary = tool_graph_state(STATE_FILE)
            prompt = build_orchestrator_prompt(graph_summary, history, turn)

            response = lm_call(AGENT_SYSTEM, prompt)
            log.write(f"\n--- turn {turn} ---\nRESPONSE:\n{response}\n")

            if args.verbose:
                print(f"\n{response[:300]}")

            # Parse response
            try:
                # Extract JSON from response
                json_match = re.search(r'\{.*\}', response, re.DOTALL)
                if not json_match:
                    print("no JSON — retrying")
                    history.append(f"(no parseable response)")
                    continue

                parsed = json.loads(json_match.group())
                thinking = parsed.get("thinking", "")
                calls = parsed.get("calls", [])

                if thinking and args.verbose:
                    print(f"  thinking: {thinking[:150]}")

                if not calls:
                    calls = parsed if isinstance(parsed, list) else []

            except json.JSONDecodeError:
                print("JSON parse error — continuing")
                history.append("(parse error)")
                continue

            # Execute tool calls
            results = []
            for call in calls:
                tool_name = call.get("tool", "")
                if tool_name not in tools:
                    results.append(f"unknown tool: {tool_name}")
                    continue

                print(f"{tool_name} ", end="", flush=True)
                t0 = time.time()
                result = tools[tool_name](call)
                elapsed = time.time() - t0

                log.write(f"\nTOOL: {tool_name} ({elapsed:.1f}s)\nRESULT: {str(result)[:500]}\n")
                results.append(f"[{tool_name}] → {str(result)[:800]}")

            # Full results in history so next turn has context
            history.append(
                f"Turn {turn} thinking: {thinking[:120]}\n" +
                "\n".join(results[:4])
            )
            print(f"✓")

    print(f"\nDone. State: {STATE_FILE}")
    print(f"Scratchpad: {SCRATCHPAD}")
    print(f"Log: {LOG_FILE}")

if __name__ == "__main__":
    main()
