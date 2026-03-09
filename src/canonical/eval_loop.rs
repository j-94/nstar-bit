/// Adaptive activation thresholding and competitive selection.
///
/// The problem this solves:
///   With a fixed 0.4 cutoff, the canonical graph collapsed to a 22-node clique
///   (all at activation=1.0, reinforcements≈41-43) while 217/240 nodes were
///   permanently dark. Z-score thresholding (mean+1.5σ) makes this worse, not better,
///   because the distribution is bimodal (0.0 or 1.0) — σ is high but there's nothing
///   between the two poles.
///
/// The actual fix: competitive selection + temporal decay.
///
///   1. competitive_cutoff() — top-K percentile, not a fixed value.
///      Always activates the top 10-15% of nodes regardless of absolute values.
///      Forces diversity: even if 200 nodes are at 0.3 and 22 are at 1.0,
///      the next tier gets a chance when the top clique is crowded.
///
///   2. apply_activation_decay() — per-turn multiplicative decay.
///      Nodes that weren't seeded this turn lose 20% of their activation.
///      Breaks the fixed-point attractor: a node must be re-observed to stay active.
///      Without decay, high-reinforcement nodes accumulate to 1.0 and never drop.
///
///   3. z_score_cutoff() — included but honest about when it applies.
///      Only meaningful when the activation distribution is approximately Gaussian.
///      Check distribution_is_gaussian() before using it.

use super::types::GraphState;

/// Returns a competitive activation cutoff: the threshold that passes
/// the top `top_fraction` of nodes.
///
/// If top_fraction=0.15, returns the 85th percentile activation value.
/// This ensures roughly the same number of nodes participate per turn,
/// regardless of absolute activation levels.
///
/// Minimum returned value: min_cutoff (default 0.05) to prevent
/// activating near-zero nodes when everything has decayed.
pub fn competitive_cutoff(graph: &GraphState, top_fraction: f32, min_cutoff: f32) -> f32 {
    if graph.nodes.is_empty() {
        return min_cutoff;
    }

    let mut activations: Vec<f32> = graph.nodes.iter().map(|n| n.activation).collect();
    activations.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let k = ((activations.len() as f32 * top_fraction).ceil() as usize).max(1);
    let threshold = activations.get(k - 1).copied().unwrap_or(0.0);
    threshold.max(min_cutoff)
}

/// Returns the Z-score cutoff: mean + k_sigma * stddev.
///
/// Only use this when the activation distribution is approximately Gaussian.
/// Call `distribution_is_gaussian()` first. On a bimodal 0/1 distribution,
/// this returns a value in the gap and behaves identically to a fixed threshold.
pub fn z_score_cutoff(graph: &GraphState, k_sigma: f32) -> f32 {
    let n = graph.nodes.len();
    if n == 0 {
        return 0.4;
    }

    let mean = graph.nodes.iter().map(|n| n.activation).sum::<f32>() / n as f32;
    let variance = graph
        .nodes
        .iter()
        .map(|n| (n.activation - mean).powi(2))
        .sum::<f32>()
        / n as f32;
    let stddev = variance.sqrt();

    (mean + k_sigma * stddev).clamp(0.0, 1.0)
}

/// Returns true if the activation distribution is approximately Gaussian
/// (skewness below threshold, no strong bimodal spike at 0 or 1).
///
/// A bimodal distribution like 217 nodes at 0.0 + 22 nodes at 1.0
/// returns false — Z-score thresholding is not meaningful in that case.
pub fn distribution_is_gaussian(graph: &GraphState) -> bool {
    if graph.nodes.len() < 10 {
        return false;
    }

    let n = graph.nodes.len() as f32;
    let at_zero = graph.nodes.iter().filter(|node| node.activation == 0.0).count() as f32;
    let at_one = graph.nodes.iter().filter(|node| node.activation >= 0.99).count() as f32;

    // If more than 60% of nodes are at exactly 0 or exactly 1, it's bimodal.
    let bimodal_fraction = (at_zero + at_one) / n;
    bimodal_fraction < 0.6
}

/// Apply per-turn multiplicative decay to all node activations.
///
/// Nodes that were seeded this turn (via apply_observations) won't be affected
/// because apply_observations runs after decay and resets from observations.
/// Nodes that were NOT seeded lose `decay_rate` fraction of their activation.
///
/// Typical decay_rate: 0.15-0.25 (15-25% loss per turn).
/// At 0.20: a node at 1.0 that isn't re-observed reaches 0.4 in ~4 turns,
/// then drops below the typical competitive_cutoff and stops participating.
///
/// Call this BEFORE apply_observations each turn.
pub fn apply_activation_decay(graph: &mut GraphState, decay_rate: f32) {
    let retain = (1.0 - decay_rate).clamp(0.0, 1.0);
    for node in &mut graph.nodes {
        node.activation *= retain;
    }
}

/// Returns the recommended cutoff for the current graph state.
///
/// Selects the right strategy based on the activation distribution:
/// - Bimodal (0/1 spike): competitive_cutoff to force diversity
/// - Gaussian-ish: z_score_cutoff for statistical discrimination
///
/// This is the single function to call from apply_operator / learn_coactivation_edges
/// when you want adaptive behavior.
pub fn adaptive_cutoff(graph: &GraphState, criteria_cutoff: f32) -> f32 {
    if distribution_is_gaussian(graph) {
        // Z-score is valid: use it
        z_score_cutoff(graph, 1.5)
    } else {
        // Bimodal or sparse: competitive top-15%, never below criteria cutoff
        competitive_cutoff(graph, 0.15, criteria_cutoff)
    }
}

/// Diagnostic snapshot of the activation distribution.
/// Use this to understand which regime you're in before choosing a strategy.
#[derive(Debug)]
pub struct ActivationStats {
    pub count: usize,
    pub mean: f32,
    pub stddev: f32,
    pub at_zero: usize,
    pub at_one: usize,
    pub bimodal_fraction: f32,
    pub is_gaussian: bool,
    pub competitive_cutoff_15pct: f32,
    pub z_score_cutoff_1_5: f32,
    pub fixed_cutoff: f32,
    pub active_at_fixed: usize,
    pub active_at_competitive: usize,
    pub active_at_z: usize,
}

pub fn activation_stats(graph: &GraphState, fixed_cutoff: f32) -> ActivationStats {
    let n = graph.nodes.len();
    if n == 0 {
        return ActivationStats {
            count: 0,
            mean: 0.0,
            stddev: 0.0,
            at_zero: 0,
            at_one: 0,
            bimodal_fraction: 0.0,
            is_gaussian: false,
            competitive_cutoff_15pct: fixed_cutoff,
            z_score_cutoff_1_5: fixed_cutoff,
            fixed_cutoff,
            active_at_fixed: 0,
            active_at_competitive: 0,
            active_at_z: 0,
        };
    }

    let mean = graph.nodes.iter().map(|n| n.activation).sum::<f32>() / n as f32;
    let variance = graph
        .nodes
        .iter()
        .map(|n| (n.activation - mean).powi(2))
        .sum::<f32>()
        / n as f32;
    let stddev = variance.sqrt();

    let at_zero = graph.nodes.iter().filter(|n| n.activation == 0.0).count();
    let at_one = graph.nodes.iter().filter(|n| n.activation >= 0.99).count();
    let bimodal_fraction = (at_zero + at_one) as f32 / n as f32;
    let is_gaussian = distribution_is_gaussian(graph);

    let comp = competitive_cutoff(graph, 0.15, fixed_cutoff);
    let z = z_score_cutoff(graph, 1.5);

    let active_at_fixed = graph.nodes.iter().filter(|n| n.activation >= fixed_cutoff).count();
    let active_at_competitive = graph.nodes.iter().filter(|n| n.activation >= comp).count();
    let active_at_z = graph.nodes.iter().filter(|n| n.activation >= z).count();

    ActivationStats {
        count: n,
        mean,
        stddev,
        at_zero,
        at_one,
        bimodal_fraction,
        is_gaussian,
        competitive_cutoff_15pct: comp,
        z_score_cutoff_1_5: z,
        fixed_cutoff,
        active_at_fixed,
        active_at_competitive,
        active_at_z,
    }
}
