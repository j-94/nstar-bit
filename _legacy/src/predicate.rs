//! A Predicate is the nstar-bit's replacement for a fixed bit.
//!
//! In the old system: `pub struct Bits { pub a: f32, pub u: f32, ... }`
//! Fixed at 9. Each is a named f32 field. Adding a 10th requires changing the struct.
//!
//! In nstar-bit: predicates are dynamic. The system starts with zero and discovers
//! them from the pattern of its own successes and failures.
//!
//! Each predicate is a node in the graph. A metacognitive predicate ("Alignment")
//! and a domain concept ("auth.py") are the same type — interchangeable.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single predicate — one dimension of awareness.
///
/// Replaces a fixed field like `pub a: f32` in the old `ExtendedBits`.
/// Unlike a fixed field, predicates are created at runtime when the system
/// discovers a failure mode not covered by existing predicates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Predicate {
    /// Unique identifier
    pub id: String,

    /// Prime number identity for composing Ruliad coordinates
    pub prime_id: u64,

    /// Human-readable name (e.g., "Alignment", "Coupling", "Urgency")
    pub name: String,

    /// When was this predicate discovered? (turn number)
    pub discovered_at: u64,

    /// How this predicate gets activated — a natural language condition
    /// that the LLM evaluates at each turn.
    /// e.g., "The user's intent is unclear or ambiguous"
    pub activation_condition: String,

    /// What happens when this predicate is strongly active
    pub gate: GateType,

    /// Activation threshold — above this, the gate fires
    pub threshold: f32,

    /// Current activation level (0.0 to 1.0)
    pub activation: f32,

    /// How many times this predicate has been useful (reinforcement count)
    pub reinforcements: u64,

    /// Optional: if this predicate was merged from others
    pub merged_from: Vec<String>,
}

/// What action a predicate gates when it fires.
///
/// Maps to the old kernel's gate functions:
/// - `ask_act_gate` → Halt (don't act, ask first)
/// - `evidence_gate` → Verify (require evidence before trusting)
/// - New: Escalate (flag for human judgment)
/// - New: Simulate (run in latent space before materializing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateType {
    /// Stop and ask a clarifying question (old: A=0 → ask)
    Halt,
    /// Verify understanding before proceeding (old: U≥τ → verify)
    Verify,
    /// Flag for human judgment (new: not in old system)
    Escalate,
    /// Run simulation before materializing (from LSI theory)
    Simulate,
    /// No gate — informational only
    None,
}

impl Predicate {
    /// Create a new predicate discovered at a given turn.
    pub fn new(name: &str, activation_condition: &str, gate: GateType, threshold: f32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            prime_id: 2, // Default, will be updated by registry later
            name: name.to_string(),
            discovered_at: 0,
            activation_condition: activation_condition.to_string(),
            gate,
            threshold,
            activation: 0.0,
            reinforcements: 0,
            merged_from: Vec::new(),
        }
    }

    /// Does this predicate's gate fire given its current activation?
    pub fn gate_fires(&self) -> bool {
        self.activation >= self.threshold
    }

    /// Reinforce this predicate (it was useful this turn)
    pub fn reinforce(&mut self) {
        self.reinforcements += 1;
    }

    /// Set activation level for this turn
    pub fn activate(&mut self, level: f32) {
        self.activation = level.clamp(0.0, 1.0);
    }

    /// Merge another predicate into this one (they always co-activate)
    pub fn merge(&mut self, other: &Predicate) {
        self.merged_from.push(other.id.clone());
        // Take the stricter threshold
        self.threshold = self.threshold.max(other.threshold);
        // Sum reinforcements
        self.reinforcements += other.reinforcements;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicate_starts_inactive() {
        let p = Predicate::new("Uncertainty", "The approach is unclear", GateType::Verify, 0.7);
        assert!(!p.gate_fires());
        assert_eq!(p.activation, 0.0);
    }

    #[test]
    fn gate_fires_above_threshold() {
        let mut p = Predicate::new("Uncertainty", "The approach is unclear", GateType::Verify, 0.7);
        p.activate(0.8);
        assert!(p.gate_fires());
    }

    #[test]
    fn gate_does_not_fire_below_threshold() {
        let mut p = Predicate::new("Uncertainty", "The approach is unclear", GateType::Verify, 0.7);
        p.activate(0.5);
        assert!(!p.gate_fires());
    }

    #[test]
    fn reinforcement_accumulates() {
        let mut p = Predicate::new("Alignment", "Intent is clear", GateType::None, 0.5);
        p.reinforce();
        p.reinforce();
        assert_eq!(p.reinforcements, 2);
    }
}
