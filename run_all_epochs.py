#!/usr/bin/env python3
"""Unified epoch runner — forks, plans, runs, and adopts indefinitely.

Each epoch:
  1. propose --execute   (Opus authors + executes fork)
  2. plan                (Opus authors task list for this epoch)
  3. run tasks           (Flash processes each turn)
  4. propose (dry-run)   (Opus authors next-epoch proposal)
  5. adopt if LM says so

Usage:
  python3 run_all_epochs.py --state nstar-autogenesis/epoch4_fork.json
  python3 run_all_epochs.py --state nstar-autogenesis/epoch4_fork.json --epochs 3
  python3 run_all_epochs.py --state nstar-autogenesis/epoch4_fork.json --resume
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

# Live dashboard snapshot — writes episteme/web/data/live.json after each turn
SNAPSHOT = Path(__file__).parent / "episteme" / "web" / "snapshot.py"


def write_snapshot(state_path: str, epoch: int, task_i: int, tasks_total: int):
    """Emit a live snapshot for the dashboard (non-fatal if it fails)."""
    try:
        import importlib.util
        spec = importlib.util.spec_from_file_location("snapshot", SNAPSHOT)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        mod.write_snapshot(state_path, epoch, task_i, tasks_total)
    except Exception as e:
        print(f"  [snapshot] skipped: {e}", file=sys.stderr)

parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument("--state", required=True, help="Starting state file")
parser.add_argument("--epochs", type=int, default=0, help="Number of epochs to run (0 = unlimited)")
parser.add_argument("--resume", action="store_true", help="Resume: skip fork+plan if state already has turns beyond start")
parser.add_argument("--log-dir", default="epoch_logs", help="Directory for per-epoch summaries")
parser.add_argument("--max-violations", type=int, default=10,
                    help="Stop for human review if violations exceed this count")
parser.add_argument("--inflation-threshold", type=float, default=0.1,
                    help="Auto-decay if inflation_score exceeds this")
parser.add_argument("--convergence-window", type=int, default=3,
                    help="Stop if no meaningful change in this many turns")
args = parser.parse_args()

BIN      = "target/debug/autogenesis"
LOG_DIR  = Path(args.log_dir)
LOG_DIR.mkdir(exist_ok=True)
SEP      = "━" * 65

# ── policy ─────────────────────────────────────────────────────────────────────
_POLICY_DEFAULTS = {
    "max_violations": 10,
    "inflation_threshold": 0.1,
    "convergence_window": 3,
    "deficit_contested_conf_floor": 0.3,
    "deficit_stale_conf_floor": 0.7,
    "deficit_ungrounded_mention_floor": 10,
    "deficit_agent_timeout_s": 90,
    "decay_evidence_floor": 0.1,
    "adoption_slope_floor": -0.01,
    "adoption_min_turns": 10,
    "adoption_failure_rate_cap": 0.5,
    "presence_window_size": 3,
}

_POLICY_FILE = LOG_DIR / "policy.json"

def _load_policy() -> dict:
    """Load policy.json, falling back to defaults for any missing keys."""
    p = dict(_POLICY_DEFAULTS)
    if _POLICY_FILE.exists():
        try:
            on_disk = json.loads(_POLICY_FILE.read_text())
            p.update({k: v for k, v in on_disk.items() if not k.startswith("_")})
        except Exception as e:
            print(f"[policy] failed to load {_POLICY_FILE}: {e} — using defaults", file=sys.stderr)
    return p

POLICY = _load_policy()
print(f"[policy] loaded v{POLICY.get('policy_version', '?')} from {_POLICY_FILE}")

# ── helpers ────────────────────────────────────────────────────────────────────

def run(*cmd: str, check: bool = False) -> subprocess.CompletedProcess:
    return subprocess.run(list(cmd), check=check)


def deficit_scan(state_path: str, max_tasks: int = 5) -> list[str]:
    """Scan graph state for epistemic deficits and return targeted agent tasks.

    Returns a list of turn prompts, each scoped to a specific concept or
    relation that needs evidence. These are disposable agents — each has
    one job: fill a specific gap in the graph.
    """
    state = load_state(state_path)
    if not state:
        return []

    concepts  = state.get("concepts", {})
    relations = state.get("relations", {})
    ev_log    = state.get("evidence_log", [])
    current_turn = state.get("turn", 0)

    dialogue_ids = {e["id"] for e in ev_log if e.get("origin") == "dialogue"}

    deficits: list[tuple[int, str]] = []  # (priority, task_prompt)

    # ── 1. Contested relations: evidence_against > evidence_for, conf > 0.3 ──
    for v in relations.values():
        if v.get("status") == "archived":
            continue
        ea = v.get("evidence_against", 0)
        ef = v.get("evidence_for", 0)
        conf = v.get("confidence", 0)
        if ea > ef and conf > POLICY["deficit_contested_conf_floor"]:
            src, rel, tgt = v["source"], v.get("relation", "?"), v["target"]
            deficits.append((
                10 + int(conf * 10),
                f"The relation '{src} {rel} {tgt}' has confidence {conf:.2f} but "
                f"{ea} disconfirming vs {ef} confirming evidence entries. "
                f"Apply targeted dialogue evidence to either confirm or formally disconfirm it, "
                f"updating its confidence based solely on dialogue-originated evidence."
            ))

    # ── 2. Stale high-confidence relations: conf > 0.7, no dialogue evidence ─
    for v in relations.values():
        if v.get("status") == "archived":
            continue
        conf = v.get("confidence", 0)
        support = set(v.get("support_set", []))
        has_dialogue = bool(support & dialogue_ids)
        if conf > POLICY["deficit_stale_conf_floor"] and not has_dialogue and v.get("evidence_for", 0) > 0:
            src, rel, tgt = v["source"], v.get("relation", "?"), v["target"]
            deficits.append((
                8,
                f"The relation '{src} {rel} {tgt}' has confidence {conf:.2f} but "
                f"zero dialogue-originated evidence in its support_set. "
                f"Inject one direct dialogue evidence entry that either grounds or "
                f"challenges this claim."
            ))

    # ── 3. High-mention concepts with no grounding in any relation ────────────
    grounded = {v["source"] for v in relations.values() if v.get("evidence_for", 0) > 0}
    grounded |= {v["target"] for v in relations.values() if v.get("evidence_for", 0) > 0}
    for v in concepts.values():
        if v.get("status") == "archived":
            continue
        name = v.get("label", v.get("name", ""))
        mc = v.get("mention_count", 0)
        if mc >= POLICY["deficit_ungrounded_mention_floor"] and name not in grounded:
            deficits.append((
                6,
                f"'{name}' has been mentioned {mc} times but has no evidence-backed "
                f"relations. Establish at least one relation from dialogue evidence "
                f"that grounds this concept in something testable."
            ))

    # ── 4. Active tensions from seed_queue ───────────────────────────────────
    # Only use seeds that are NOT actionable epoch proposals — those are
    # reserved for drain_engine_proposals() and become full fork cycles.
    EPOCH_KINDS = {"fork_proposal", "experiment", "test", "contradiction_resolution", "contrast", "repair"}
    for seed in state.get("seed_queue", [])[:5]:
        if not isinstance(seed, dict):
            continue
        kind   = seed.get("kind", "")
        prompt = seed.get("prompt", "").strip()
        if not prompt or kind in EPOCH_KINDS:
            continue  # reserve for epoch proposal, not a turn prompt
        deficits.append((5, f"Unresolved probe: {prompt}"))

    # Sort by priority descending, deduplicate, cap
    deficits.sort(key=lambda x: -x[0])
    seen: set[str] = set()
    tasks: list[str] = []
    for _, prompt in deficits:
        key = prompt[:60]
        if key not in seen:
            seen.add(key)
            tasks.append(prompt)
        if len(tasks) >= max_tasks:
            break

    return tasks


def drain_engine_proposals(state_path: str) -> list[dict]:
    """Read the engine's own seed_queue and unresolved_tensions.

    Seeds with actionable kinds become epoch proposals — not turn prompts.
    This is the feedback loop: the engine writes what it wants to explore,
    and the runner executes it as a full fork/compare/adopt cycle.

    Returns a list of proposal dicts (same shape as LmForkProposal), ordered
    by priority. Caller uses the first one instead of asking Opus to author a
    proposal from scratch.
    """
    state = load_state(state_path)
    if not state:
        return []

    ACTIONABLE_KINDS = {
        "fork_proposal", "experiment", "test",
        "contradiction_resolution", "contrast", "repair",
    }

    proposals = []

    # ── Seeds the engine emitted ──────────────────────────────────────────────
    for seed in state.get("seed_queue", []):
        if not isinstance(seed, dict):
            continue
        kind  = seed.get("kind", "")
        prompt = seed.get("prompt", "").strip()
        if not prompt or kind not in ACTIONABLE_KINDS:
            continue

        # Map seed kind to a success criterion the epoch gate can evaluate
        if kind == "contradiction_resolution":
            criterion = "violation_count_rhs < violation_count_lhs AND relation_delta >= 0"
        elif kind in ("test", "experiment"):
            criterion = "relation_delta > 2 AND inflation_score_rhs <= inflation_score_lhs"
        elif kind == "repair":
            criterion = "violation_count_rhs < violation_count_lhs"
        else:
            criterion = "relation_delta > 3"

        proposals.append({
            "proposed_change": prompt,
            "reason": f"engine-emitted seed (kind={kind})",
            "comparison_reason": f"Measure whether '{prompt[:60]}' reduced violations and grew relations.",
            "success_criterion": criterion,
            "should_adopt": False,   # gate decides, not us
            "adoption_reason": "",
            "_source": "seed_queue",
            "_kind": kind,
        })

    # ── Unresolved tensions the engine flagged ────────────────────────────────
    for tension in state.get("unresolved_tensions", []):
        if not isinstance(tension, str) or not tension.strip():
            continue
        proposals.append({
            "proposed_change": f"Resolve tension: {tension.strip()}",
            "reason": "engine-flagged unresolved tension",
            "comparison_reason": "Measure whether tension resolution reduced violations.",
            "success_criterion": "relation_delta > 1",
            "should_adopt": False,
            "adoption_reason": "",
            "_source": "unresolved_tensions",
            "_kind": "tension",
        })

    return proposals


def run_deficit_cycle(state_path: str, epoch: int, summary_file: str) -> int:
    """Run targeted deficit-agents against the current state.

    Each agent is a single Flash turn scoped to one graph gap.
    Writes results to the summary file. Returns number of agents run.
    """
    tasks = deficit_scan(state_path)
    if not tasks:
        print(f"  [deficit] No deficits found — graph is healthy")
        return 0

    print(f"  [deficit] {len(tasks)} deficit agent(s) to run:")
    for i, t in enumerate(tasks, 1):
        print(f"    {i}. {t[:100]}...")

    epoch_start_turn = state_turn(state_path)
    summary_path = Path(summary_file)
    ran = 0

    for i, task in enumerate(tasks, 1):
        print(f"\n  [deficit agent {i}/{len(tasks)}] {task[:80]}...")
        try:
            subprocess.run(
                autogenesis(state_path, "turn", task),
                timeout=POLICY["deficit_agent_timeout_s"]
            )
        except subprocess.TimeoutExpired:
            print(f"  [deficit agent {i}] timed out — skipping")
            continue
        ran += 1

        # Append to summary with real metrics
        st = load_state(state_path)
        m = st.get("metrics", {})
        focus = st.get("active_focus", [])
        row = (
            f"{epoch_start_turn}\tDEF-{i}\tok\t{st.get('turn','?')}\t"
            f"{m.get('concepts', len(st.get('concepts',{})))}\t"
            f"{m.get('relations', len(st.get('relations',{})))}\t"
            f"{m.get('evidence', len(st.get('evidence_log',[])))}\t"
            f"{','.join(focus[:3])[:50]}\t0\t0\tdeficit_targeted"
        )
        with open(summary_path, "a") as f:
            f.write(row + "\n")

    print(f"  [deficit] {ran} agent(s) ran")
    return ran


def autogenesis(state: str, *extra: str) -> list[str]:
    return [BIN, "--state", state] + list(extra)


def load_state(path: str) -> dict:
    try:
        return json.loads(Path(path).read_text())
    except Exception:
        return {}


def state_turn(path: str) -> int:
    return load_state(path).get("turn", 0)


def state_run_id(path: str) -> str:
    return load_state(path).get("run_lineage", {}).get("run_id", "?")


def state_status(path: str) -> str:
    return load_state(path).get("run_lineage", {}).get("status", "active")


def state_metrics(path: str) -> dict:
    s = load_state(path)
    return {
        "turn":      s.get("turn", 0),
        "concepts":  len(s.get("concepts", {})),
        "relations": len(s.get("relations", {})),
        "evidence":  len(s.get("evidence_log", [])),
        "focus":     s.get("active_focus", []),
        "gate":      s.get("latest_gate", {}).get("reason", ""),
        "tensions":  s.get("unresolved_tensions", []),
        "seeds":     len(s.get("seed_queue", [])),
    }


def load_health(path: str) -> dict:
    """Read the latest_health signal from state JSON."""
    s = load_state(path)
    return s.get("latest_health", {"healthy": True, "violations": [], "stats": {}})


def auto_decay(path: str) -> dict:
    """Run inline confidence decay — the decay_bootstrap.py logic, but as a function.
    Resets unsupported relations to evidence-derived confidence.
    Returns stats about what changed."""
    s = load_state(path)
    relations = s.get("relations", {})
    if not isinstance(relations, dict):
        return {"error": "relations is not a dict"}

    stats = {"decayed": 0, "total_delta": 0.0}
    for rel in relations.values():
        if rel.get("status") == "archived":
            continue
        ev_for = int(rel.get("evidence_for", 0))
        ev_against = int(rel.get("evidence_against", 0))
        old_conf = float(rel.get("confidence", 0.0))

        if ev_for == 0 and ev_against == 0:
            new_conf = 0.0
        else:
            total = ev_for + ev_against
            raw = ev_for / total
            floor = POLICY["decay_evidence_floor"] if ev_for > 0 else 0.0
            new_conf = min(raw + floor, 1.0)

        if abs(new_conf - old_conf) > 1e-6:
            stats["decayed"] += 1
            stats["total_delta"] += abs(new_conf - old_conf)
            rel["confidence"] = round(new_conf, 4)
            if "support_set" not in rel:
                rel["support_set"] = []

    if stats["decayed"] > 0:
        import time
        s["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        # Clear stale health violations so the next load_health reflects the decayed state
        if "latest_health" in s:
            s["latest_health"]["violations"] = []
            s["latest_health"]["healthy"] = True
            if "stats" in s["latest_health"]:
                s["latest_health"]["stats"]["inflation_score"] = 0.0
                s["latest_health"]["stats"]["unsupported_confident"] = 0
        Path(path).write_text(json.dumps(s, indent=2, ensure_ascii=True))
    return stats


def check_convergence(metrics_history: list, window: int = 3) -> bool:
    """True if the last `window` turns show no meaningful change."""
    if len(metrics_history) < window:
        return False
    recent = metrics_history[-window:]
    # Check if concepts, relations, and evidence counts are all stable
    concepts = [m["concepts"] for m in recent]
    relations = [m["relations"] for m in recent]
    evidence = [m["evidence"] for m in recent]
    return (
        max(concepts) - min(concepts) == 0
        and max(relations) - min(relations) == 0
        and max(evidence) - min(evidence) == 0
    )


def run_turn(state: str, prompt: str, max_retries: int = 5, base_wait: int = 20) -> bool:
    before = state_turn(state)
    for attempt in range(max_retries):
        run(*autogenesis(state, "turn", prompt))
        if state_turn(state) > before:
            return True
        if attempt < max_retries - 1:
            wait = min(base_wait * (2 ** attempt), 240)
            print(f"  !! turn did not advance — retrying in {wait}s")
            time.sleep(wait)
    return False


def propose_and_execute(state: str, fork_output: str) -> dict:
    """Call propose --execute and return the proposal JSON."""
    result = subprocess.run(
        autogenesis(state, "propose", "--fork-output", fork_output, "--execute"),
        capture_output=True, text=True
    )
    print(result.stdout)
    # parse proposal JSON from stdout (first JSON block)
    for line in result.stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                return json.loads(line)
            except Exception:
                pass
    # try multiline
    try:
        import re
        match = re.search(r'\{[^{}]*"proposed_change"[^{}]*\}', result.stdout, re.DOTALL)
        if match:
            return json.loads(match.group())
    except Exception:
        pass
    return {}


def plan_tasks(state: str, proposal: dict, tasks_file: str) -> list[str]:
    """Call plan with proposal JSON and write tasks file. Returns task list."""
    proposal_json = json.dumps(proposal)
    result = subprocess.run(
        autogenesis(state, "plan", "--proposal", proposal_json, "--output", tasks_file),
        capture_output=True, text=True
    )
    print(result.stdout)
    try:
        return [l.strip() for l in Path(tasks_file).read_text().splitlines() if l.strip()]
    except Exception:
        return []


THREAD_FILE = LOG_DIR / "thread.md"


def write_thread_brief(state: str, epoch: int, summary_file: str, prev_brief: str = "") -> str:
    """Call Opus to write a plain-language thread brief. Appends to thread.md."""
    s = load_state(state)
    tensions = s.get("unresolved_tensions", [])
    seeds = s.get("seed_queue", [])
    focus = s.get("active_focus", [])
    gate = s.get("latest_gate", {}).get("reason", "")
    turn = s.get("turn", 0)
    concepts_n = len(s.get("concepts", {}))
    relations_n = len(s.get("relations", {}))

    # Read last few summary rows for context
    try:
        rows = Path(summary_file).read_text().splitlines()[-4:]
        summary_rows = "\n".join(rows)
    except Exception:
        summary_rows = ""

    tensions_str = "\n".join(
        f"- {t if isinstance(t, str) else t.get('description', str(t))}"
        for t in tensions[:4]
    )
    seeds_str = "\n".join(
        f"- {s.get('prompt', str(s)) if isinstance(s, dict) else str(s)}"
        for s in seeds[:3]
    )

    prompt = f"""You are writing a session brief for a self-evolving epistemic graph system.
This brief IS the conversational thread — future sessions will read it to orient without needing to re-derive context.
Write 200-250 words. Plain language. No jargon padding.

Structure:
**Epoch {epoch} — Turn {turn} | {concepts_n} concepts, {relations_n} relations**

**What was being tested:** [1-2 sentences: the specific hypothesis or mechanism under investigation]
**What it found:** [1-2 sentences: what was proved, disproved, or adopted — be specific, name the mechanism]
**Why it matters:** [1 sentence: what architectural insight does this produce — not just "needs more testing"]
**Open question:** [1 sentence: the specific falsifiable claim the next epoch must resolve]
**Watch for:** [1 sentence: what failure mode or drift to watch for]
**Resume:** python3 run_all_epochs.py --state epoch_logs/epoch{epoch}_fork.json

Current state:
- Focus: {focus}
- Gate: {gate[:120]}
- Tensions:
{tensions_str}
- Seeds:
{seeds_str}
- Recent task results:
{summary_rows}

Previous brief (for continuity):
{prev_brief[-600:] if prev_brief else "None — this is the first brief."}

Write only the brief. No preamble."""

    # Build a structured brief from data (no LM call needed — always works)
    brief = (
        f"**Epoch {epoch} — Turn {turn} | {concepts_n} concepts, {relations_n} relations**\n\n"
        f"Focus: {', '.join(focus) or '—'}\n\n"
        f"{s.get('summary', '')[:200]}\n\n"
        f"Gate: {gate}\n\n"
        f"Open tensions:\n{tensions_str or '  - (none)'}\n\n"
        f"Seeds for next epoch:\n{seeds_str or '  - (none)'}"
    )

    # Attempt LM enhancement (non-blocking — falls back to structured brief on any error)
    try:
        import os, urllib.request
        api_key = os.environ.get("OPENAI_API_KEY") or ""
        if not api_key:
            api_key = subprocess.run(
                ["security", "find-generic-password", "-s", "OPENAI_API_KEY", "-w"],
                capture_output=True, text=True
            ).stdout.strip()

        payload = json.dumps({
            "model": "gpt-5.3-chat-latest",
            "messages": [{"role": "user", "content": prompt}],
            "max_completion_tokens": 400,
        }).encode()

        req = urllib.request.Request(
            "https://api.openai.com/v1/chat/completions",
            data=payload,
            headers={"Content-Type": "application/json", "Authorization": f"Bearer {api_key}"},
            method="POST"
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read())
            brief = data["choices"][0]["message"]["content"].strip()
    except Exception:
        pass  # structured brief already set above

    # Append to thread.md
    divider = "\n\n---\n\n"
    existing = THREAD_FILE.read_text() if THREAD_FILE.exists() else ""
    THREAD_FILE.write_text(existing + divider + brief if existing else brief)
    print(f"\n[thread] Brief written to {THREAD_FILE}")

    # ── Architectural reflection ────────────────────────────────────────────
    # Ask the LM: does this epoch's finding imply any change to the runner/architecture?
    # Writes actionable recommendations to FINDINGS.md without waiting for a human to ask.
    _reflect_on_findings(epoch, brief, tensions, s)

    return brief


FINDINGS_FILE = Path(__file__).parent / "FINDINGS.md"


def _reflect_on_findings(epoch: int, brief: str, tensions: list, state: dict) -> None:
    """After each epoch brief, ask Sonnet if graph findings imply architectural changes.
    Appends concrete recommendations to FINDINGS.md automatically."""
    try:
        import os, urllib.request
        api_key = os.environ.get("OPENROUTER_API_KEY") or os.environ.get("OPENAI_API_KEY") or ""
        if not api_key:
            api_key = subprocess.run(
                ["security", "find-generic-password", "-s", "OPENROUTER_API_KEY", "-w"],
                capture_output=True, text=True
            ).stdout.strip()
        if not api_key:
            api_key = subprocess.run(
                ["security", "find-generic-password", "-s", "OPENAI_API_KEY", "-w"],
                capture_output=True, text=True
            ).stdout.strip()
        if not api_key:
            return

        # Read current FINDINGS.md to avoid duplicating known items
        existing_findings = FINDINGS_FILE.read_text()[-1500:] if FINDINGS_FILE.exists() else ""

        adoption_verdict = state.get("run_lineage", {}).get("status", "unknown")
        top_relations = sorted(
            state.get("relations", {}).values(),
            key=lambda r: r.get("confidence", 0), reverse=True
        )[:5]
        rel_summary = "; ".join(
            f"{r['source']} {r.get('relation','?')} {r['target']} (conf={r.get('confidence',0):.2f} for={r.get('evidence_for',0)} against={r.get('evidence_against',0)})"
            for r in top_relations
        )

        prompt = f"""You are reviewing a self-evolving epistemic graph system after epoch {epoch}.
Your job: identify if this epoch's findings imply any concrete change to the runner architecture, prompts, or thresholds.
Only flag things that are ACTIONABLE and NOT already known.

Epoch brief:
{brief[:600]}

Adoption verdict: {adoption_verdict}
Top relations: {rel_summary}
Unresolved tensions: {'; '.join(str(t) for t in tensions[:3])}

Already known in FINDINGS.md (do not repeat):
{existing_findings[-800:]}

Respond with ONE of:
1. "NO_ACTION" — if the epoch found nothing that implies an architectural change
2. A short bullet (1-3 lines) describing a SPECIFIC actionable change: what to change, why, what outcome to expect.
   Format: "ARCH [{epoch}]: <change> — because <finding> — expected: <outcome>"

Be conservative. Only flag genuine architectural signals, not routine exploration."""

        payload = json.dumps({
            "model": os.environ.get("ROUTER_META_MODEL", "anthropic/claude-sonnet-4-6"),
            "messages": [{"role": "user", "content": prompt}],
            "max_completion_tokens": 200,
        }).encode()

        url = os.environ.get("ROUTER_URL", "https://openrouter.ai/api/v1/chat/completions")
        req = urllib.request.Request(
            url, data=payload,
            headers={"Content-Type": "application/json", "Authorization": f"Bearer {api_key}"},
            method="POST"
        )
        with urllib.request.urlopen(req, timeout=20) as resp:
            result = json.loads(resp.read())
            recommendation = result["choices"][0]["message"]["content"].strip()

        if recommendation and recommendation != "NO_ACTION" and not recommendation.startswith("NO_ACTION"):
            findings_text = FINDINGS_FILE.read_text() if FINDINGS_FILE.exists() else ""
            FINDINGS_FILE.write_text(findings_text + f"\n\n{recommendation}\n")
            print(f"\n[findings] Architectural signal written: {recommendation[:80]}...")
        else:
            print(f"\n[findings] No architectural signal this epoch.")

    except Exception as e:
        print(f"\n[findings] Reflection skipped: {e}")


_last_brief = ""  # carry forward for continuity


PORTFOLIO_FILE = LOG_DIR / "portfolio.json"

# Receipt fields that can be evaluated by the structured DSL.
# Any field present in the portfolio entry is eligible.
_RECEIPT_FIELDS = {
    "inflation_score_lhs", "inflation_score_rhs", "inflation_delta",
    "unsupported_confident_lhs", "unsupported_confident_rhs",
    "violation_count_lhs", "violation_count_rhs",
    "concept_delta", "relation_delta", "evidence_delta",
    "archived_concept_delta", "archived_relation_delta",
    "rejected_field_delta",
}

_OPS = {"<": float.__lt__, ">": float.__gt__, "<=": float.__le__,
        ">=": float.__ge__, "==": float.__eq__, "!=": float.__ne__}


def _eval_structured(criterion: str, receipt: dict) -> Optional[bool]:
    """Try to evaluate a structured predicate against receipt fields.

    Format: FIELD OP VALUE [AND FIELD OP VALUE ...]
    Returns True/False if fully parseable; None if not structured.
    """
    import re
    parts = [p.strip() for p in re.split(r'\bAND\b', criterion, flags=re.IGNORECASE)]
    results = []
    for part in parts:
        m = re.fullmatch(
            r'(\w+)\s*(<=|>=|==|!=|<|>)\s*(-?\d+(?:\.\d+)?)',
            part.strip()
        )
        if not m:
            return None  # not parseable
        field, op, val = m.group(1), m.group(2), float(m.group(3))
        if field not in _RECEIPT_FIELDS:
            return None  # unknown field — don't guess
        actual = receipt.get(field)
        if actual is None:
            return None
        results.append(_OPS[op](float(actual), val))
    return all(results) if results else None


def _eval_criterion_lm(criterion: str, receipt: dict, state_summary: str) -> tuple[bool, str]:
    """Ask the LM to evaluate a natural-language criterion against receipt data.

    Returns (criterion_met: bool, evidence: str).
    Falls back to (True, 'not evaluated') on any error so adoption is not silently blocked.
    """
    import urllib.request, urllib.error
    try:
        api_key = subprocess.run(
            ["security", "find-generic-password", "-s", "OPENROUTER_API_KEY", "-w"],
            capture_output=True, text=True
        ).stdout.strip()
        if not api_key:
            api_key = subprocess.run(
                ["security", "find-generic-password", "-s", "OPENAI_API_KEY", "-w"],
                capture_output=True, text=True
            ).stdout.strip()
        if not api_key:
            return True, "no api key — criterion not evaluated"

        meta_model = os.environ.get("ROUTER_META_MODEL", "anthropic/claude-sonnet-4-6")
        receipt_str = json.dumps({k: receipt.get(k) for k in sorted(_RECEIPT_FIELDS) if receipt.get(k) is not None}, indent=2)

        prompt = f"""Evaluate whether this success criterion was met based on the evidence provided.

SUCCESS CRITERION:
{criterion}

EPOCH RECEIPT (measured outcomes):
{receipt_str}

STATE SUMMARY (abbreviated):
{state_summary[:800]}

Answer with strict JSON only:
{{"criterion_met": true|false, "evidence": "one sentence citing specific numbers from the receipt that confirm or refute the criterion"}}"""

        body = json.dumps({
            "model": meta_model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 200,
            "temperature": 0,
        }).encode()
        req = urllib.request.Request(
            "https://openrouter.ai/api/v1/chat/completions",
            data=body,
            headers={"Content-Type": "application/json", "Authorization": f"Bearer {api_key}"},
            method="POST"
        )
        with urllib.request.urlopen(req, timeout=20) as resp:
            result = json.loads(resp.read())
            text = result["choices"][0]["message"]["content"].strip()
            # parse JSON from response
            import re
            m = re.search(r'\{[^{}]*"criterion_met"[^{}]*\}', text, re.DOTALL)
            if m:
                parsed = json.loads(m.group())
                return bool(parsed.get("criterion_met", True)), parsed.get("evidence", "")
    except Exception as e:
        print(f"  [criterion] LM evaluation failed: {e}", file=sys.stderr)
    return True, "evaluation error — defaulting to not blocked"


def evaluate_criterion(criterion: str, receipt: dict, state_summary: str = "") -> tuple:
    """Evaluate a success criterion against a portfolio receipt.

    1. Try structured DSL (deterministic, no LM call).
    2. Fall back to LM evaluation for natural language.
    Returns (criterion_met: bool|None, method: str).
    None means 'not evaluable' — adoption is not blocked.
    """
    if not criterion or criterion.strip() == "":
        return None, "no_criterion"

    result = _eval_structured(criterion, receipt)
    if result is not None:
        return result, "structured"

    met, evidence = _eval_criterion_lm(criterion, receipt, state_summary)
    return met, f"lm:{evidence[:80]}"


def _write_portfolio_entry(epoch: int, receipt: dict, adopted: bool, proposal: dict, state_summary: str = "") -> None:
    """Append a typed receipt entry to portfolio.json after each epoch.

    Receives the pre-computed comparison receipt (from the caller's compare step).
    """
    import time as _time
    criterion = proposal.get("success_criterion", "")
    criterion_result, criterion_method = evaluate_criterion(criterion, receipt, state_summary)
    print(f"  [criterion] epoch {epoch}: met={criterion_result} via {criterion_method}")

    entry = {
        "epoch": epoch,
        "adopted": adopted,
        "policy_version": POLICY.get("policy_version", 0),
        "success_criterion": criterion,
        "criterion_met": criterion_result,
        "criterion_method": criterion_method,
        "should_adopt_lm": proposal.get("should_adopt", False),
        "adoption_reason": proposal.get("adoption_reason", ""),
        "ts": _time.strftime("%Y-%m-%dT%H:%M:%SZ", _time.gmtime()),
        # structural deltas
        "concept_delta": receipt.get("concept_delta", 0),
        "relation_delta": receipt.get("relation_delta", 0),
        "evidence_delta": receipt.get("evidence_delta", 0),
        "archived_concept_delta": receipt.get("archived_concept_delta", 0),
        "archived_relation_delta": receipt.get("archived_relation_delta", 0),
        # health signals
        "inflation_score_lhs": receipt.get("inflation_score_lhs", 0.0),
        "inflation_score_rhs": receipt.get("inflation_score_rhs", 0.0),
        "inflation_delta": receipt.get("inflation_score_rhs", 0.0) - receipt.get("inflation_score_lhs", 0.0),
        "unsupported_confident_lhs": receipt.get("unsupported_confident_lhs", 0),
        "unsupported_confident_rhs": receipt.get("unsupported_confident_rhs", 0),
        "violation_count_lhs": receipt.get("violation_count_lhs", 0),
        "violation_count_rhs": receipt.get("violation_count_rhs", 0),
        "comparison_id": receipt.get("comparison_id", ""),
    }

    portfolio: list = []
    if PORTFOLIO_FILE.exists():
        try:
            portfolio = json.loads(PORTFOLIO_FILE.read_text())
        except Exception:
            portfolio = []
    portfolio.append(entry)
    PORTFOLIO_FILE.write_text(json.dumps(portfolio, indent=2))
    print(f"  [portfolio] epoch {epoch} receipt written ({len(portfolio)} total entries)")
    return criterion_result  # None=not evaluable, True=met, False=not met


def propose_dry(state: str, fork_output: str) -> dict:
    """Dry-run propose (no --execute). Returns proposal JSON."""
    result = subprocess.run(
        autogenesis(state, "propose", "--fork-output", fork_output),
        capture_output=True, text=True
    )
    print(result.stdout)
    for line in result.stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                return json.loads(line)
            except Exception:
                pass
    try:
        import re
        match = re.search(r'\{[^{}]*"proposed_change"[^{}]*\}', result.stdout, re.DOTALL)
        if match:
            return json.loads(match.group())
    except Exception:
        pass
    return {}


# ── main loop ──────────────────────────────────────────────────────────────────

current_state = args.state
epochs_run = 0

# Infer starting epoch from state filename (e.g. epoch32_fork.json → start at 33)
import re as _re_init
_epoch_match = _re_init.search(r'epoch(\d+)_fork\.json', str(args.state))
epoch = (int(_epoch_match.group(1)) + 1) if _epoch_match else 1
resume_from_task = 0  # index into task list to start from (for partial epochs)

# ── Resume: find last epoch with real work, handle partials and zombies ────
if args.resume:
    import re as _re

    def _epoch_tasks_done(n: int) -> int:
        """Number of completed task rows in summary (header doesn't count)."""
        s = LOG_DIR / f"epoch{n}_summary.tsv"
        if not s.exists():
            return -1  # no summary at all
        lines = [l for l in s.read_text().splitlines() if l.strip()]
        return max(0, len(lines) - 1)  # subtract header

    def _epoch_tasks_total(n: int) -> int:
        t = LOG_DIR / f"epoch{n}_tasks.txt"
        if not t.exists():
            return 0
        return len([l for l in t.read_text().splitlines() if l.strip()])

    existing = sorted(LOG_DIR.glob("epoch*_fork.json"))
    nums = sorted([int(m.group(1)) for f in existing
                   if (m := _re.search(r'epoch(\d+)_fork\.json', f.name))])

    if nums:
        # Find the last epoch with at least 1 completed task (not a zombie)
        last_real = 0
        for n in nums:
            if _epoch_tasks_done(n) > 0:
                last_real = n

        if last_real == 0:
            print("[resume] No epochs with completed tasks found, starting fresh")
        else:
            done = _epoch_tasks_done(last_real)
            total = _epoch_tasks_total(last_real)
            last_fork = LOG_DIR / f"epoch{last_real}_fork.json"

            if done >= total and total > 0:
                # Last real epoch fully complete — start next epoch fresh
                epoch = last_real + 1
                current_state = str(last_fork)
                print(f"[resume] Epochs 1–{last_real} complete. Starting epoch {epoch} from {current_state}")
            else:
                # Last real epoch is partial — resume it from where it left off
                epoch = last_real
                current_state = str(last_fork)
                resume_from_task = done
                print(f"[resume] Epoch {epoch} partial ({done}/{total} tasks done). Resuming from task {done+1}.")
    else:
        print("[resume] No existing epochs found, starting fresh")

print(f"\n{SEP}")
print(f"Starting auto-epoch loop from: {current_state}")
print(f"Turn model: google/gemini-3-flash-preview (Flash)")
print(f"Meta model: anthropic/claude-opus-4-6 (Opus)")
print(f"Epochs: {'unlimited' if args.epochs == 0 else args.epochs}")
print(SEP)

while True:
    if args.epochs > 0 and epochs_run >= args.epochs:
        print(f"\nReached --epochs {args.epochs}. Done.")
        break

    fork_path   = str(LOG_DIR / f"epoch{epoch}_fork.json")
    tasks_file  = str(LOG_DIR / f"epoch{epoch}_tasks.txt")
    summary_file = str(LOG_DIR / f"epoch{epoch}_summary.tsv")

    print(f"\n{SEP}")
    print(f"EPOCH {epoch}  (current state: {current_state}  turn={state_turn(current_state)})")
    print(SEP)

    # ── 1. Fork (skip if resuming a partial epoch — fork already exists) ───────
    is_resuming_partial = (resume_from_task > 0 and Path(fork_path).exists()
                           and Path(tasks_file).exists())
    if is_resuming_partial:
        print(f"\n[Epoch {epoch}] Resuming partial epoch — reusing existing fork and tasks")
        proposal = {}
        tasks = [l.strip() for l in Path(tasks_file).read_text().splitlines() if l.strip()]
        print(f"  fork={fork_path}  tasks={len(tasks)}  resuming from task {resume_from_task+1}")
    else:
        print(f"\n[Epoch {epoch}] Proposing fork → {fork_path}")

        # ── Check if the engine already authored a proposal ────────────────
        # drain_engine_proposals() reads seed_queue + unresolved_tensions.
        # If the engine has something to say, use it — don't ask Opus to invent.
        engine_proposals = drain_engine_proposals(current_state)
        if engine_proposals:
            ep = engine_proposals[0]
            print(f"  [engine proposal] kind={ep.get('_kind')} — {ep['proposed_change'][:80]}")
            if len(engine_proposals) > 1:
                print(f"  ({len(engine_proposals)-1} additional engine proposals queued for future epochs)")
            # Fork using the engine's proposed_change directly
            import shutil
            shutil.copy2(current_state, fork_path)
            run(*autogenesis(fork_path, "fork",
                             "--output", fork_path,
                             "--proposed-change", ep["proposed_change"],
                             "--reason", ep["reason"]))
            proposal = ep
        else:
            proposal = propose_and_execute(current_state, fork_path)

        if not Path(fork_path).exists():
            # Retry once with a minimal fallback proposal
            print(f"  !! Fork file not created. Retrying with fallback proposal...")
            import shutil
            fallback = {
                "proposed_change": "Continue probing the highest-tension unresolved concept in the graph.",
                "reason": "Fallback: previous proposal failed to produce a fork.",
                "comparison_reason": "Compare confidence deltas on top contested relations.",
                "success_criterion": "At least one contested relation gains evidence_for > evidence_against.",
                "should_adopt": False,
                "adoption_reason": ""
            }
            shutil.copy2(current_state, fork_path)
            proposal = fallback
            print(f"  Fallback fork created from current state.")
        if not Path(fork_path).exists():
            print(f"  !! Fork file still not created. Aborting epoch {epoch}.")
            break
        print(f"  run_id={state_run_id(fork_path)}  proposed_change={proposal.get('proposed_change','?')[:80]}")

        # ── 1b. Deficit agents — targeted passes before generic planning ───────
        print(f"\n[Epoch {epoch}] Running deficit agents (graph-driven)...")
        run_deficit_cycle(fork_path, epoch, summary_file)

        # ── 2. Plan ────────────────────────────────────────────────────────────
        print(f"\n[Epoch {epoch}] Planning tasks → {tasks_file}")
        tasks = plan_tasks(fork_path, proposal, tasks_file)
        if not tasks:
            # Retry once with a minimal generic proposal
            print(f"  !! No tasks generated. Retrying plan with minimal proposal...")
            minimal_proposal = {
                "proposed_change": proposal.get("proposed_change") or "Probe the highest-tension unresolved concept.",
                "reason": "Retry after empty task generation.",
                "success_criterion": "At least one contested relation has its confidence updated by dialogue evidence.",
                "should_adopt": False,
                "adoption_reason": ""
            }
            tasks = plan_tasks(fork_path, minimal_proposal, tasks_file)
        if not tasks:
            print(f"  !! No tasks generated after retry. Aborting epoch {epoch}.")
            break
        print(f"  {len(tasks)} tasks authored by Opus")

    # ── 3. Run tasks ───────────────────────────────────────────────────────────
    epoch_start_turn = state_turn(fork_path)
    metrics_history = []
    epoch_stopped = False
    epoch_converged = False

    # When resuming partial: append to existing summary; otherwise write fresh header
    if is_resuming_partial:
        # summary already has resume_from_task rows — just append going forward
        pass
    else:
        Path(summary_file).write_text(
            "epoch_start_turn\ttask\tresult\tturn\tconcepts\trelations"
            "\tevidence\tfocus\tseeds\ttensions\tgate\n"
        )

    for i, prompt in enumerate(tasks, 1):
        if i <= resume_from_task:
            continue  # skip already-done tasks
        m_before = state_metrics(fork_path)
        print(f"\n  TASK {i}/{len(tasks)}  (turn {m_before['turn']} → {m_before['turn']+1})")
        print(f"  {prompt[:110]}{'…' if len(prompt) > 110 else ''}")

        ok = run_turn(fork_path, prompt)
        result = "ok" if ok else "failed"

        m = state_metrics(fork_path)
        focus_str = ",".join(m.get("focus", []))[:60]
        tensions  = len(m.get("tensions", []))
        gate_str  = m.get("gate", "")[:80]

        print(f"  turn={m['turn']}  concepts={m['concepts']}  relations={m['relations']}  evidence={m['evidence']}")
        print(f"  gate={gate_str or '—'}")

        # ── Health check after every turn ──────────────────────────────────
        health = load_health(fork_path)
        h_stats = health.get("stats", {})
        violations = health.get("violations", [])
        is_healthy = health.get("healthy", True)

        health_line = (
            f"  health: {'✅' if is_healthy else '⚠️'}  "
            f"inflation={h_stats.get('inflation_score', 0):.4f}  "
            f"unsupported={h_stats.get('unsupported_confident', 0)}  "
            f"violations={len(violations)}"
        )
        print(health_line)

        # Auto-correct: inflation detected → run inline decay
        inflation = h_stats.get("inflation_score", 0)
        if inflation > POLICY["inflation_threshold"]:
            print(f"  ⚠ inflation_score={inflation:.4f} > {POLICY['inflation_threshold']} — running auto-decay")
            decay_stats = auto_decay(fork_path)
            print(f"    decayed {decay_stats['decayed']} relations, total_delta={decay_stats['total_delta']:.4f}")
            # Reload health after decay so violation check sees corrected state
            health = load_health(fork_path)
            violations = health.get("violations", [])

        # Stop condition: too many violations → surface to human
        if len(violations) > POLICY["max_violations"]:
            print(f"  🛑 {len(violations)} violations exceed max_violations={POLICY['max_violations']}")
            print(f"     Stopping for human review.")
            for v in violations[:5]:
                print(f"     - [{v.get('kind')}] {v.get('entity_id')}: {v.get('detail')}")
            epoch_stopped = True
            break

        # Track metrics for convergence detection
        metrics_history.append(m)

        # ── Live snapshot for dashboard ─────────────────────────────────────
        write_snapshot(fork_path, epoch, i, len(tasks))

        with open(summary_file, "a") as f:
            f.write(
                f"{epoch_start_turn}\t{i}\t{result}\t{m['turn']}\t{m['concepts']}\t"
                f"{m['relations']}\t{m['evidence']}\t{focus_str}\t"
                f"{m['seeds']}\t{tensions}\t{gate_str}\n"
            )

    # Check convergence after all tasks in this epoch
    if not epoch_stopped and check_convergence(metrics_history, POLICY["convergence_window"]):
        print(f"\n  ✅ Converged: no meaningful change in last {POLICY['convergence_window']} turns.")
        epoch_converged = True

    # ── 4. Show final state ────────────────────────────────────────────────────
    print(f"\n{SEP}")
    print(f"Epoch {epoch} complete")
    run(*autogenesis(fork_path, "show"))

    # ── 4b. Write thread brief ─────────────────────────────────────────────────
    _last_brief = write_thread_brief(fork_path, epoch, summary_file, _last_brief)

    # ── 5. Print summary table ─────────────────────────────────────────────────
    rows = [line.split("\t") for line in Path(summary_file).read_text().splitlines() if line.strip()]
    if rows:
        widths = [max(len(r[c]) for r in rows if c < len(r)) for c in range(len(rows[0]))]
        for row in rows:
            print("  ".join(cell.ljust(widths[c]) for c, cell in enumerate(row)))

    # ── 6. Compute epoch receipt (needed for criterion gate + portfolio) ────────
    _cmp_result = subprocess.run(
        autogenesis(current_state, "compare",
                    "--other-state", fork_path,
                    "--reason", f"epoch{epoch}_end",
                    "--json"),
        capture_output=True, text=True
    )
    _epoch_receipt: dict = {}
    try:
        _epoch_receipt = json.loads(_cmp_result.stdout)
    except Exception:
        pass
    if not _epoch_receipt:
        for _line in _cmp_result.stdout.splitlines():
            _line = _line.strip()
            if _line.startswith("{"):
                try:
                    _epoch_receipt = json.loads(_line)
                    break
                except Exception:
                    pass

    _state_summary = json.dumps(load_state(fork_path).get("run_lineage", {}))[:500]

    # ── 6b. Criterion gate — evaluate before allowing LM adoption vote ─────────
    _adopted = False
    _final_proposal = proposal
    _criterion = proposal.get("success_criterion", "")
    _criterion_gate, _criterion_method = evaluate_criterion(_criterion, _epoch_receipt, _state_summary)
    print(f"\n[Epoch {epoch}] Criterion gate: met={_criterion_gate} via {_criterion_method}")

    if _criterion_gate is True:
        # Criterion definitively met — adopt without requiring LM veto.
        # The gate already evaluated the evidence; LM should_adopt is advisory only.
        print(f"  Criterion met ({_criterion_method}) — adopting fork")
        run(*autogenesis(fork_path, "adopt",
                         "--reason", f"criterion met via {_criterion_method}: {_criterion}"))
        _adopted = True
    elif proposal.get("should_adopt"):
        if _criterion_gate is False:
            print(f"  LM recommends adoption but success_criterion not met — skipping")
        else:
            # Criterion unevaluable (None) and LM recommends adoption — proceed
            print(f"  Adopting fork (LM recommended, criterion unevaluable)")
            run(*autogenesis(fork_path, "adopt",
                             "--reason", proposal.get("adoption_reason", "LM-recommended adoption")))
            _adopted = True
    else:
        # Ask Opus to re-evaluate now that all tasks are done
        next_fork_path = str(LOG_DIR / f"epoch{epoch+1}_fork.json")
        print(f"  Evaluating adoption via Opus...")
        next_proposal = propose_dry(fork_path, next_fork_path)
        _final_proposal = next_proposal
        # Re-use the same receipt but evaluate against next_proposal's criterion if different
        _criterion2 = next_proposal.get("success_criterion") or _criterion
        _criterion_gate2, _criterion_method2 = evaluate_criterion(_criterion2, _epoch_receipt, _state_summary)
        if _criterion_gate2 is True:
            print(f"  Criterion met ({_criterion_method2}) after Opus review — adopting fork")
            run(*autogenesis(fork_path, "adopt",
                             "--reason", f"criterion met via {_criterion_method2}: {_criterion2}"))
            _adopted = True
        elif next_proposal.get("should_adopt"):
            if _criterion_gate2 is False:
                print(f"  Opus recommends adoption but success_criterion not met — skipping")
            else:
                print(f"  Opus recommends adoption after review")
                run(*autogenesis(fork_path, "adopt",
                                 "--reason", next_proposal.get("adoption_reason", "post-epoch Opus review")))
                _adopted = True
        else:
            print(f"  Opus: further experiment needed — fork stays active")

    # ── 6c. Write portfolio receipt ────────────────────────────────────────────
    _write_portfolio_entry(epoch, _epoch_receipt, _adopted, _final_proposal, _state_summary)

    resume_from_task = 0  # only applies to the first epoch when resuming

    # ── 7. Advance ────────────────────────────────────────────────────────────
    if epoch_stopped:
        print(f"\n  Epoch {epoch} stopped due to violations. Human review needed.")
        break
    if epoch_converged:
        print(f"\n  Epoch {epoch} converged. System is stable.")
        break

    current_state = fork_path
    epoch += 1
    epochs_run += 1

print(f"\n{SEP}")
print(f"Loop ended after {epochs_run} epoch(s). Final state: {current_state}")
print(SEP)
run(*autogenesis(current_state, "show"))
