use chrono::Utc;
use evalexpr::{ContextWithMutableFunctions, ContextWithMutableVariables};

use crate::lm::OvmOp;

use super::graph::active_nodes;
use super::schema::{
    BenchmarkMetric, BenchmarkReport, BenchmarkSuiteResult, CapabilityLedgerView, ChampionPolicy,
    DerivedWarRoomView, GraphProjectionView, LiveControlState, MetricComparator,
    PromotionAction, PromotionDecisionRecord, RepoAbsorptionMap, RepoContribution,
    RequirementItem, RequirementsLockView, RuntimeExecutionRecord, RuntimePolicyCandidate,
    RuntimeSnapshot,
};
use super::types::{CanonicalState, EdgeKind, GraphState, TurnTrace};

pub struct DerivedArtifacts {
    pub war_room: DerivedWarRoomView,
    pub capability_ledger: CapabilityLedgerView,
    pub requirements_lock: RequirementsLockView,
    pub graph_projection: GraphProjectionView,
    pub repo_absorption: RepoAbsorptionMap,
}

pub fn extract_policy_candidate(
    graph: &GraphState,
    ops: &[OvmOp],
    turn: u64,
) -> Option<RuntimePolicyCandidate> {
    let mut candidate = RuntimePolicyCandidate {
        candidate_id: format!("candidate-turn-{}", turn),
        scoring_rule: graph.scoring_rule.clone(),
        selection_predicate: graph.selection_predicate.clone(),
        source_turn: turn,
    };
    let mut changed = false;

    for op in ops {
        match op {
            OvmOp::DefineScoringRule { rule } => {
                candidate.scoring_rule = rule.trim_start_matches("maximize ").trim().to_string();
                changed = true;
            }
            OvmOp::DefineSelectionPredicate { predicate } => {
                candidate.selection_predicate = predicate.trim().to_string();
                changed = true;
            }
        }
    }

    changed.then_some(candidate)
}

pub fn validate_policy_candidate(candidate: &RuntimePolicyCandidate) -> Vec<String> {
    let mut violations = Vec::new();
    let mut context = evalexpr::HashMapContext::new();
    let _ = context.set_value("c11".into(), 3.0.into());
    let _ = context.set_value("c10".into(), 1.0.into());
    let _ = context.set_value("c01".into(), 1.0.into());
    let _ = context.set_value("c00".into(), 0.0.into());
    let _ = context.set_value("t".into(), 5.0.into());
    let _ = context.set_value("score".into(), 0.5.into());
    let _ = context.set_function(
        "log".into(),
        evalexpr::Function::new(|v| Ok(v.as_float()?.ln().into())),
    );
    let _ = context.set_function(
        "sqrt".into(),
        evalexpr::Function::new(|v| Ok(v.as_float()?.sqrt().into())),
    );
    let _ = context.set_function(
        "abs".into(),
        evalexpr::Function::new(|v| Ok(v.as_float()?.abs().into())),
    );
    let _ = context.set_function(
        "max".into(),
        evalexpr::Function::new(|v| {
            let t = v.as_tuple()?;
            Ok(t[0].as_float()?.max(t[1].as_float()?).into())
        }),
    );
    let _ = context.set_function(
        "min".into(),
        evalexpr::Function::new(|v| {
            let t = v.as_tuple()?;
            Ok(t[0].as_float()?.min(t[1].as_float()?).into())
        }),
    );

    if !candidate.scoring_rule.is_empty()
        && evalexpr::eval_float_with_context(&candidate.scoring_rule, &context).is_err()
    {
        violations.push("ovm:scoring_eval_failed:candidate".to_string());
    }

    if !candidate.selection_predicate.is_empty()
        && evalexpr::eval_boolean_with_context(&candidate.selection_predicate, &context).is_err()
    {
        violations.push("ovm:predicate_eval_failed:candidate".to_string());
    }

    violations
}

pub fn build_runtime_execution_record(
    trace: &TurnTrace,
    operation_count: usize,
) -> RuntimeExecutionRecord {
    RuntimeExecutionRecord {
        executor_id: "guarded_utir_v1".to_string(),
        task_id: format!("nstar-canonical-turn-{}", trace.input.turn),
        operation_count,
        effect_count: trace.execution_effects.len(),
        blocked: !trace.simulation.can_materialize,
    }
}

pub fn build_benchmark_report(
    state: &CanonicalState,
    trace: &TurnTrace,
    runtime: &RuntimeExecutionRecord,
) -> BenchmarkReport {
    let deterministic_ratio = deterministic_ratio(state);
    let scorecard = state.graph.rule_scorecard.clone();
    let heldout_precision = scorecard
        .as_ref()
        .map(|s| s.precision_at_k)
        .unwrap_or(0.0);
    let heldout_recall = scorecard.as_ref().map(|s| s.recall_at_k).unwrap_or(0.0);
    let has_runtime_effects = !trace.execution_effects.is_empty() || runtime.operation_count == 0;

    let safety_metrics = vec![
        metric_bool(
            "materialization_safe",
            "Materialization safe",
            trace.simulation.can_materialize,
        ),
        metric_bool(
            "effects_closed_cleanly",
            "Effects closed cleanly",
            trace.execution_effects.iter().all(effect_ok),
        ),
        metric_bool(
            "runtime_path_guarded",
            "Runtime path guarded",
            runtime.executor_id == "guarded_utir_v1",
        ),
    ];

    let evidence_metrics = vec![
        metric_threshold(
            "evidence_coverage",
            "Evidence coverage",
            trace.invariants.evidence_coverage,
            state.graph.criteria.min_evidence_coverage,
            MetricComparator::AtLeast,
            "ratio",
        ),
        metric_threshold(
            "contradiction_score",
            "Contradiction score",
            trace.invariants.contradiction_score,
            state.graph.criteria.contradiction_threshold,
            MetricComparator::AtMost,
            "score",
        ),
        metric_bool(
            "runtime_effects_present",
            "Runtime effects present when needed",
            has_runtime_effects,
        ),
    ];

    let promotion_metrics = vec![
        metric_threshold(
            "deterministic_ratio",
            "Replay-verified receipt ratio",
            deterministic_ratio,
            0.0,
            MetricComparator::AtLeast,
            "ratio",
        ),
        metric_threshold(
            "heldout_precision",
            "Held-out precision",
            heldout_precision,
            0.0,
            MetricComparator::AtLeast,
            "ratio",
        ),
        metric_threshold(
            "heldout_recall",
            "Held-out recall",
            heldout_recall,
            0.0,
            MetricComparator::AtLeast,
            "ratio",
        ),
    ];

    let suites = vec![
        suite("runtime_safety", "Runtime safety", safety_metrics),
        suite("evidence", "Evidence quality", evidence_metrics),
        suite("promotion_readiness", "Promotion readiness", promotion_metrics),
    ];

    let total_metrics = suites.iter().map(|s| s.metrics.len()).sum::<usize>().max(1);
    let passed_metrics = suites
        .iter()
        .flat_map(|s| &s.metrics)
        .filter(|m| m.passed)
        .count();
    let macro_score = (passed_metrics as f32 / total_metrics as f32) * 100.0;
    let failing_action_gates = suites
        .iter()
        .flat_map(|suite| {
            suite
                .metrics
                .iter()
                .filter(|metric| !metric.passed)
                .map(|metric| format!("{}:{}", suite.id, metric.id))
        })
        .collect();

    BenchmarkReport {
        generated_at: Utc::now().to_rfc3339(),
        turn: trace.input.turn,
        macro_score,
        suites,
        failing_action_gates,
    }
}

pub fn evaluate_promotion(
    state: &CanonicalState,
    graph: &GraphState,
    trace: &TurnTrace,
    report: &BenchmarkReport,
    candidate: Option<RuntimePolicyCandidate>,
) -> PromotionDecisionRecord {
    let champion_before = state.live_control.active_champion.clone();

    if let Some(candidate) = candidate {
        if !trace.invariants.passed {
            return PromotionDecisionRecord {
                action: PromotionAction::Reject,
                reason: "turn_invariants_failed".to_string(),
                benchmark_macro_score: report.macro_score,
                failing_action_gates: report.failing_action_gates.clone(),
                candidate: Some(candidate),
                champion_before: champion_before.clone(),
                champion_after: champion_before.clone(),
            };
        }

        if !report.failing_action_gates.is_empty() {
            return PromotionDecisionRecord {
                action: PromotionAction::Hold,
                reason: "hard_gates_open".to_string(),
                benchmark_macro_score: report.macro_score,
                failing_action_gates: report.failing_action_gates.clone(),
                candidate: Some(candidate),
                champion_before: champion_before.clone(),
                champion_after: champion_before.clone(),
            };
        }

        let prior_score = champion_before
            .as_ref()
            .map(|c| c.macro_score)
            .unwrap_or(0.0);
        if report.macro_score + 0.001 < prior_score {
            return PromotionDecisionRecord {
                action: PromotionAction::Hold,
                reason: "candidate_under_champion_score".to_string(),
                benchmark_macro_score: report.macro_score,
                failing_action_gates: report.failing_action_gates.clone(),
                candidate: Some(candidate),
                champion_before: champion_before.clone(),
                champion_after: champion_before.clone(),
            };
        }

        let champion_after = Some(build_champion_from_candidate(&candidate, graph, trace, report));
        return PromotionDecisionRecord {
            action: PromotionAction::Promote,
            reason: "candidate_cleared_benchmark_kernel".to_string(),
            benchmark_macro_score: report.macro_score,
            failing_action_gates: report.failing_action_gates.clone(),
            candidate: Some(candidate),
            champion_before,
            champion_after,
        };
    }

    let champion_after = if champion_before.is_none()
        && (!graph.scoring_rule.is_empty() || !graph.selection_predicate.is_empty())
        && report.failing_action_gates.is_empty()
        && trace.invariants.passed
    {
        Some(build_champion_from_graph(graph, trace, report))
    } else {
        champion_before.clone()
    };

    let action = if champion_before.is_none() && champion_after.is_some() {
        PromotionAction::Promote
    } else {
        PromotionAction::Hold
    };
    let reason = if matches!(action, PromotionAction::Promote) {
        "bootstrap_active_policy".to_string()
    } else {
        "no_candidate".to_string()
    };

    PromotionDecisionRecord {
        action,
        reason,
        benchmark_macro_score: report.macro_score,
        failing_action_gates: report.failing_action_gates.clone(),
        candidate: None,
        champion_before,
        champion_after,
    }
}

pub fn apply_governance_update(
    live: &mut LiveControlState,
    decision: PromotionDecisionRecord,
    report: BenchmarkReport,
    trace: &TurnTrace,
    deterministic_ratio: f32,
) {
    live.failing_action_gates = report.failing_action_gates.clone();
    live.latest_benchmark = Some(report.clone());
    live.runtime_history.push(RuntimeSnapshot {
        turn: trace.input.turn,
        decision: format!("{:?}", trace.decision),
        gate_summary: trace.gate.summary(),
        evidence_coverage: trace.invariants.evidence_coverage,
        contradiction_score: trace.invariants.contradiction_score,
        deterministic_ratio,
    });
    if live.runtime_history.len() > 50 {
        let overflow = live.runtime_history.len() - 50;
        live.runtime_history.drain(0..overflow);
    }

    if let Some(champion) = decision.champion_after.clone() {
        live.active_champion = Some(champion);
    }
    live.promotion_history.push(decision);
    if live.promotion_history.len() > 50 {
        let overflow = live.promotion_history.len() - 50;
        live.promotion_history.drain(0..overflow);
    }
}

pub fn build_derived_artifacts(state: &CanonicalState) -> DerivedArtifacts {
    let generated_at = Utc::now().to_rfc3339();
    let active_capabilities = state
        .graph
        .nodes
        .iter()
        .filter(|n| n.reinforcements > 0)
        .map(|n| n.id.clone())
        .collect::<Vec<_>>();
    let evidence_lane = state
        .receipts
        .iter()
        .rev()
        .take(10)
        .map(|r| format!("receipt:{}:{}", r.turn, r.hash))
        .collect::<Vec<_>>();
    let failing_action_gates = state.live_control.failing_action_gates.clone();
    let stalled_critical_suites = failing_action_gates.clone();

    let mut requirements = failing_action_gates
        .iter()
        .map(|gate| RequirementItem {
            id: gate.replace(':', "_"),
            statement: format!("Resolve failing action gate `{}` before promotion.", gate),
            strictness: "hard".to_string(),
        })
        .collect::<Vec<_>>();
    if state.live_control.active_champion.is_none() {
        requirements.push(RequirementItem {
            id: "establish_active_champion".to_string(),
            statement: "Promote at least one benchmark-cleared champion policy.".to_string(),
            strictness: "hard".to_string(),
        });
    }

    let ready_for_implementation =
        requirements.is_empty() && state.live_control.active_champion.is_some();

    let active_nodes = active_nodes(&state.graph, state.graph.criteria.activation_cutoff)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect::<Vec<_>>();
    let hypothesis_edges = state
        .graph
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Hypothesis))
        .count();

    DerivedArtifacts {
        war_room: DerivedWarRoomView {
            generated_at: generated_at.clone(),
            summary: if failing_action_gates.is_empty() {
                "Kernel state is benchmark-cleared; derived views are green.".to_string()
            } else {
                "Kernel state still has failing action gates.".to_string()
            },
            active_champion: state
                .live_control
                .active_champion
                .as_ref()
                .map(|c| c.id.clone()),
            failing_action_gates,
            stalled_critical_suites,
        },
        capability_ledger: CapabilityLedgerView {
            generated_at: generated_at.clone(),
            active_capabilities,
            evidence_lane,
        },
        requirements_lock: RequirementsLockView {
            generated_at: generated_at.clone(),
            ready_for_implementation,
            requirements,
        },
        graph_projection: GraphProjectionView {
            generated_at: generated_at.clone(),
            node_count: state.graph.nodes.len(),
            edge_count: state.graph.edges.len(),
            active_nodes,
            hypothesis_edges,
        },
        repo_absorption: RepoAbsorptionMap {
            generated_at,
            target_repo: "nstar-bit".to_string(),
            contributions: vec![
                contribution(
                    "tmp-meta3-engine-test",
                    "UTIR, guarded execution, receipts, graph adapters",
                    "kernel_runtime",
                ),
                contribution(
                    "macro-hard",
                    "champion/challenger policy scoring and promotion",
                    "evaluation_and_promotion",
                ),
                contribution(
                    "Foundational tools",
                    "hard gate evaluation and loop discipline",
                    "evaluation_and_promotion",
                ),
                contribution(
                    "agentic-os",
                    "receipt manifests, state snapshots, smoke-proof loop",
                    "control_state",
                ),
                contribution(
                    "core-clarity-dashboard",
                    "war-room and capability ledger operator vocabulary",
                    "derived_views",
                ),
            ],
        },
    }
}

fn deterministic_ratio(state: &CanonicalState) -> f32 {
    if state.receipts.is_empty() {
        return 0.0;
    }
    let verified = state.receipts.iter().filter(|r| r.deterministic).count();
    verified as f32 / state.receipts.len() as f32
}

fn effect_ok(effect: &crate::receipt::Effect) -> bool {
    match effect {
        crate::receipt::Effect::WriteFile { ok, .. } => *ok,
        crate::receipt::Effect::ReadFile { ok, .. } => *ok,
        crate::receipt::Effect::HttpGet { ok, .. } => *ok,
        crate::receipt::Effect::GitPatch { ok, .. } => *ok,
        crate::receipt::Effect::Assert { ok, .. } => *ok,
        crate::receipt::Effect::Blocked { .. } => false,
        crate::receipt::Effect::Exec { ok, .. } => *ok,
    }
}

fn metric_bool(id: &str, label: &str, passed: bool) -> BenchmarkMetric {
    BenchmarkMetric {
        id: id.to_string(),
        label: label.to_string(),
        value: if passed { 1.0 } else { 0.0 },
        target: 1.0,
        unit: "bool".to_string(),
        comparator: MetricComparator::AtLeast,
        passed,
    }
}

fn metric_threshold(
    id: &str,
    label: &str,
    value: f32,
    target: f32,
    comparator: MetricComparator,
    unit: &str,
) -> BenchmarkMetric {
    let passed = match comparator {
        MetricComparator::AtLeast => value >= target,
        MetricComparator::AtMost => value <= target,
    };
    BenchmarkMetric {
        id: id.to_string(),
        label: label.to_string(),
        value,
        target,
        unit: unit.to_string(),
        comparator,
        passed,
    }
}

fn suite(id: &str, label: &str, metrics: Vec<BenchmarkMetric>) -> BenchmarkSuiteResult {
    let passed = metrics.iter().all(|metric| metric.passed);
    let score = if metrics.is_empty() {
        0.0
    } else {
        (metrics.iter().filter(|metric| metric.passed).count() as f32 / metrics.len() as f32)
            * 100.0
    };
    BenchmarkSuiteResult {
        id: id.to_string(),
        label: label.to_string(),
        metrics,
        passed,
        score,
    }
}

fn build_champion_from_candidate(
    candidate: &RuntimePolicyCandidate,
    graph: &GraphState,
    trace: &TurnTrace,
    report: &BenchmarkReport,
) -> ChampionPolicy {
    ChampionPolicy {
        id: candidate.candidate_id.clone(),
        scoring_rule: candidate.scoring_rule.clone(),
        selection_predicate: candidate.selection_predicate.clone(),
        promoted_at_turn: trace.input.turn,
        macro_score: report.macro_score,
        evidence_coverage: trace.invariants.evidence_coverage,
        contradiction_score: trace.invariants.contradiction_score,
        heldout_precision: graph
            .rule_scorecard
            .as_ref()
            .map(|s| s.precision_at_k)
            .unwrap_or(0.0),
        heldout_recall: graph
            .rule_scorecard
            .as_ref()
            .map(|s| s.recall_at_k)
            .unwrap_or(0.0),
    }
}

fn build_champion_from_graph(
    graph: &GraphState,
    trace: &TurnTrace,
    report: &BenchmarkReport,
) -> ChampionPolicy {
    ChampionPolicy {
        id: format!("champion-turn-{}", trace.input.turn),
        scoring_rule: graph.scoring_rule.clone(),
        selection_predicate: graph.selection_predicate.clone(),
        promoted_at_turn: trace.input.turn,
        macro_score: report.macro_score,
        evidence_coverage: trace.invariants.evidence_coverage,
        contradiction_score: trace.invariants.contradiction_score,
        heldout_precision: graph
            .rule_scorecard
            .as_ref()
            .map(|s| s.precision_at_k)
            .unwrap_or(0.0),
        heldout_recall: graph
            .rule_scorecard
            .as_ref()
            .map(|s| s.recall_at_k)
            .unwrap_or(0.0),
    }
}

fn contribution(source_repo: &str, contribution: &str, target_subsystem: &str) -> RepoContribution {
    RepoContribution {
        source_repo: source_repo.to_string(),
        contribution: contribution.to_string(),
        target_subsystem: target_subsystem.to_string(),
    }
}
