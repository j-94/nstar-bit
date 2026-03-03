//! Gate conditions — what happens when a predicate fires.
//!
//! Replaces the old kernel's hardcoded gates:
//! - `ask_act_gate`: A≥1 && P≥1 && Δ==0
//! - `evidence_gate`: U < τ
//!
//! In nstar-bit, gates are dynamic and attached to individual predicates.
//! New gate conditions can be proposed by the reflection pass.

use serde::{Deserialize, Serialize};

use crate::collapse::FiredGate;
use crate::predicate::GateType;

/// The result of evaluating all gates for a turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Can the system act? (false if any Halt gate fired)
    pub can_act: bool,

    /// Must the system verify first? (true if any Verify gate fired)
    pub must_verify: bool,

    /// Should the system escalate to human? (true if any Escalate gate fired)
    pub must_escalate: bool,

    /// Should the system simulate before materializing?
    pub must_simulate: bool,

    /// All gates that fired with details
    pub fired: Vec<FiredGate>,
}

impl GateResult {
    /// Evaluate all fired gates and determine the composite action.
    ///
    /// Priority order (from old kernel's logic):
    /// 1. Halt overrides everything (old: ask_act_gate failure)
    /// 2. Escalate requires human (new)
    /// 3. Simulate before materializing (from LSI theory)
    /// 4. Verify requires evidence check (old: evidence_gate)
    /// 5. If nothing fires → proceed normally
    pub fn from_fired_gates(gates: &[FiredGate]) -> Self {
        let can_act = !gates.iter().any(|g| matches!(g.gate_type, GateType::Halt));
        let must_verify = gates.iter().any(|g| matches!(g.gate_type, GateType::Verify));
        let must_escalate = gates.iter().any(|g| matches!(g.gate_type, GateType::Escalate));
        let must_simulate = gates.iter().any(|g| matches!(g.gate_type, GateType::Simulate));

        Self {
            can_act,
            must_verify,
            must_escalate,
            must_simulate,
            fired: gates.to_vec(),
        }
    }

    /// Is the system completely clear to act without any constraints?
    pub fn clear_to_act(&self) -> bool {
        self.can_act && !self.must_verify && !self.must_escalate && !self.must_simulate
    }

    /// Summary for display
    pub fn summary(&self) -> String {
        if self.clear_to_act() {
            "CLEAR: proceed".to_string()
        } else {
            let mut parts = Vec::new();
            if !self.can_act {
                parts.push("HALT");
            }
            if self.must_escalate {
                parts.push("ESCALATE");
            }
            if self.must_simulate {
                parts.push("SIMULATE");
            }
            if self.must_verify {
                parts.push("VERIFY");
            }
            parts.join(" + ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_gates_means_clear() {
        let result = GateResult::from_fired_gates(&[]);
        assert!(result.clear_to_act());
        assert_eq!(result.summary(), "CLEAR: proceed");
    }

    #[test]
    fn halt_blocks_action() {
        let gates = vec![FiredGate {
            predicate_name: "Alignment".to_string(),
            gate_type: GateType::Halt,
            activation: 0.9,
        }];
        let result = GateResult::from_fired_gates(&gates);
        assert!(!result.can_act);
        assert!(!result.clear_to_act());
    }

    #[test]
    fn verify_requires_evidence() {
        let gates = vec![FiredGate {
            predicate_name: "Uncertainty".to_string(),
            gate_type: GateType::Verify,
            activation: 0.8,
        }];
        let result = GateResult::from_fired_gates(&gates);
        assert!(result.can_act); // Can act, but must verify first
        assert!(result.must_verify);
        assert!(!result.clear_to_act());
    }
}
