use std::collections::HashMap;

use super::types::{
    CausalAnchor, EdgeKind, GateDecision, GatePattern, GraphEdge, GraphState, NodeDiscovery,
    NodeObservation, RuleScorecard, ScorecardEdge, CanonicalReceipt, Scale,
};
use evalexpr::{ContextWithMutableFunctions, ContextWithMutableVariables};

fn is_prime(n: u64) -> bool {
    if n <= 1 {
        return false;
    }
    if n <= 3 {
        return true;
    }
    if n % 2 == 0 || n % 3 == 0 {
        return false;
    }
    let mut i = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    true
}

fn next_prime(after: u64) -> u64 {
    let mut candidate = if after < 2 { 2 } else { after + 1 };
    while !is_prime(candidate) {
        candidate += 1;
    }
    candidate
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn get_node_idx(graph: &GraphState, id: &str) -> Option<usize> {
    graph.nodes.iter().position(|n| n.id == id)
}

fn current_max_prime(graph: &GraphState) -> u64 {
    graph.nodes.iter().map(|n| n.prime_id).max().unwrap_or(1)
}

fn ensure_node(graph: &mut GraphState, obs: &NodeObservation, discovered_turn: u64) {
    if let Some(idx) = get_node_idx(graph, &obs.id) {
        let node = &mut graph.nodes[idx];
        node.label = obs.label.clone();
        node.condition = obs.condition.clone();
        node.control_signals = obs.control_signals.clone();
        node.threshold = obs.threshold;
    } else {
        let prime = next_prime(current_max_prime(graph));
        graph.nodes.push(super::types::GraphNode {
            id: obs.id.clone(),
            label: obs.label.clone(),
            condition: obs.condition.clone(),
            prime_id: prime,
            control_signals: obs.control_signals.clone(),
            threshold: obs.threshold,
            activation: 0.0,
            discovered_turn,
            reinforcements: 0,
        });
    }
}

pub fn apply_observations(
    graph: &mut GraphState,
    observations: &[NodeObservation],
    discovered_turn: u64,
) {
    for obs in observations {
        ensure_node(graph, obs, discovered_turn);
    }

    // Reset all activations before applying this turn's observed seed activations.
    for node in &mut graph.nodes {
        node.activation = 0.0;
    }

    for obs in observations {
        if let Some(idx) = get_node_idx(graph, &obs.id) {
            graph.nodes[idx].activation = clamp01(obs.activation);
        }
    }
}

pub fn apply_discoveries(
    graph: &mut GraphState,
    discoveries: &[NodeDiscovery],
    discovered_turn: u64,
) -> Vec<String> {
    let mut new_nodes = Vec::new();

    for d in discoveries {
        let obs = NodeObservation {
            id: d.id.clone(),
            label: d.label.clone(),
            condition: d.condition.clone(),
            activation: 0.0,
            control_signals: d.control_signals.clone(),
            threshold: d.threshold,
        };

        let existed = get_node_idx(graph, &d.id).is_some();
        ensure_node(graph, &obs, discovered_turn);
        if !existed {
            new_nodes.push(d.id.clone());
        }

        if !d.require_all.is_empty() || !d.block_any.is_empty() {
            graph.patterns.push(GatePattern {
                require_all: d.require_all.clone(),
                block_any: d.block_any.clone(),
                control_signals: d.control_signals.clone(),
                reason: format!("pattern from discovery:{}", d.id),
            });
        }
    }

    new_nodes
}

pub fn propagate_activations(graph: &mut GraphState, steps: usize) {
    if graph.edges.is_empty() || graph.nodes.is_empty() {
        return;
    }

    let idx_map: HashMap<String, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), i))
        .collect();

    for _ in 0..steps {
        let mut delta = vec![0.0f32; graph.nodes.len()];

        for edge in &graph.edges {
            let Some(&from_idx) = idx_map.get(edge.from.as_str()) else {
                continue;
            };
            let Some(&to_idx) = idx_map.get(edge.to.as_str()) else {
                continue;
            };

            let source = graph.nodes[from_idx].activation;
            let effect = source * edge.weight;
            match edge.kind {
                EdgeKind::Supports => {
                    delta[to_idx] += effect;
                }
                EdgeKind::Inhibits => {
                    delta[to_idx] -= effect;
                }
                EdgeKind::Hypothesis => {} // Substrate does not propagate by default
            }
        }

        for (idx, node) in graph.nodes.iter_mut().enumerate() {
            node.activation = clamp01(node.activation + delta[idx]);
        }
    }
}

pub fn evaluate_gates(graph: &GraphState) -> GateDecision {
    let mut emitted_signals = Vec::new();

    for node in &graph.nodes {
        if node.activation >= node.threshold {
            emitted_signals.extend(node.control_signals.clone());
        }
    }

    for pattern in &graph.patterns {
        let required_ok = pattern.require_all.iter().all(|id| {
            graph
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .map(|n| n.activation >= graph.criteria.activation_cutoff)
                .unwrap_or(false)
        });

        if !required_ok {
            continue;
        }

        let blocked = pattern.block_any.iter().any(|id| {
            graph
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .map(|n| n.activation >= graph.criteria.activation_cutoff)
                .unwrap_or(false)
        });

        if blocked {
            continue;
        }

        emitted_signals.extend(pattern.control_signals.clone());
    }

    GateDecision { emitted_signals }
}

pub fn active_nodes(graph: &GraphState, cutoff: f32) -> Vec<(String, f32, u64)> {
    let mut out = graph
        .nodes
        .iter()
        .filter(|n| n.activation >= cutoff)
        .map(|n| (n.id.clone(), n.activation, n.prime_id))
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub fn reinforce_active_nodes(graph: &mut GraphState, cutoff: f32) {
    for node in &mut graph.nodes {
        if node.activation >= cutoff {
            node.reinforcements += 1;
        }
    }
}

pub fn learn_coactivation_edges(graph: &mut GraphState, cutoff: f32, turn: u64) {
    let active = active_nodes(graph, cutoff)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect::<Vec<_>>();

    if active.len() < 2 {
        return;
    }

    for i in 0..active.len() {
        for j in (i + 1)..active.len() {
            let a = &active[i];
            let b = &active[j];
            let snippet = format!("{} ∧ {}", a, b);
            let anchor = CausalAnchor { turn_id: turn, snippet };
            maybe_add_or_reinforce_edge(graph, a, b, 0.08, EdgeKind::Supports, Some(anchor.clone()));
            maybe_add_or_reinforce_edge(graph, b, a, 0.08, EdgeKind::Supports, Some(anchor));
        }
    }
}

fn maybe_add_or_reinforce_edge(
    graph: &mut GraphState,
    from: &str,
    to: &str,
    delta: f32,
    kind: EdgeKind,
    anchor: Option<CausalAnchor>,
) {
    if let Some(edge) = graph.edges.iter_mut().find(|e| {
        e.from == from
            && e.to == to
            && std::mem::discriminant(&e.kind) == std::mem::discriminant(&kind)
    }) {
        edge.weight = (edge.weight + delta).clamp(0.0, 1.0);
        if let Some(a) = anchor {
            if edge.anchors.len() < 8 {
                edge.anchors.push(a);
            }
        }
    } else {
        let anchors = anchor.into_iter().collect();
        graph.edges.push(GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            weight: delta,
            kind,
            c11: 0,
            c10: 0,
            c01: 0,
            c00: 0,
            anchors,
        });
    }
}

/// Update the hypothesis substrate using behavioral node co-occurrence.
///
/// Tracks which `node:` predicates activate together across turns.
/// This is the empirical signal the scoring rule learns from — not word
/// presence but behavioral co-activation. A node pair's counts reflect
/// whether those two behavioral dimensions genuinely co-occur.
pub fn update_hypothesis_substrate(
    graph: &mut GraphState,
    observations: &[NodeObservation],
    turn: u64,
) {
    // Only track behavioral nodes (node: prefix), not sym: word nodes.
    let behavioral_ids: Vec<String> = graph
        .nodes
        .iter()
        .filter(|n| n.id.starts_with("node:"))
        .map(|n| n.id.clone())
        .collect();

    if behavioral_ids.len() < 2 {
        return;
    }

    let cutoff = graph.criteria.activation_cutoff;

    // Which behavioral nodes are active this turn?
    let active_set: std::collections::HashSet<&str> = observations
        .iter()
        .filter(|obs| obs.activation >= cutoff)
        .map(|obs| obs.id.as_str())
        .collect();

    // Ensure hypothesis edges exist between all behavioral node pairs.
    for i in 0..behavioral_ids.len() {
        for j in (i + 1)..behavioral_ids.len() {
            ensure_hypothesis_edge(graph, &behavioral_ids[i], &behavioral_ids[j]);
        }
    }

    // Update contingency counts for all behavioral hypothesis edges.
    for edge in &mut graph.edges {
        if !matches!(edge.kind, EdgeKind::Hypothesis) {
            continue;
        }
        if !edge.from.starts_with("node:") || !edge.to.starts_with("node:") {
            continue;
        }
        let a_on = active_set.contains(edge.from.as_str());
        let b_on = active_set.contains(edge.to.as_str());
        match (a_on, b_on) {
            (true, true) => {
                edge.c11 += 1;
                if edge.anchors.len() < 8 {
                    let snippet = format!("{} ∧ {}", edge.from, edge.to);
                    edge.anchors.push(CausalAnchor { turn_id: turn, snippet });
                }
            }
            (true, false) => edge.c10 += 1,
            (false, true) => edge.c01 += 1,
            (false, false) => edge.c00 += 1,
        }
    }
}

fn ensure_hypothesis_edge(graph: &mut GraphState, a: &str, b: &str) {
    let exists = graph.edges.iter().any(|e| {
        matches!(e.kind, EdgeKind::Hypothesis)
            && ((e.from == a && e.to == b) || (e.from == b && e.to == a))
    });

    if !exists {
        graph.edges.push(GraphEdge {
            from: a.to_string(),
            to: b.to_string(),
            weight: 0.0,
            kind: EdgeKind::Hypothesis,
            c11: 0,
            c10: 0,
            c01: 0,
            c00: 0,
            anchors: vec![],
        });
    }
}

pub fn apply_operator(graph: &mut GraphState) -> Vec<String> {
    use evalexpr::HashMapContext;

    let mut violations: Vec<String> = Vec::new();

    if graph.scoring_rule.is_empty() {
        // Bootstrap path: until a rule exists, the substrate accumulates counts
        // but does not score or activate hypothesis edges.
        return violations;
    }

    let mut hypotheses: Vec<usize> = Vec::new();
    for (idx, e) in graph.edges.iter().enumerate() {
        if matches!(e.kind, EdgeKind::Hypothesis) {
            hypotheses.push(idx);
        }
    }

    if hypotheses.is_empty() {
        return violations;
    }

    for idx in hypotheses {
        let (c11, c10, c01, c00, t) = {
            let e = &graph.edges[idx];
            (
                e.c11 as f64,
                e.c10 as f64,
                e.c01 as f64,
                e.c00 as f64,
                (e.c11 + e.c10 + e.c01 + e.c00) as f64,
            )
        };

        if t == 0.0 {
            let e = &graph.edges[idx];
            violations.push(format!("ovm:zero_observations:{}→{}", e.from, e.to));
            continue;
        }

        let mut context = HashMapContext::new();
        let _ = context.set_value("c11".into(), c11.into());
        let _ = context.set_value("c10".into(), c10.into());
        let _ = context.set_value("c01".into(), c01.into());
        let _ = context.set_value("c00".into(), c00.into());
        let _ = context.set_value("t".into(), t.into());

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

        let score = match evalexpr::eval_float_with_context(&graph.scoring_rule, &context) {
            Ok(s) => s,
            Err(e) => {
                violations.push(format!("ovm:scoring_eval_failed:{}", e));
                graph.edges[idx].weight = 0.0;
                continue;
            }
        };

        let _ = context.set_value("score".into(), score.into());

        let active = if !graph.selection_predicate.is_empty() {
            match evalexpr::eval_boolean_with_context(&graph.selection_predicate, &context) {
                Ok(b) => b,
                Err(e) => {
                    violations.push(format!("ovm:predicate_eval_failed:{}", e));
                    false
                }
            }
        } else {
            score > 0.0
        };

        if active {
            let edge = &mut graph.edges[idx];
            edge.weight = score as f32;

            let from_id = edge.from.clone();
            let to_id = edge.to.clone();

            if let Some(n) = graph.nodes.iter_mut().find(|n| n.id == from_id) {
                n.activation = (n.activation + (score as f32 * 0.1)).clamp(0.0, 1.0);
            }
            if let Some(n) = graph.nodes.iter_mut().find(|n| n.id == to_id) {
                n.activation = (n.activation + (score as f32 * 0.1)).clamp(0.0, 1.0);
            }
        } else {
            graph.edges[idx].weight = 0.0;
        }
    }

    violations
}

/// Evaluate the current scoring rule using held-out prediction.
///
/// Splits receipts in half by turn. Builds contingency counts (c11/c10/c01/c00)
/// from Token-scale active nodes in the first half. Ranks all hypothesis edge
/// pairs under the current rule. Measures precision@K and recall@K against
/// co-occurrences observed in the second half.
pub fn evaluate_rule_heldout(
    graph: &GraphState,
    receipts: &[CanonicalReceipt],
    k: usize,
) -> Option<RuleScorecard> {
    if graph.scoring_rule.is_empty() || receipts.len() < 4 {
        return None;
    }

    let split = receipts.len() / 2;
    let train = &receipts[..split];
    let test = &receipts[split..];

    // Extract Turn-scale active behavioral node sets per turn.
    // Scale::Turn captures which node: predicates were active above the
    // activation cutoff — actual behavioral co-occurrence, not word presence.
    let extract = |r: &CanonicalReceipt| -> Vec<String> {
        r.coordinates
            .iter()
            .filter(|c| c.scale == Scale::Turn)
            .flat_map(|c| c.active_nodes.iter().cloned())
            .filter(|n| n.starts_with("node:"))
            .collect()
    };

    // Build contingency counts from a set of turns
    let build_counts = |turns: &[CanonicalReceipt]| -> HashMap<(String, String), [u64; 4]> {
        let mut all_nodes: Vec<String> = turns
            .iter()
            .flat_map(|r| extract(r))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        all_nodes.sort();

        let per_turn: Vec<std::collections::HashSet<String>> = turns
            .iter()
            .map(|r| extract(r).into_iter().collect())
            .collect();

        let mut counts: HashMap<(String, String), [u64; 4]> = HashMap::new();
        for i in 0..all_nodes.len() {
            for j in (i + 1)..all_nodes.len() {
                let a = &all_nodes[i];
                let b = &all_nodes[j];
                let mut c = [0u64; 4]; // c11, c10, c01, c00
                for active in &per_turn {
                    let a_on = active.contains(a);
                    let b_on = active.contains(b);
                    match (a_on, b_on) {
                        (true, true) => c[0] += 1,
                        (true, false) => c[1] += 1,
                        (false, true) => c[2] += 1,
                        (false, false) => c[3] += 1,
                    }
                }
                if c[0] > 0 || c[1] > 0 || c[2] > 0 {
                    counts.insert((a.clone(), b.clone()), c);
                }
            }
        }
        counts
    };

    let train_counts = build_counts(train);
    let test_counts = build_counts(test);

    // Test positives: pairs with c11 > 0 in test
    let test_positives: std::collections::HashSet<(String, String)> = test_counts
        .iter()
        .filter(|(_, c)| c[0] > 0)
        .map(|(k, _)| k.clone())
        .collect();

    if test_positives.is_empty() {
        return None;
    }

    // Score and rank training candidates (c11 >= 1 in training)
    let rule = &graph.scoring_rule;
    let mut scored: Vec<(f32, (String, String), [u64; 4])> = train_counts
        .iter()
        .filter(|(_, c)| c[0] >= 1)
        .map(|(pair, c)| {
            let s = eval_rule_on_counts(rule, c[0], c[1], c[2], c[3]);
            (s, pair.clone(), *c)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let effective_k = k.min(scored.len());
    let top_k = &scored[..effective_k];
    let hits: usize = top_k
        .iter()
        .filter(|(_, pair, _)| test_positives.contains(pair))
        .count();

    let precision = hits as f32 / effective_k as f32;
    let recall = hits as f32 / test_positives.len() as f32;

    let top_hits: Vec<ScorecardEdge> = top_k
        .iter()
        .enumerate()
        .filter(|(_, (_, pair, _))| test_positives.contains(pair))
        .take(5)
        .map(|(rank, (score, pair, c))| ScorecardEdge {
            from: pair.0.clone(),
            to: pair.1.clone(),
            c11: c[0], c10: c[1], c01: c[2], c00: c[3],
            score: *score,
            rank,
        })
        .collect();

    let top_misses: Vec<ScorecardEdge> = top_k
        .iter()
        .enumerate()
        .filter(|(_, (_, pair, _))| !test_positives.contains(pair))
        .take(5)
        .map(|(rank, (score, pair, c))| ScorecardEdge {
            from: pair.0.clone(),
            to: pair.1.clone(),
            c11: c[0], c10: c[1], c01: c[2], c00: c[3],
            score: *score,
            rank,
        })
        .collect();

    Some(RuleScorecard {
        rule: rule.clone(),
        precision_at_k: precision,
        recall_at_k: recall,
        k: effective_k,
        train_turns: train.len(),
        test_turns: test.len(),
        top_misses,
        top_hits,
    })
}

fn eval_rule_on_counts(rule: &str, c11: u64, c10: u64, c01: u64, c00: u64) -> f32 {
    let t = (c11 + c10 + c01 + c00).max(1) as f64;
    let mut ctx = evalexpr::HashMapContext::new();
    let _ = ctx.set_value("c11".into(), evalexpr::Value::Float(c11 as f64));
    let _ = ctx.set_value("c10".into(), evalexpr::Value::Float(c10 as f64));
    let _ = ctx.set_value("c01".into(), evalexpr::Value::Float(c01 as f64));
    let _ = ctx.set_value("c00".into(), evalexpr::Value::Float(c00 as f64));
    let _ = ctx.set_value("t".into(), evalexpr::Value::Float(t));
    let _ = ctx.set_function("log".into(),
        evalexpr::Function::new(|v| Ok(v.as_float()?.ln().into())));
    let _ = ctx.set_function("sqrt".into(),
        evalexpr::Function::new(|v| Ok(v.as_float()?.sqrt().into())));
    let _ = ctx.set_function("abs".into(),
        evalexpr::Function::new(|v| Ok(v.as_float()?.abs().into())));
    let _ = ctx.set_function("max".into(),
        evalexpr::Function::new(|v| {
            let t = v.as_tuple()?;
            let a = t[0].as_float()?;
            let b = t[1].as_float()?;
            Ok(a.max(b).into())
        }));
    let _ = ctx.set_function("min".into(),
        evalexpr::Function::new(|v| {
            let t = v.as_tuple()?;
            let a = t[0].as_float()?;
            let b = t[1].as_float()?;
            Ok(a.min(b).into())
        }));
    match evalexpr::eval_with_context(rule, &ctx) {
        Ok(evalexpr::Value::Float(f)) => f as f32,
        Ok(evalexpr::Value::Int(i)) => i as f32,
        _ => 0.0,
    }
}
