//! The Collapse — the core function of nstar-bit.
//!
//! `n★(ins, outs) → collapsed_state`
//!
//! Takes what went in and what came out of a computation turn,
//! and produces the causal collapse: a snapshot of which predicates
//! are active, which gates fired, and what the system learned.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::predicate::{GateType, Predicate};

/// The inputs to a turn — what went in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnIns {
    /// The user's message or task
    pub prompt: String,

    /// Context provided (file contents, history, etc.)
    pub context: Vec<String>,

    /// Turn number in this session
    pub turn: u64,
}

/// The outputs of a turn — what came out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnOuts {
    /// The system's response
    pub response: String,

    /// Actions taken (tools called, files written, etc.)
    pub actions: Vec<String>,

    /// Self-assessed quality (0.0 to 1.0)
    pub quality: f32,

    /// Any errors or issues
    pub errors: Vec<String>,
}

/// The causal collapse — the result of n★(ins, outs).
///
/// This is what remains after a turn: the fingerprint of which
/// computation actually occurred, expressed as an activation pattern
/// over the current predicate set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collapse {
    /// Unique hash of this collapse
    pub hash: String,

    /// When this collapse occurred
    pub timestamp: String,

    /// Turn number
    pub turn: u64,

    /// The activation pattern — predicate name → activation level
    pub activations: Vec<(String, f32)>,

    /// Which gates fired
    pub gates_fired: Vec<FiredGate>,

    /// New predicate discovered this turn (if any)
    pub discovered: Option<PredicateDiscovery>,

    /// Overall quality of this turn
    pub quality: f32,

    /// Dimensionality — how many predicates were active (> 0.0)
    pub n: usize,
}

/// A gate that fired during this turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiredGate {
    pub predicate_name: String,
    pub gate_type: GateType,
    pub activation: f32,
}

/// A new predicate discovered during the reflection pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredicateDiscovery {
    pub name: String,
    pub activation_condition: String,
    pub gate: GateType,
    pub reason: String,
}

impl Collapse {
    /// Compute the causal collapse for a turn.
    ///
    /// This is the n★ function. It evaluates each predicate against
    /// the turn's ins/outs and produces the collapsed state.
    pub fn compute(
        ins: &TurnIns,
        outs: &TurnOuts,
        predicates: &mut [Predicate],
        evaluations: &[(String, f32)],
    ) -> Self {
        // Step 1: Evaluate each predicate's activation
        // In the nstar-bit protocol, this requires LM evaluation.
        let mut activations = Vec::new();
        let mut gates_fired = Vec::new();

        for pred in predicates.iter_mut() {
            let activation = evaluations
                .iter()
                .find(|(n, _)| n == &pred.name)
                .map(|(_, a)| *a)
                .unwrap_or(0.0);
                
            pred.activate(activation);
            activations.push((pred.name.clone(), activation));

            if pred.gate_fires() {
                gates_fired.push(FiredGate {
                    predicate_name: pred.name.clone(),
                    gate_type: pred.gate.clone(),
                    activation: pred.activation,
                });
            }
        }

        // Step 2: Compute dimensionality (how many predicates are active)
        let n = activations.iter().filter(|(_, a)| *a > 0.0).count();

        // Step 3: Compute hash (the receipt — proof this collapse occurred)
        let hash = collapse_hash(ins, outs, &activations);

        Collapse {
            hash,
            timestamp: Utc::now().to_rfc3339(),
            turn: ins.turn,
            activations,
            gates_fired,
            discovered: None, // Set by the reflection pass
            quality: outs.quality,
            n,
        }
    }
}

/// Compute a SHA-256 hash of the collapse for receipt purposes.
fn collapse_hash(ins: &TurnIns, outs: &TurnOuts, activations: &[(String, f32)]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ins.prompt.as_bytes());
    hasher.update(outs.response.as_bytes());
    for (name, val) in activations {
        hasher.update(name.as_bytes());
        hasher.update(val.to_le_bytes());
    }
    let result = hasher.finalize();
    format!("{:x}", result)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_with_no_predicates() {
        let ins = TurnIns {
            prompt: "Fix the auth bug".to_string(),
            context: vec![],
            turn: 1,
        };
        let outs = TurnOuts {
            response: "Done".to_string(),
            actions: vec![],
            quality: 0.8,
            errors: vec![],
        };
        let mut predicates: Vec<Predicate> = vec![];
        let collapse = Collapse::compute(&ins, &outs, &mut predicates, &[]);
        assert_eq!(collapse.n, 0);
        assert_eq!(collapse.turn, 1);
        assert!(collapse.gates_fired.is_empty());
    }

    #[test]
    fn collapse_detects_error_predicate() {
        let ins = TurnIns {
            prompt: "Why is this failing?".to_string(),
            context: vec![],
            turn: 3,
        };
        let outs = TurnOuts {
            response: "There's a null pointer".to_string(),
            actions: vec![],
            quality: 0.3,
            errors: vec!["NullPointerException".to_string()],
        };
        let mut predicates = vec![
            Predicate::new(
                "Error Recovery",
                "An error occurred and the system needs to recover",
                GateType::Verify,
                0.3,
            ),
        ];
        let evals = vec![("Error Recovery".to_string(), 0.9)];
        let collapse = Collapse::compute(&ins, &outs, &mut predicates, &evals);
        assert!(collapse.n > 0);
    }
}
