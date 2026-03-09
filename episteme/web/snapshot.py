#!/usr/bin/env python3
"""
snapshot.py — writes a live.json and appends to history.json after every turn.
Called by run_all_epochs.py after each task turn.
"""
import json
import sys
import time
from pathlib import Path

WEB_DATA = Path(__file__).parent / "data"
LIVE_JSON = WEB_DATA / "live.json"
HISTORY_JSON = WEB_DATA / "history.json"
HISTORY_MAX = 500  # keep the last 500 snapshots for charts


def write_snapshot(state_path: str, epoch: int, task_i: int, tasks_total: int):
    state_path = Path(state_path)
    if not state_path.exists():
        print(f"[snapshot] {state_path} not found", file=sys.stderr)
        return

    state = json.loads(state_path.read_text())
    health = state.get("latest_health", {})
    stats = health.get("stats", {})
    metrics = state.get("metrics", {})
    gate = state.get("latest_gate", {})
    active_focus = state.get("active_focus", [])
    seed_queue = state.get("seed_queue", [])
    relations = state.get("relations", {})
    concepts = state.get("concepts", {})
    evidence_log = state.get("evidence_log", [])

    # Top relations by confidence
    live_rels = [
        r for r in relations.values()
        if r.get("status") != "archived" and r.get("confidence", 0) > 0
    ]
    live_rels.sort(key=lambda r: r.get("confidence", 0), reverse=True)
    top_relations = [
        {
            "id": r["id"],
            "source": r.get("source", ""),
            "target": r.get("target", ""),
            "relation": r.get("relation", ""),
            "confidence": round(r.get("confidence", 0), 4),
            "evidence_for": r.get("evidence_for", 0),
            "evidence_against": r.get("evidence_against", 0),
        }
        for r in live_rels[:12]
    ]

    # Recent events
    events = state.get("events", [])
    recent_events = []
    for ev in events[-8:]:
        ext = ev.get("extraction") or {}
        recent_events.append({
            "turn": ev.get("turn", 0),
            "symbols": len(ext.get("concepts", [])) + len(ext.get("relations", [])),
            "focus": ext.get("active_focus", [])[:3],
        })

    # Confidence distribution
    all_conf = [r.get("confidence", 0) for r in relations.values() if r.get("status") != "archived"]
    dist = {"0": 0, "0-25": 0, "25-50": 0, "50-75": 0, "75-100": 0}
    for c in all_conf:
        if c == 0: dist["0"] += 1
        elif c < 0.25: dist["0-25"] += 1
        elif c < 0.50: dist["25-50"] += 1
        elif c < 0.75: dist["50-75"] += 1
        else: dist["75-100"] += 1

    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    live = {
        "updated_at": now,
        "run_id": state.get("run_lineage", {}).get("run_id", ""),
        "epoch": epoch,
        "task": task_i,
        "tasks_total": tasks_total,
        "turn": state.get("turn", 0),
        "health": {
            "healthy": health.get("healthy", True),
            "violation_count": len(health.get("violations", [])),
            "violations": health.get("violations", [])[:5],
            "inflation_score": round(stats.get("inflation_score", 0), 6),
            "unsupported_confident": stats.get("unsupported_confident", 0),
            "mean_confidence": round(stats.get("mean_confidence", 0), 4),
            "live_relations": stats.get("live_relations", 0),
        },
        "metrics": {
            "concepts": len(concepts),
            "relations": len(relations),
            "live_relations": len(live_rels),
            "evidence": len(evidence_log),
            "focus_size": len(active_focus),
            "seeds": len(seed_queue),
        },
        "gate": {
            "allow_act": gate.get("allow_act", False),
            "reason": gate.get("reason", ""),
        },
        "active_focus": active_focus[:8],
        "top_relations": top_relations,
        "recent_events": recent_events,
        "confidence_distribution": dist,
        "tensions": state.get("unresolved_tensions", [])[:5],
    }

    WEB_DATA.mkdir(exist_ok=True)
    LIVE_JSON.write_text(json.dumps(live, indent=2))

    # Append to history
    history_entry = {
        "t": now,
        "turn": live["turn"],
        "epoch": epoch,
        "concepts": live["metrics"]["concepts"],
        "relations": live["metrics"]["relations"],
        "evidence": live["metrics"]["evidence"],
        "live_relations": live["metrics"]["live_relations"],
        "inflation": live["health"]["inflation_score"],
        "mean_conf": live["health"]["mean_confidence"],
        "violations": live["health"]["violation_count"],
        "healthy": live["health"]["healthy"],
    }

    history = []
    if HISTORY_JSON.exists():
        try:
            history = json.loads(HISTORY_JSON.read_text())
        except Exception:
            history = []

    history.append(history_entry)
    if len(history) > HISTORY_MAX:
        history = history[-HISTORY_MAX:]

    HISTORY_JSON.write_text(json.dumps(history))
    print(f"[snapshot] turn={live['turn']} epoch={epoch} healthy={live['health']['healthy']} inflation={live['health']['inflation_score']:.4f}")


if __name__ == "__main__":
    # Usage: python3 snapshot.py <state_path> <epoch> <task_i> <tasks_total>
    if len(sys.argv) < 5:
        print("usage: snapshot.py <state> <epoch> <task_i> <tasks_total>")
        sys.exit(1)
    write_snapshot(sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]))
