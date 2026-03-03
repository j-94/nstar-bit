use std::collections::HashMap;

use super::types::{
    CanonicalConfig, EdgeKind, GateDecision, GatePattern, GraphEdge, GraphState,
    NodeDiscovery, NodeObservation,
};

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
            }
        }

        for (idx, node) in graph.nodes.iter_mut().enumerate() {
            node.activation = clamp01(node.activation + delta[idx]);
        }
    }
}

pub fn evaluate_gates(graph: &GraphState, config: &CanonicalConfig) -> GateDecision {
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
                .map(|n| n.activation >= config.activation_cutoff)
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
                .map(|n| n.activation >= config.activation_cutoff)
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

pub fn learn_coactivation_edges(graph: &mut GraphState, cutoff: f32) {
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
            maybe_add_or_reinforce_edge(graph, a, b, 0.08, EdgeKind::Supports);
            maybe_add_or_reinforce_edge(graph, b, a, 0.08, EdgeKind::Supports);
        }
    }
}

fn maybe_add_or_reinforce_edge(
    graph: &mut GraphState,
    from: &str,
    to: &str,
    delta: f32,
    kind: EdgeKind,
) {
    if let Some(edge) = graph
        .edges
        .iter_mut()
        .find(|e| e.from == from && e.to == to && std::mem::discriminant(&e.kind) == std::mem::discriminant(&kind))
    {
        edge.weight = (edge.weight + delta).clamp(0.0, 1.0);
    } else {
        graph.edges.push(GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            weight: delta,
            kind,
        });
    }
}
