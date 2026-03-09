use crate::receipt::Effect;
use crate::utir::Operation;

use super::types::{
    CanonicalCriteria, CanonicalInput, CanonicalProposal, GateDecision, InvariantReport,
    SimulationReport,
};

#[derive(Debug, Clone, Copy)]
enum RequiredEvidence {
    Read,
    Write,
    Verification,
    Effects,
}

pub fn evaluate_invariants(
    input: &CanonicalInput,
    proposal: &CanonicalProposal,
    gate: &GateDecision,
    simulation: &SimulationReport,
    effects: &[Effect],
    criteria: &CanonicalCriteria,
    audit_triggered: bool,
) -> InvariantReport {
    let mut violations = Vec::new();

    if !simulation.can_materialize {
        violations.push("simulation_blocked_materialization".to_string());
    }

    if criteria.require_read_before_write && has_write_before_read(&proposal.operations) {
        violations.push("write_before_read_operation_order".to_string());
    }

    if has_failed_effect(effects) {
        violations.push("execution_effect_failure".to_string());
    }

    if gate.has_signal("escalate") && !proposal.response.to_lowercase().contains("escalat") {
        violations.push("missing_escalation_behavior".to_string());
    }

    let has_ovm_write = !proposal.ovm_ops.is_empty();
    let required = required_evidence(gate, proposal);
    let satisfied = required
        .iter()
        .filter(|r| evidence_satisfied(**r, effects, &proposal.response, has_ovm_write))
        .count();

    let evidence_coverage = if required.is_empty() {
        1.0
    } else {
        satisfied as f32 / required.len() as f32
    };

    if evidence_coverage < criteria.min_evidence_coverage {
        violations.push(format!(
            "insufficient_evidence_coverage:{:.2}",
            evidence_coverage
        ));
    }

    let contradiction_score = contradiction_score(input, proposal, effects, audit_triggered, gate);
    if contradiction_score > criteria.contradiction_threshold {
        violations.push(format!(
            "contradiction_score_exceeded:{:.2}",
            contradiction_score
        ));
    }

    if has_ovm_write && !effects.iter().any(|e| matches!(e, Effect::ReadFile { ok: true, .. })) {
        violations.push("no_evidence_at_ovm_op: scored without reading graph context".into());
    }

    InvariantReport {
        passed: violations.is_empty(),
        violations,
        evidence_coverage,
        contradiction_score,
    }
}

fn required_evidence(gate: &GateDecision, proposal: &CanonicalProposal) -> Vec<RequiredEvidence> {
    let mut required = Vec::new();

    if !proposal.operations.is_empty() {
        required.push(RequiredEvidence::Effects);
    }
    if gate.has_signal("require_evidence:fs.read") {
        required.push(RequiredEvidence::Read);
    }
    if gate.has_signal("require_evidence:fs.write") {
        required.push(RequiredEvidence::Write);
    }
    if gate.has_signal("verify") {
        required.push(RequiredEvidence::Verification);
    }

    required
}

fn evidence_satisfied(req: RequiredEvidence, effects: &[Effect], response: &str, has_ovm_write: bool) -> bool {
    match req {
        // A read must be a real ReadFile effect. OVM ops do NOT satisfy this —
        // that conflation is exactly what no_evidence_at_ovm_op is here to prevent.
        RequiredEvidence::Read => {
            effects.iter().any(|e| matches!(e, Effect::ReadFile { ok: true, .. }))
        }
        // OVM ops (DefineScoringRule, DefineSelectionPredicate) are the canonical write
        // action in scoring-rule mode — they produce no filesystem side-effects but are real writes.
        RequiredEvidence::Write => {
            effects.iter().any(|e| matches!(e, Effect::WriteFile { ok: true, .. }))
                || has_ovm_write
        }
        // OVM rule proposals constitute verification of graph state consistency.
        RequiredEvidence::Verification => {
            effects.iter().any(|e| {
                matches!(
                    e,
                    Effect::Assert { ok: true, .. }
                        | Effect::ReadFile { ok: true, .. }
                        | Effect::Exec { ok: true, .. }
                )
            }) || response.to_lowercase().contains("verified")
                || has_ovm_write
        }
        RequiredEvidence::Effects => !effects.is_empty() || has_ovm_write,
    }
}

fn has_failed_effect(effects: &[Effect]) -> bool {
    effects.iter().any(|e| match e {
        Effect::WriteFile { ok, .. } => !ok,
        Effect::ReadFile { ok, .. } => !ok,
        Effect::HttpGet { ok, .. } => !ok,
        Effect::GitPatch { ok, .. } => !ok,
        Effect::Assert { ok, .. } => !ok,
        Effect::Blocked { .. } => true,
        Effect::Exec { ok, .. } => !ok,
    })
}

fn has_write_before_read(ops: &[Operation]) -> bool {
    let mut seen_read = false;
    let mut write_before_read = false;
    flatten_ops(ops, &mut |op| match op {
        Operation::FsRead { .. } => seen_read = true,
        Operation::FsWrite { .. } => {
            if !seen_read {
                write_before_read = true;
            }
        }
        _ => {}
    });
    write_before_read
}

fn contradiction_score(
    _input: &CanonicalInput,
    proposal: &CanonicalProposal,
    effects: &[Effect],
    audit_triggered: bool,
    gate: &GateDecision,
) -> f32 {
    let mut score: f32 = 0.0;

    let has_write_effect = effects
        .iter()
        .any(|e| matches!(e, Effect::WriteFile { ok: true, .. }));
    let has_read_effect = effects
        .iter()
        .any(|e| matches!(e, Effect::ReadFile { ok: true, .. }));
    // OVM ops count as writes for the assert:wrote gate — they are the canonical
    // write action when the LM proposes scoring rules or selection predicates.
    let has_ovm_write = !proposal.ovm_ops.is_empty();

    if gate.has_signal("assert:wrote") && !has_write_effect && !has_ovm_write {
        score += 0.7;
    }

    if gate.has_signal("assert:read") && !has_read_effect {
        score += 0.4;
    }

    if gate.has_signal("assert:cannot") && !proposal.operations.is_empty() {
        score += 0.2;
    }

    if audit_triggered && gate.has_signal("assert:definitely") && proposal.errors.is_empty() {
        score += 0.2;
    }

    score.clamp(0.0, 1.0)
}

fn flatten_ops<F: FnMut(&Operation)>(ops: &[Operation], f: &mut F) {
    for op in ops {
        f(op);
        match op {
            Operation::Attempt { operation } => {
                flatten_ops(std::slice::from_ref(operation.as_ref()), f)
            }
            Operation::Sequence { steps } => flatten_ops(steps, f),
            Operation::Parallel { steps, .. } => flatten_ops(steps, f),
            Operation::Conditional {
                condition,
                then_op,
                else_op,
            } => {
                flatten_ops(std::slice::from_ref(condition.as_ref()), f);
                flatten_ops(std::slice::from_ref(then_op.as_ref()), f);
                if let Some(else_op) = else_op {
                    flatten_ops(std::slice::from_ref(else_op.as_ref()), f);
                }
            }
            Operation::Retry { operation, .. } => {
                flatten_ops(std::slice::from_ref(operation.as_ref()), f)
            }
            _ => {}
        }
    }
}
