use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::utir::{Operation, UtirDocument};
use crate::utir_exec::{execute_utir, GuardConfig};

use crate::lm::OvmOp;

use super::graph::{
    active_nodes, apply_discoveries, apply_observations, evaluate_gates, learn_coactivation_edges,
    propagate_activations, reinforce_active_nodes, update_hypothesis_substrate, apply_operator,
    evaluate_rule_heldout,
};
use super::invariants::evaluate_invariants;
use super::types::{
    CanonicalInput, CanonicalProposal, CanonicalReceipt, CanonicalState, CanonicalTurnResult,
    EdgeKind, NodeDiscovery, NodeObservation, Scale, ScaleCoordinate, SimulationReport,
    TurnDecision, TurnCost, TurnTrace,
};

pub struct CanonicalCore {
    pub state: CanonicalState,
    /// When set, DefineScoringRule OVM ops are ignored and this expression is used instead.
    pub frozen_rule: Option<String>,
}

impl CanonicalCore {
    pub fn new() -> Self {
        Self {
            state: CanonicalState::default(),
            frozen_rule: None,
        }
    }

    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            let state: CanonicalState = serde_json::from_str(&data)?;
            Ok(Self { state, frozen_rule: None })
        } else {
            Ok(Self::new())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self.state)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn reset_files(state_path: &Path, receipts_path: &Path) -> Result<()> {
        if state_path.exists() {
            std::fs::remove_file(state_path)?;
        }
        if receipts_path.exists() {
            std::fs::remove_file(receipts_path)?;
        }
        Ok(())
    }

    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str("N* Canonical Core\n");
        out.push_str(&format!("session={}\n", &self.state.session_id[..8]));
        out.push_str(&format!("turns={}\n", self.state.turn_count));
        out.push_str(&format!(
            "nodes={} edges={} patterns={}\n",
            self.state.graph.nodes.len(),
            self.state.graph.edges.len(),
            self.state.graph.patterns.len()
        ));

        let active = active_nodes(
            &self.state.graph,
            self.state.graph.criteria.activation_cutoff,
        );
        if active.is_empty() {
            out.push_str("active_nodes=0\n");
        } else {
            out.push_str("active_nodes:\n");
            for (id, a, p) in active {
                out.push_str(&format!("- {} a={:.2} prime={}\n", id, a, p));
            }
        }

        if let Some(last) = self.state.receipts.last() {
            out.push_str(&format!("last_decision={:?}\n", last.decision));
            out.push_str(&format!("last_gate={}\n", last.gate_summary));
        }

        let promo = self.evaluate_promotion();
        out.push_str(&format!(
            "promotion_eligible={} slope={:.3} failures={}\n",
            promo.passes_promotion, promo.trend_slope, promo.repeated_failures
        ));

        out
    }

    pub fn process_turn(
        &mut self,
        mut input: CanonicalInput,
        proposal: CanonicalProposal,
        observations: Vec<NodeObservation>,
        discoveries: Vec<NodeDiscovery>,
        guard: &GuardConfig,
        receipts_path: &Path,
    ) -> Result<CanonicalTurnResult> {
        input.turn = self.state.turn_count + 1;

        let discovered_nodes = apply_discoveries(&mut self.state.graph, &discoveries, input.turn);
        apply_observations(&mut self.state.graph, &observations, input.turn);
        let criteria_before = self.state.graph.criteria.clone();
        let propagation_steps = self.state.graph.criteria.propagation_steps;
        propagate_activations(&mut self.state.graph, propagation_steps);

        let gate = evaluate_gates(&self.state.graph);
        let simulation = self.simulate_operations(&proposal.operations, &input.prompt);

        let should_execute =
            !gate.has_signal("halt") && !gate.has_signal("escalate") && simulation.can_materialize;
        let execution_effects = if should_execute && !proposal.operations.is_empty() {
            let doc = UtirDocument {
                task_id: format!("nstar-canonical-turn-{}", input.turn),
                description: "canonical-core materialization".to_string(),
                operations: proposal.operations.clone(),
                policy: None,
                bits_tracking: None,
            };
            execute_utir(&doc, guard)
        } else {
            Vec::new()
        };

        let audit_triggered = self.audit_triggered(&input, &proposal);
        let mut invariants = evaluate_invariants(
            &input,
            &proposal,
            &gate,
            &simulation,
            &execution_effects,
            &self.state.graph.criteria,
            audit_triggered,
        );

        // Lane D: Apply Operator (OVM) — runs before make_receipt so violations land in the receipt.
        // Ingest operator definitions from the LM's ovm_ops field.
        for op in &proposal.ovm_ops {
            match op {
                OvmOp::DefineScoringRule { rule } => {
                    if self.frozen_rule.is_none() {
                        let old_rule = self.state.graph.scoring_rule.clone();
                        let new_rule =
                            rule.trim_start_matches("maximize ").trim().to_string();
                        self.state.graph.scoring_rule = new_rule.clone();
                        let trajectory_path =
                            receipts_path.with_file_name("rule_trajectory.jsonl");
                        let _ = append_rule_trajectory(
                            &trajectory_path,
                            input.turn,
                            &old_rule,
                            &new_rule,
                            &proposal,
                            &self.state.graph.edges,
                        );
                    }
                }
                OvmOp::DefineSelectionPredicate { predicate } => {
                    self.state.graph.selection_predicate = predicate.clone();
                }
            }
        }

        // Internal co-occurrence counting (the substrate).
        // Tracks which behavioral node: predicates co-activate — not words.
        update_hypothesis_substrate(
            &mut self.state.graph,
            &observations,
            input.turn,
        );

        let ovm_violations = apply_operator(&mut self.state.graph);
        if !ovm_violations.is_empty() {
            invariants.violations.extend(ovm_violations);
            invariants.passed = invariants.violations.is_empty();
        }

        let decision = if gate.has_signal("halt") {
            TurnDecision::Halt
        } else if gate.has_signal("escalate") {
            TurnDecision::Escalate
        } else if invariants.passed {
            TurnDecision::Commit
        } else {
            TurnDecision::Rollback
        };

        if matches!(decision, TurnDecision::Commit) {
            let cutoff = self.state.graph.criteria.activation_cutoff;
            reinforce_active_nodes(&mut self.state.graph, cutoff);
            learn_coactivation_edges(&mut self.state.graph, cutoff);
        }

        // Every 5 turns, recompute held-out scorecard for the current scoring rule.
        if input.turn % 5 == 0 && !self.state.graph.scoring_rule.is_empty() {
            self.state.graph.rule_scorecard =
                evaluate_rule_heldout(&self.state.graph, &self.state.receipts, 50);
        }

        self.update_project_activation();
        let coordinates = self.compute_coordinates(&input, &proposal);

        let trace = TurnTrace {
            input,
            proposal,
            gate,
            simulation,
            execution_effects,
            invariants,
            coordinates: coordinates.clone(),
            audit_triggered,
            decision: decision.clone(),
            criteria_before,
            criteria_after: self.state.graph.criteria.clone(),
        };

        let receipt = self.make_receipt(&trace, &observations, &discoveries);
        self.append_receipt(receipts_path, &receipt)?;

        self.state.turn_count += 1;
        self.state.last_turn_activation = self
            .state
            .graph
            .nodes
            .iter()
            .map(|n| (n.id.clone(), n.activation))
            .collect();
        self.state.receipts.push(receipt.clone());

        // Lane E: Track motifs and trends
        self.track_motifs(&trace);

        Ok(CanonicalTurnResult {
            trace,
            receipt,
            discovered_nodes,
        })
    }

    fn audit_triggered(&self, input: &CanonicalInput, proposal: &CanonicalProposal) -> bool {
        if self.state.graph.criteria.audit_rate <= 0.0 {
            return false;
        }

        let mut hasher = Sha256::new();
        hasher.update(input.prompt.as_bytes());
        hasher.update(proposal.response.as_bytes());
        hasher.update(self.state.turn_count.to_le_bytes());
        let digest = hasher.finalize();
        let bucket = (digest[0] as f32) / 255.0;
        bucket <= self.state.graph.criteria.audit_rate
    }

    fn simulate_operations(&self, operations: &[Operation], _prompt: &str) -> SimulationReport {
        let mut max_risk = 0.0f64;
        let mut predicted_effects = Vec::new();
        let mut blocked_reasons = Vec::new();

        flatten_ops(operations, &mut |op| {
            let risk = op_risk(op);
            if risk > max_risk {
                max_risk = risk;
            }
            predicted_effects.push(op_label(op));
        });

        if max_risk > self.state.graph.criteria.max_risk {
            blocked_reasons.push(format!(
                "max_risk_exceeded:{:.2}>{:.2}",
                max_risk, self.state.graph.criteria.max_risk
            ));
        }

        if self.state.graph.criteria.require_read_before_write && has_write_before_read(operations)
        {
            blocked_reasons.push("write_before_read_in_plan".to_string());
        }

        SimulationReport {
            max_risk,
            predicted_effects,
            blocked_reasons: blocked_reasons.clone(),
            can_materialize: blocked_reasons.is_empty(),
        }
    }

    fn compute_coordinates(
        &self,
        input: &CanonicalInput,
        proposal: &CanonicalProposal,
    ) -> Vec<ScaleCoordinate> {
        let mut out = Vec::new();

        let token_coord = token_coordinate(&input.prompt, &proposal.response);
        out.push(token_coord);

        let active_turn = active_nodes(
            &self.state.graph,
            self.state.graph.criteria.activation_cutoff,
        );
        out.push(scale_from_active(Scale::Turn, &active_turn));

        let session_active = self
            .state
            .project_activation
            .iter()
            .filter(|(_, v)| *v >= self.state.graph.criteria.activation_cutoff * 0.8)
            .filter_map(|(id, v)| {
                self.state
                    .graph
                    .nodes
                    .iter()
                    .find(|n| &n.id == id)
                    .map(|n| (n.id.clone(), *v, n.prime_id))
            })
            .collect::<Vec<_>>();
        out.push(scale_from_active(Scale::Session, &session_active));

        let project_active = self
            .state
            .graph
            .nodes
            .iter()
            .filter(|n| n.reinforcements > 0)
            .map(|n| {
                let intensity = if self.state.turn_count == 0 {
                    0.0
                } else {
                    n.reinforcements as f32 / self.state.turn_count as f32
                };
                (n.id.clone(), intensity, n.prime_id)
            })
            .filter(|(_, intensity, _)| {
                *intensity >= self.state.graph.criteria.activation_cutoff * 0.5
            })
            .collect::<Vec<_>>();
        out.push(scale_from_active(Scale::Project, &project_active));

        out
    }

    fn update_project_activation(&mut self) {
        let mut map = HashMap::<String, f32>::new();
        for (id, v) in &self.state.project_activation {
            map.insert(id.clone(), *v);
        }
        for node in &self.state.graph.nodes {
            let current = map.get(&node.id).copied().unwrap_or(0.0);
            let next = (current * 0.8 + node.activation * 0.2).clamp(0.0, 1.0);
            map.insert(node.id.clone(), next);
        }

        let mut vec = map.into_iter().collect::<Vec<_>>();
        vec.sort_by(|a, b| a.0.cmp(&b.0));
        self.state.project_activation = vec;
    }

    fn make_receipt(
        &self,
        trace: &TurnTrace,
        observations: &[NodeObservation],
        discoveries: &[NodeDiscovery],
    ) -> CanonicalReceipt {
        let prev_hash = self
            .state
            .receipts
            .last()
            .map(|r| r.hash.clone())
            .unwrap_or_else(|| "genesis".to_string());

        let mut hasher = Sha256::new();
        hasher.update(trace.input.prompt.as_bytes());
        hasher.update(trace.proposal.response.as_bytes());
        hasher.update(format!("{:?}", trace.decision).as_bytes());
        hasher.update(prev_hash.as_bytes());
        for c in &trace.coordinates {
            hasher.update(format!("{:?}-{}-{}", c.scale, c.event_id, c.intensity).as_bytes());
        }
        let hash = format!("{:x}", hasher.finalize())[0..16].to_string();

        CanonicalReceipt {
            version: "1.0.0".to_string(),
            deterministic: false,
            turn: trace.input.turn,
            timestamp: Utc::now().to_rfc3339(),
            prev_hash,
            hash,
            recorded_input: trace.input.clone(),
            recorded_proposal: trace.proposal.clone(),
            recorded_observations: observations.to_vec(),
            recorded_discoveries: discoveries.to_vec(),
            proposal_quality: trace.proposal.quality,
            decision: trace.decision.clone(),
            gate_summary: trace.gate.summary(),
            audit_triggered: trace.audit_triggered,
            simulation_max_risk: trace.simulation.max_risk,
            invariant_passed: trace.invariants.passed,
            evidence_coverage: trace.invariants.evidence_coverage,
            contradiction_score: trace.invariants.contradiction_score,
            coordinates: trace.coordinates.clone(),
            cost: TurnCost::default(),
            discovered_nodes: discoveries.iter().map(|d| d.id.clone()).collect(),
            violations: trace.invariants.violations.clone(),
            criteria_before: trace.criteria_before.clone(),
            criteria_after: trace.criteria_after.clone(),
        }
    }

    fn track_motifs(&mut self, trace: &TurnTrace) {
        self.state.contradiction_history.push(trace.invariants.contradiction_score);
        if self.state.contradiction_history.len() > 100 {
            self.state.contradiction_history.remove(0);
        }

        if !trace.invariants.passed {
            for violation in &trace.invariants.violations {
                *self.state.motif_counts.entry(violation.clone()).or_insert(0) += 1;
            }
        }

        if matches!(trace.decision, TurnDecision::Rollback | TurnDecision::Halt) {
            *self.state.motif_counts.entry("system_interruption".to_string()).or_insert(0) += 1;
            self.state.intervention_count += 1;
        }
    }

    pub fn evaluate_promotion(&self) -> super::types::MotifReport {
        let repeated_failures = self.state.motif_counts.values().sum::<u64>();
        let trend_slope = self.calculate_contradiction_slope();

        let turns = self.state.turn_count;

        // Block promotion if any receipt in the chain has not been replay-verified.
        let has_unverified = self.state.receipts.iter().any(|r| !r.deterministic);

        let passes_promotion = if has_unverified {
            false // Replay verifier has not confirmed all receipts — promotion blocked
        } else if turns < 10 {
            false // Not enough data
        } else {
            // Significant negative slope of contradiction or failures
            trend_slope < -0.01 || (turns >= 20 && (repeated_failures as f32 / turns as f32) < 0.5)
        };

        super::types::MotifReport {
            repeated_failures,
            trend_slope,
            passes_promotion,
        }
    }

    fn calculate_contradiction_slope(&self) -> f32 {
        let history = &self.state.contradiction_history;
        if history.len() < 5 {
            return 0.0;
        }

        let n = history.len() as f32;
        let mut x_sum = 0.0;
        let mut y_sum = 0.0;
        let mut xy_sum = 0.0;
        let mut x2_sum = 0.0;

        for (i, &y) in history.iter().enumerate() {
            let x = i as f32;
            x_sum += x;
            y_sum += y;
            xy_sum += x * y;
            x2_sum += x * x;
        }

        let denominator = n * x2_sum - x_sum * x_sum;
        if denominator == 0.0 {
            0.0
        } else {
            (n * xy_sum - x_sum * y_sum) / denominator
        }
    }

    fn append_receipt(&self, receipts_path: &Path, receipt: &CanonicalReceipt) -> Result<()> {
        use std::io::Write;

        if let Some(parent) = receipts_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(receipts_path)?;
        writeln!(file, "{}", serde_json::to_string(receipt)?)?;
        Ok(())
    }
}

fn scale_from_active(scale: Scale, active: &[(String, f32, u64)]) -> ScaleCoordinate {
    let mut event_id: u64 = 1;
    let mut primes = Vec::new();
    let mut intensity_sum = 0.0f32;

    for (_, activation, prime) in active {
        event_id = event_id.saturating_mul(*prime);
        primes.push(*prime);
        intensity_sum += *activation;
    }

    let intensity = if active.is_empty() {
        0.0
    } else {
        intensity_sum / active.len() as f32
    };

    ScaleCoordinate {
        scale,
        event_id,
        primes,
        intensity,
        active_nodes: active.iter().map(|(id, _, _)| id.clone()).collect(),
    }
}

fn token_coordinate(prompt: &str, response: &str) -> ScaleCoordinate {
    let mut token_scores = HashMap::<String, f32>::new();

    for token in tokenize(prompt) {
        *token_scores.entry(token).or_insert(0.0) += 1.0;
    }
    for token in tokenize(response) {
        *token_scores.entry(token).or_insert(0.0) += 0.5;
    }

    let mut tokens = token_scores.into_iter().collect::<Vec<_>>();
    tokens.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    tokens.truncate(6);

    let mut active = Vec::new();
    for (token, score) in tokens {
        let prime = token_prime(&token);
        active.push((token, (score / 4.0).clamp(0.0, 1.0), prime));
    }

    scale_from_active(Scale::Token, &active)
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .map(|w| w.trim().to_lowercase())
        .filter(|w| w.len() >= 4)
        .take(64)
        .collect()
}

fn token_prime(token: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    const TOKEN_PRIMES: [u64; 128] = [
        2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
        97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181,
        191, 193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281,
        283, 293, 307, 311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397,
        401, 409, 419, 421, 431, 433, 439, 443, 449, 457, 461, 463, 467, 479, 487, 491, 499, 503,
        509, 521, 523, 541, 547, 557, 563, 569, 571, 577, 587, 593, 599, 601, 607, 613, 617, 619,
        631, 641, 643, 647, 653, 659, 661, 673, 677, 683, 691, 701, 709, 719,
    ];
    let idx = (digest[0] as usize) % TOKEN_PRIMES.len();
    TOKEN_PRIMES[idx]
}

fn has_write_before_read(ops: &[Operation]) -> bool {
    let mut seen_read = false;
    let mut bad = false;
    flatten_ops(ops, &mut |op| match op {
        Operation::FsRead { .. } => seen_read = true,
        Operation::FsWrite { .. } => {
            if !seen_read {
                bad = true;
            }
        }
        _ => {}
    });
    bad
}

fn op_risk(op: &Operation) -> f64 {
    match op {
        Operation::Shell { .. } => 0.7,
        Operation::FsRead { .. } => 0.2,
        Operation::FsWrite { .. } => 0.6,
        Operation::HttpGet { .. } => 0.5,
        Operation::GitPatch { .. } => 0.8,
        Operation::AssertFileExists { .. } => 0.1,
        Operation::AssertShellSuccess { .. } => 0.3,
        Operation::Attempt { operation } => op_risk(operation),
        Operation::Sequence { steps } => steps.iter().map(op_risk).fold(0.0, f64::max),
        Operation::Parallel { steps, .. } => steps.iter().map(op_risk).fold(0.0, f64::max),
        Operation::Conditional {
            condition,
            then_op,
            else_op,
        } => {
            let mut max = op_risk(condition);
            max = max.max(op_risk(then_op));
            if let Some(else_op) = else_op {
                max = max.max(op_risk(else_op));
            }
            max
        }
        Operation::Retry { operation, .. } => op_risk(operation),
    }
}

fn op_label(op: &Operation) -> String {
    match op {
        Operation::Shell { command, .. } => format!("shell:{}", command),
        Operation::FsRead { path, .. } => format!("fs.read:{}", path),
        Operation::FsWrite { path, .. } => format!("fs.write:{}", path),
        Operation::HttpGet { url, .. } => format!("http.get:{}", url),
        Operation::GitPatch { repo_path, .. } => format!("git.patch:{}", repo_path),
        Operation::AssertFileExists { path } => format!("assert.file_exists:{}", path),
        Operation::AssertShellSuccess { command, .. } => {
            format!("assert.shell_success:{}", command)
        }
        Operation::Attempt { .. } => "attempt".to_string(),
        Operation::Sequence { .. } => "sequence".to_string(),
        Operation::Parallel { .. } => "parallel".to_string(),
        Operation::Conditional { .. } => "conditional".to_string(),
        Operation::Retry { .. } => "retry".to_string(),
    }
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

fn append_rule_trajectory(
    path: &Path,
    turn: u64,
    old_rule: &str,
    new_rule: &str,
    proposal: &CanonicalProposal,
    edges: &[super::types::GraphEdge],
) -> Result<()> {
    use std::io::Write;

    let hypothesis_edges: Vec<f32> = edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Hypothesis))
        .map(|e| e.weight)
        .collect();

    let hypothesis_count = hypothesis_edges.len();
    let mean_weight = if hypothesis_edges.is_empty() {
        0.0_f64
    } else {
        let sum: f32 = hypothesis_edges.iter().sum();
        ((sum / hypothesis_edges.len() as f32) as f64 * 100.0).round() / 100.0
    };

    let reason = proposal
        .actions
        .first()
        .cloned()
        .unwrap_or_else(|| proposal.response.chars().take(120).collect());

    let entry = serde_json::json!({
        "turn": turn,
        "old_rule": old_rule,
        "new_rule": new_rule,
        "hypothesis_count": hypothesis_count,
        "mean_weight": mean_weight,
        "reason": reason,
    });

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(&entry)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::types::NodeObservation;

    #[test]
    fn audit_rate_zero_never_audits() {
        let mut core = CanonicalCore::new();
        core.state.graph.criteria.audit_rate = 0.0;
        let input = CanonicalInput {
            prompt: "hello".to_string(),
            context: vec![],
            turn: 1,
        };
        let proposal = CanonicalProposal {
            response: "world".to_string(),
            actions: vec![],
            errors: vec![],
            quality: 1.0,
            operations: vec![],
            ovm_ops: vec![],
        };
        assert!(!core.audit_triggered(&input, &proposal));
    }

    #[test]
    fn simulation_blocks_write_before_read() {
        let core = CanonicalCore::new();
        let ops = vec![Operation::FsWrite {
            path: "a.txt".to_string(),
            content: "x".to_string(),
            mode: "0644".to_string(),
            create_dirs: false,
        }];
        let sim = core.simulate_operations(&ops, "write file");
        assert!(!sim.can_materialize);
    }

    #[test]
    fn commit_path_with_no_ops() {
        let mut core = CanonicalCore::new();
        let input = CanonicalInput {
            prompt: "explain bounds bug".to_string(),
            context: vec![],
            turn: 1,
        };
        let proposal = CanonicalProposal {
            response: "explanation".to_string(),
            actions: vec![],
            errors: vec![],
            quality: 0.9,
            operations: vec![],
            ovm_ops: vec![],
        };
        let obs = vec![NodeObservation {
            id: "sig:analysis".to_string(),
            label: "analysis".to_string(),
            condition: "analysis task".to_string(),
            activation: 0.8,
            control_signals: vec![],
            threshold: 0.7,
        }];

        let guard = GuardConfig::from_env();
        let receipt_path = std::env::temp_dir().join("canonical_core_test_receipts.jsonl");
        let _ = std::fs::remove_file(&receipt_path);
        let res = core
            .process_turn(input, proposal, obs, vec![], &guard, &receipt_path)
            .expect("turn should process");
        assert!(matches!(res.trace.decision, TurnDecision::Commit));
        assert!(res.trace.invariants.passed);
        let _ = std::fs::remove_file(&receipt_path);
    }

    #[test]
    fn invalid_ovm_rule_forces_rollback() {
        let mut core = CanonicalCore::new();
        let input = CanonicalInput {
            prompt: "compare signal strength".to_string(),
            context: vec![],
            turn: 1,
        };
        let proposal = CanonicalProposal {
            response: "signal strength differs".to_string(),
            actions: vec![],
            errors: vec![],
            quality: 0.9,
            operations: vec![],
            ovm_ops: vec![OvmOp::DefineScoringRule {
                rule: "sqrt(".to_string(),
            }],
        };

        let guard = GuardConfig::from_env();
        let receipt_path =
            std::env::temp_dir().join("canonical_core_invalid_ovm_rule_receipts.jsonl");
        let _ = std::fs::remove_file(&receipt_path);
        let res = core
            .process_turn(input, proposal, vec![], vec![], &guard, &receipt_path)
            .expect("turn should process");
        assert!(matches!(res.trace.decision, TurnDecision::Rollback));
        assert!(
            res.trace
                .invariants
                .violations
                .iter()
                .any(|v| v.starts_with("ovm:scoring_eval_failed"))
        );
        let _ = std::fs::remove_file(&receipt_path);
    }
}
