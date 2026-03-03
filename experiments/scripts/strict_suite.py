#!/usr/bin/env python3
import json
import os
import statistics
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Dict, List, Optional, Tuple

ROOT = Path('/Users/jobs/Developer/nstar-bit')
PROMPTS_DIR = ROOT / 'experimets' / 'prompts'
OUT_BASE = ROOT / 'experimets' / 'output_strict'

NSTAR_BIN = ROOT / 'target' / 'debug' / 'nstar-bit'
CANON_BIN = ROOT / 'target' / 'debug' / 'canonical'

FAIL_QUALITY_THRESHOLD = 0.90

CODE_KEYWORDS = {
    'code', 'rust', 'bug', 'test', 'api', 'auth', 'parser', 'sql', 'latency', 'cache', 'thread',
    'panic', 'utf', 'async', 'lock', 'service'
}
STRATEGY_KEYWORDS = {
    'strategy', 'operator', 'governance', 'decision', 'portfolio', 'stakeholder', 'organization',
    'risk', 'protocol', 'policy', 'executive', 'systems', 'crisis', 'drift'
}


@dataclass
class RunArtifacts:
    name: str
    workdir: Path
    receipts_path: Path
    state_path: Path
    log_path: Path
    receipts: List[dict]
    state: dict


def log(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def run_cmd(cmd: List[str], cwd: Path, log_file: Path) -> int:
    log_file.parent.mkdir(parents=True, exist_ok=True)
    with log_file.open('a', encoding='utf-8') as f:
        f.write(f"\n$ {' '.join(cmd)}\n")
        f.flush()
        p = subprocess.run(cmd, cwd=str(cwd), stdout=f, stderr=subprocess.STDOUT, text=True)
    return p.returncode


def read_prompts(path: Path) -> List[str]:
    prompts = []
    for line in path.read_text(encoding='utf-8').splitlines():
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        prompts.append(line)
    return prompts


def read_jsonl(path: Path) -> List[dict]:
    if not path.exists():
        return []
    out = []
    for line in path.read_text(encoding='utf-8').splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return out


def read_json(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding='utf-8'))
    except json.JSONDecodeError:
        return {}


def safe_mean(vals: List[float]) -> float:
    return float(sum(vals) / len(vals)) if vals else 0.0


def nstar_failure(r: dict) -> bool:
    gate = r.get('gate_summary', 'CLEAR: proceed')
    q = float(r.get('quality', 0.0))
    return gate != 'CLEAR: proceed' or q < FAIL_QUALITY_THRESHOLD


def canonical_failure(r: dict) -> bool:
    decision = r.get('decision', 'Commit')
    inv = bool(r.get('invariant_passed', True))
    q = float(r.get('proposal_quality', 1.0))
    contradiction = float(r.get('contradiction_score', 0.0))
    return decision != 'Commit' or (not inv) or q < FAIL_QUALITY_THRESHOLD or contradiction > 0.1


def nonclear_gate(r: dict) -> bool:
    return r.get('gate_summary', 'CLEAR: proceed') != 'CLEAR: proceed'


def reduction(first_rate: float, last_rate: float) -> float:
    if first_rate <= 0:
        return 0.0
    return (first_rate - last_rate) / first_rate


def failure_rate(receipts: List[dict], fn: Callable[[dict], bool]) -> float:
    if not receipts:
        return 0.0
    fails = sum(1 for r in receipts if fn(r))
    return fails / len(receipts)


def split_half_rates(receipts: List[dict], fn: Callable[[dict], bool]) -> Tuple[float, float]:
    mid = len(receipts) // 2
    first = receipts[:mid]
    second = receipts[mid:]
    f = failure_rate(first, fn) if first else 0.0
    s = failure_rate(second, fn) if second else 0.0
    return f, s


def gate_warning_precision_next_turn(receipts: List[dict], failure_fn: Callable[[dict], bool]) -> float:
    warnings = 0
    hits = 0
    for i in range(len(receipts) - 1):
        if nonclear_gate(receipts[i]):
            warnings += 1
            if failure_fn(receipts[i + 1]):
                hits += 1
    return hits / warnings if warnings > 0 else 0.0


def first_convergence_turn(receipts: List[dict], failure_fn: Callable[[dict], bool], streak: int = 3) -> int:
    for i in range(streak - 1, len(receipts)):
        window = receipts[i - streak + 1 : i + 1]
        if all((not failure_fn(r)) and (not nonclear_gate(r)) for r in window):
            return i + 1
    return 999


def token_intensity(receipt: dict) -> float:
    coords = receipt.get('coordinates', [])
    for c in coords:
        if c.get('scale') == 'Token':
            return float(c.get('intensity', 0.0))
    return 0.0


def turn_intensity(receipt: dict) -> float:
    coords = receipt.get('coordinates', [])
    for c in coords:
        if c.get('scale') == 'Turn':
            return float(c.get('intensity', 0.0))
    return 0.0


def run_nstar_series(
    name: str,
    prompts: List[str],
    out_dir: Path,
    addendum_prefix: str = '',
    seed_state: Optional[dict] = None,
    disable_discovery: bool = False,
) -> RunArtifacts:
    workdir = out_dir / 'work' / name
    workdir.mkdir(parents=True, exist_ok=True)

    log_path = out_dir / 'logs' / f'{name}.log'
    receipts_path = workdir / 'receipts.jsonl'
    state_path = workdir / 'nstar_state.json'

    run_cmd([str(NSTAR_BIN), '--reset'], workdir, log_path)

    if seed_state is not None:
        state_path.write_text(json.dumps(seed_state, indent=2), encoding='utf-8')

    for idx, p in enumerate(prompts, start=1):
        prompt = f"{addendum_prefix} {p}".strip() if addendum_prefix else p
        rc = run_cmd([str(NSTAR_BIN), prompt], workdir, log_path)
        if rc != 0:
            log(f"{name}: turn {idx} failed with rc={rc}")
        if disable_discovery and state_path.exists():
            st = read_json(state_path)
            preds = st.get('predicates', [])
            if len(preds) > 9:
                st['predicates'] = preds[:9]
                state_path.write_text(json.dumps(st, indent=2), encoding='utf-8')

    return RunArtifacts(
        name=name,
        workdir=workdir,
        receipts_path=receipts_path,
        state_path=state_path,
        log_path=log_path,
        receipts=read_jsonl(receipts_path),
        state=read_json(state_path),
    )


def run_canonical_series(
    name: str,
    prompts: List[str],
    out_dir: Path,
    audit_rate: float,
    max_risk: float = 0.8,
    addendum_prefix: str = '',
) -> RunArtifacts:
    workdir = out_dir / 'work' / name
    workdir.mkdir(parents=True, exist_ok=True)

    log_path = out_dir / 'logs' / f'{name}.log'
    state_path = workdir / 'canonical_state.json'
    receipts_path = workdir / 'canonical_receipts.jsonl'

    run_cmd(
        [
            str(CANON_BIN),
            '--state-file', str(state_path),
            '--receipts-file', str(receipts_path),
            '--max-risk', str(max_risk),
            '--audit-rate', str(audit_rate),
            '--reset',
        ],
        workdir,
        log_path,
    )

    for idx, p in enumerate(prompts, start=1):
        prompt = f"{addendum_prefix} {p}".strip() if addendum_prefix else p
        rc = run_cmd(
            [
                str(CANON_BIN),
                '--state-file', str(state_path),
                '--receipts-file', str(receipts_path),
                '--max-risk', str(max_risk),
                '--audit-rate', str(audit_rate),
                prompt,
            ],
            workdir,
            log_path,
        )
        if rc != 0:
            log(f"{name}: turn {idx} failed with rc={rc}")

    return RunArtifacts(
        name=name,
        workdir=workdir,
        receipts_path=receipts_path,
        state_path=state_path,
        log_path=log_path,
        receipts=read_jsonl(receipts_path),
        state=read_json(state_path),
    )


def fixed9_seed_state() -> dict:
    names = [
        ('Alignment', 'Intent and output alignment must hold', 'Halt', 0.8),
        ('Uncertainty', 'Low confidence or missing context requires verification', 'Verify', 0.7),
        ('Permission', 'Potentially sensitive action requires explicit permission', 'Escalate', 0.8),
        ('Error', 'Execution failure or contradiction detected', 'Halt', 0.7),
        ('Drift', 'Response drifts from task objective', 'Verify', 0.7),
        ('Interrupt', 'External constraints changed mid-task', 'Escalate', 0.7),
        ('Recovery', 'System is in error recovery flow', 'Simulate', 0.7),
        ('Trust', 'Evidence sufficiency for trust decision', 'Verify', 0.7),
        ('Meta', 'Meta-level policy adaptation pressure', 'Simulate', 0.8),
    ]
    primes = [2, 3, 5, 7, 11, 13, 17, 19, 23]
    preds = []
    for i, (name, cond, gate, threshold) in enumerate(names):
        preds.append(
            {
                'id': str(uuid.uuid4()),
                'prime_id': primes[i],
                'name': name,
                'discovered_at': 0,
                'activation_condition': cond,
                'gate': gate,
                'threshold': threshold,
                'activation': 0.0,
                'reinforcements': 0,
                'merged_from': [],
            }
        )
    return {
        'predicates': preds,
        'collapses': [],
        'total_turns': 0,
        'session': str(uuid.uuid4()),
        'max_history': 100,
    }


def ensure_binaries(out_dir: Path) -> None:
    log_path = out_dir / 'logs' / 'build.log'
    rc = run_cmd(['cargo', 'build', '--bin', 'nstar-bit', '--bin', 'canonical'], ROOT, log_path)
    if rc != 0:
        raise RuntimeError('build failed; see logs/build.log')


def keyword_hit(items: List[str], keywords: set) -> bool:
    text = ' '.join(items).lower()
    return any(k in text for k in keywords)


def write_json(path: Path, obj: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, indent=2), encoding='utf-8')


def main() -> int:
    run_id = time.strftime('%Y%m%d-%H%M%S')
    out_dir = OUT_BASE / run_id
    (out_dir / 'logs').mkdir(parents=True, exist_ok=True)
    (out_dir / 'metrics').mkdir(parents=True, exist_ok=True)
    (out_dir / 'work').mkdir(parents=True, exist_ok=True)

    log(f'run_id={run_id}')

    prompts_single = read_prompts(PROMPTS_DIR / 'strict_single_task_20.txt')[:20]
    prompts_cross = read_prompts(PROMPTS_DIR / 'strict_cross_domain_20.txt')[:20]

    ensure_binaries(out_dir)

    # 1) Minimal Collapse Test
    log('exp1: minimal collapse test')
    exp1 = run_nstar_series('exp1_minimal_collapse', prompts_single, out_dir)
    exp1_first, exp1_last = split_half_rates(exp1.receipts, nstar_failure)
    exp1_reduction = reduction(exp1_first, exp1_last)
    exp1_dims = len(exp1.state.get('predicates', []))
    exp1_pass = (exp1_dims >= 2) and (exp1_reduction >= 0.30)
    exp1_metrics = {
        'setup': 'one prompt addendum + one nstar_state.json, 20 real turns on one task',
        'turns': len(exp1.receipts),
        'dimensions_end': exp1_dims,
        'failure_rate_first10': exp1_first,
        'failure_rate_last10': exp1_last,
        'failure_reduction': exp1_reduction,
        'pass_rule': 'dimensions_end >= 2 and failure_reduction >= 0.30',
        'passed': exp1_pass,
    }
    write_json(out_dir / 'metrics' / 'exp1_minimal_collapse.metrics.json', exp1_metrics)

    # 2) Dynamic-n vs Fixed-9 Ablation
    log('exp2: dynamic-n vs fixed-9 ablation')
    exp2_fixed = run_nstar_series(
        'exp2_fixed9',
        prompts_single,
        out_dir,
        seed_state=fixed9_seed_state(),
        disable_discovery=True,
    )
    dyn_fail = failure_rate(exp1.receipts, nstar_failure)
    fix_fail = failure_rate(exp2_fixed.receipts, nstar_failure)
    dyn_interventions = sum(1 for r in exp1.receipts if nonclear_gate(r))
    fix_interventions = sum(1 for r in exp2_fixed.receipts if nonclear_gate(r))
    exp2_pass = dyn_fail < fix_fail and dyn_interventions < fix_interventions
    exp2_metrics = {
        'setup': 'same task stream; dynamic discovery vs fixed seeded 9 predicates',
        'dynamic_failure_rate': dyn_fail,
        'fixed9_failure_rate': fix_fail,
        'dynamic_manual_interventions_proxy': dyn_interventions,
        'fixed9_manual_interventions_proxy': fix_interventions,
        'pass_rule': 'dynamic_failure_rate < fixed9_failure_rate and dynamic_interventions < fixed9_interventions',
        'passed': exp2_pass,
    }
    write_json(out_dir / 'metrics' / 'exp2_dynamic_vs_fixed9.metrics.json', exp2_metrics)

    # 3) Node<->State Interchangeability Test
    log('exp3: node-state interchangeability (graph-only control)')
    exp3 = run_canonical_series('exp3_graph_only', prompts_single, out_dir, audit_rate=0.33)
    nstar_gate_quality = gate_warning_precision_next_turn(exp1.receipts, nstar_failure)
    canon_gate_quality = gate_warning_precision_next_turn(exp3.receipts, canonical_failure)
    exp3_pass = canon_gate_quality >= nstar_gate_quality
    exp3_metrics = {
        'setup': 'graph activation only control (canonical core) vs nstar predicate-vector baseline',
        'nstar_gate_warning_precision_next_turn': nstar_gate_quality,
        'canonical_gate_warning_precision_next_turn': canon_gate_quality,
        'pass_rule': 'canonical_precision >= nstar_precision',
        'passed': exp3_pass,
    }
    write_json(out_dir / 'metrics' / 'exp3_node_state_interchangeability.metrics.json', exp3_metrics)

    # 4) Multi-Scale Recursion Test
    log('exp4: multi-scale recursion')
    token_vals = [token_intensity(r) for r in exp3.receipts]
    token_median = statistics.median(token_vals) if token_vals else 0.0
    alarms = []
    leads = []
    for i, r in enumerate(exp3.receipts):
        alarm = token_intensity(r) > token_median
        if not alarm:
            continue
        alarms.append(i)
        lead = None
        for j in range(i + 1, min(i + 4, len(exp3.receipts))):
            if canonical_failure(exp3.receipts[j]):
                lead = j - i
                break
        if lead is not None:
            leads.append(lead)
    precision = (len(leads) / len(alarms)) if alarms else 0.0
    mean_lead = safe_mean([float(x) for x in leads])
    exp4_pass = precision >= 0.5 and mean_lead >= 1.0
    exp4_metrics = {
        'setup': 'token/turn/session/project coordinates active each turn; token alarms predicting higher-level failures',
        'token_alarm_median_threshold': token_median,
        'alarm_count': len(alarms),
        'hits_within_3_turns': len(leads),
        'precision': precision,
        'mean_lead_turns': mean_lead,
        'pass_rule': 'precision >= 0.5 and mean_lead_turns >= 1.0',
        'passed': exp4_pass,
    }
    write_json(out_dir / 'metrics' / 'exp4_multiscale_recursion.metrics.json', exp4_metrics)

    # 5) Goodhart Resistance Test
    log('exp5: goodhart resistance (always-on vs stochastic audits)')
    exp5_always = run_canonical_series('exp5_always_on', prompts_single, out_dir, audit_rate=1.0)
    stoch = exp3.receipts
    always = exp5_always.receipts

    stoch_contra = safe_mean([float(r.get('contradiction_score', 0.0)) for r in stoch])
    always_contra = safe_mean([float(r.get('contradiction_score', 0.0)) for r in always])
    stoch_quality = safe_mean([float(r.get('proposal_quality', 0.0)) for r in stoch])
    always_quality = safe_mean([float(r.get('proposal_quality', 0.0)) for r in always])
    stoch_pass_rate = safe_mean([1.0 if bool(r.get('invariant_passed', False)) else 0.0 for r in stoch])
    always_pass_rate = safe_mean([1.0 if bool(r.get('invariant_passed', False)) else 0.0 for r in always])

    exp5_pass = (
        stoch_contra < always_contra
        and stoch_quality >= always_quality
        and stoch_pass_rate >= always_pass_rate
    )
    exp5_metrics = {
        'setup': 'canonical core with audit_rate=1.0 vs audit_rate=0.33',
        'stochastic_contradiction_mean': stoch_contra,
        'always_on_contradiction_mean': always_contra,
        'stochastic_quality_mean': stoch_quality,
        'always_on_quality_mean': always_quality,
        'stochastic_invariant_pass_rate': stoch_pass_rate,
        'always_on_invariant_pass_rate': always_pass_rate,
        'pass_rule': 'stochastic_contradiction < always_contradiction and stochastic_quality >= always_quality and stochastic_pass_rate >= always_pass_rate',
        'passed': exp5_pass,
    }
    write_json(out_dir / 'metrics' / 'exp5_goodhart_resistance.metrics.json', exp5_metrics)

    # 6) Human Rejection Distillation Test (proxy)
    log('exp6: rejection distillation')
    rejection_prefix = (
        'REJECTION SIGNAL: If information is missing, refuse premature completion, '
        'state uncertainty explicitly, ask/verify before acting.'
    )
    exp6_cued = run_nstar_series('exp6_rejection_cued', prompts_single, out_dir, addendum_prefix=rejection_prefix)

    plain_conv = first_convergence_turn(exp1.receipts, nstar_failure)
    cued_conv = first_convergence_turn(exp6_cued.receipts, nstar_failure)
    plain_redirects = sum(1 for r in exp1.receipts if nonclear_gate(r))
    cued_redirects = sum(1 for r in exp6_cued.receipts if nonclear_gate(r))

    exp6_pass = cued_conv < plain_conv and cued_redirects < plain_redirects
    exp6_metrics = {
        'setup': 'same stream plain vs rejection-signal prefixed stream',
        'plain_convergence_turn_proxy': plain_conv,
        'cued_convergence_turn_proxy': cued_conv,
        'plain_redirect_count_proxy': plain_redirects,
        'cued_redirect_count_proxy': cued_redirects,
        'pass_rule': 'cued_convergence < plain_convergence and cued_redirects < plain_redirects',
        'passed': exp6_pass,
    }
    write_json(out_dir / 'metrics' / 'exp6_rejection_distillation.metrics.json', exp6_metrics)

    # 7) Cross-Domain Adaptation Test
    log('exp7: cross-domain adaptation')
    exp7 = run_canonical_series('exp7_cross_domain', prompts_cross, out_dir, audit_rate=0.33)
    first_half = exp7.receipts[:10]
    second_half = exp7.receipts[10:20]

    discovered_first = [n for r in first_half for n in r.get('discovered_nodes', [])]
    discovered_second = [n for r in second_half for n in r.get('discovered_nodes', [])]

    code_specific = keyword_hit(discovered_first, CODE_KEYWORDS)
    strategy_specific = keyword_hit(discovered_second, STRATEGY_KEYWORDS)

    clear_first = safe_mean([1.0 if not nonclear_gate(r) else 0.0 for r in first_half])
    clear_second = safe_mean([1.0 if not nonclear_gate(r) else 0.0 for r in second_half])
    clear_diff = abs(clear_first - clear_second)

    exp7_pass = code_specific and strategy_specific and clear_diff <= 0.20
    exp7_metrics = {
        'setup': 'single protocol, first 10 code turns then 10 strategy turns',
        'discovered_first_half': discovered_first,
        'discovered_second_half': discovered_second,
        'code_specific_dimensions_detected': code_specific,
        'strategy_specific_dimensions_detected': strategy_specific,
        'clear_rate_first_half': clear_first,
        'clear_rate_second_half': clear_second,
        'clear_rate_diff': clear_diff,
        'pass_rule': 'code_specific and strategy_specific and clear_rate_diff <= 0.20',
        'passed': exp7_pass,
    }
    write_json(out_dir / 'metrics' / 'exp7_cross_domain_adaptation.metrics.json', exp7_metrics)

    # 8) Operator Legibility Test (proxy)
    log('exp8: operator legibility proxy')
    compact_rows = []
    for i, r in enumerate(exp7.receipts):
        compact_rows.append(
            {
                'turn': r.get('turn', i + 1),
                'ins_prompt': prompts_cross[i] if i < len(prompts_cross) else '',
                'outs_quality': float(r.get('proposal_quality', 0.0)),
                'gate': r.get('gate_summary', ''),
                'token_intensity': token_intensity(r),
                'turn_intensity': turn_intensity(r),
                'decision': r.get('decision', ''),
                'active_relations_proxy': len(r.get('coordinates', [])),
            }
        )

    compact_path = out_dir / 'metrics' / 'exp8_compact_view.json'
    write_json(compact_path, {'rows': compact_rows})

    token_med = statistics.median([row['token_intensity'] for row in compact_rows]) if compact_rows else 0.0

    def target_fail(idx: int) -> bool:
        if idx + 1 >= len(exp7.receipts):
            return False
        return canonical_failure(exp7.receipts[idx + 1])

    compact_pred_ok = 0
    logs_pred_ok = 0
    total = 0
    for i in range(len(exp7.receipts) - 1):
        r = exp7.receipts[i]
        compact_pred = (
            token_intensity(r) > token_med
            or turn_intensity(r) > 0.45
            or nonclear_gate(r)
        )
        logs_pred = bool(r.get('violations'))
        actual = target_fail(i)
        compact_pred_ok += 1 if compact_pred == actual else 0
        logs_pred_ok += 1 if logs_pred == actual else 0
        total += 1

    compact_acc = (compact_pred_ok / total) if total else 0.0
    logs_acc = (logs_pred_ok / total) if total else 0.0
    exp8_pass = compact_acc > logs_acc
    exp8_metrics = {
        'setup': 'compact ins/outs/coordinates view vs log-violation baseline predictor',
        'note': 'proxy for human operator; no human-in-the-loop scoring in this automated run',
        'cases': total,
        'compact_view_accuracy_proxy': compact_acc,
        'logs_baseline_accuracy_proxy': logs_acc,
        'pass_rule': 'compact_view_accuracy_proxy > logs_baseline_accuracy_proxy',
        'passed': exp8_pass,
    }
    write_json(out_dir / 'metrics' / 'exp8_operator_legibility.metrics.json', exp8_metrics)

    suite = {
        'run_id': run_id,
        'timestamp': time.strftime('%Y-%m-%dT%H:%M:%S'),
        'order': [
            'exp1_minimal_collapse',
            'exp2_dynamic_vs_fixed9',
            'exp3_node_state_interchangeability',
            'exp4_multiscale_recursion',
            'exp5_goodhart_resistance',
            'exp6_rejection_distillation',
            'exp7_cross_domain_adaptation',
            'exp8_operator_legibility',
        ],
        'results': {
            'exp1_minimal_collapse': exp1_metrics,
            'exp2_dynamic_vs_fixed9': exp2_metrics,
            'exp3_node_state_interchangeability': exp3_metrics,
            'exp4_multiscale_recursion': exp4_metrics,
            'exp5_goodhart_resistance': exp5_metrics,
            'exp6_rejection_distillation': exp6_metrics,
            'exp7_cross_domain_adaptation': exp7_metrics,
            'exp8_operator_legibility': exp8_metrics,
        },
        'passed_count': sum(
            1
            for m in [
                exp1_metrics,
                exp2_metrics,
                exp3_metrics,
                exp4_metrics,
                exp5_metrics,
                exp6_metrics,
                exp7_metrics,
                exp8_metrics,
            ]
            if m.get('passed')
        ),
        'total': 8,
    }

    write_json(out_dir / 'suite_report.json', suite)

    summary_lines = [
        f"run_id: {run_id}",
        f"out_dir: {out_dir}",
        f"pass_count: {suite['passed_count']}/{suite['total']}",
    ]
    for k in suite['order']:
        summary_lines.append(f"- {k}: {'PASS' if suite['results'][k]['passed'] else 'FAIL'}")

    (out_dir / 'SUMMARY.txt').write_text('\n'.join(summary_lines) + '\n', encoding='utf-8')

    print('\n'.join(summary_lines))
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as e:
        print(f"fatal: {e}", file=sys.stderr)
        sys.exit(1)
