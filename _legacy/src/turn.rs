//! The Turn Processor — the complete nstar-bit loop.
//!
//! This wires together: predicates + LM evaluation + collapse + gates + reflection + receipts.
//!
//! One function: `process_turn(ins, outs, state) → (collapse, gate_result)`
//!
//! This is the entry point for any system using nstar-bit.

use anyhow::Result;
use std::path::Path;

use crate::collapse::{Collapse, TurnIns, TurnOuts};
use crate::gate::GateResult;
use crate::lm::{self, LmClient};
use crate::predicate::Predicate;
use crate::receipt::Receipt;
use crate::state::NstarState;

/// The result of processing a turn through the nstar-bit protocol.
#[derive(Debug)]
pub struct TurnResult {
    pub collapse: Collapse,
    pub gate_result: GateResult,
    pub new_predicate: Option<Predicate>,
    pub reinforced: Vec<String>,
    pub reasoning: String,
}

/// Process a turn through the full nstar-bit protocol.
///
/// 1. META PASS: Evaluate all predicates (via LM or heuristic)
/// 2. Compute the causal collapse
/// 3. Evaluate gates
/// 4. REFLECTION PASS: Discover new predicates (via LM)
/// 5. Write receipt
///
/// If no LM client is available, falls back to heuristic activation.
pub async fn process_turn(
    ins: &TurnIns,
    outs: &TurnOuts,
    state: &mut NstarState,
    state_path: &Path,
    receipts_path: &Path,
) -> Result<TurnResult> {
    let lm = LmClient::new().ok_or_else(|| anyhow::anyhow!("No LM available. API key is required."))?;

    // ── PASS 1: META — Evaluate predicates ──

    let eval_results = lm.evaluate_predicates(&state.predicates, ins, outs).await?;
    let eval_tuples: Vec<(String, f32)> = eval_results
        .into_iter()
        .map(|e| (e.name, e.activation))
        .collect();

    // ── PASS 2: Compute the causal collapse ──

    let mut collapse = Collapse::compute(ins, outs, &mut state.predicates, &eval_tuples);

    // ── PASS 3: Evaluate gates ──

    let gate_result = GateResult::from_fired_gates(&collapse.gates_fired);

    // ── PASS 4: REFLECTION — Discover new predicates ──

    let mut new_predicate: Option<Predicate> = None;
    let mut reinforced: Vec<String> = Vec::new();
    let reasoning;

    match lm.reflect(&state.predicates, ins, outs).await {
        Ok(reflection) => {
            reasoning = reflection.reasoning;
            reinforced = reflection.reinforced.clone();

            // Reinforce predicates the LM identified as useful
            for name in &reflection.reinforced {
                if let Some(pred) = state.predicates.iter_mut().find(|p| &p.name == name) {
                    pred.reinforce();
                }
            }

            // If the LM discovered a new predicate, add it
            if let Some(proposal) = reflection.new_predicate {
                let gate = lm::parse_gate_type(&proposal.gate_type);
                let pred = Predicate::new(
                    &proposal.name,
                    &proposal.activation_condition,
                    gate,
                    proposal.threshold,
                );

                // Record the discovery in the collapse
                collapse.discovered = Some(crate::collapse::PredicateDiscovery {
                    name: proposal.name.clone(),
                    activation_condition: proposal.activation_condition,
                    gate: lm::parse_gate_type(&proposal.gate_type),
                    reason: proposal.reason,
                });

                state.add_predicate(pred.clone());
                new_predicate = Some(pred);
            }
        }
        Err(e) => {
            reasoning = format!("Reflection failed: {}", e);
        }
    }

    // ── PASS 5: Record ──

    // Get previous hash for chain
    let prev_hash = state
        .collapses
        .last()
        .map(|c| c.hash.as_str())
        .unwrap_or("genesis");

    let receipt = Receipt::from_collapse(&collapse, &gate_result, prev_hash);
    receipt.append_to_file(receipts_path)?;

    state.add_collapse(collapse.clone());
    state.save(state_path)?;

    // ── Summary ──

    let n_preds = state.predicates.len();
    let n_turns = state.total_turns;

    println!(
        "n★ turn {} | n={} | predicates={} | gate={} | {}",
        n_turns,
        collapse.n,
        n_preds,
        gate_result.summary(),
        if let Some(ref p) = new_predicate {
            format!("NEW: {}", p.name)
        } else {
            "no new predicate".to_string()
        }
    );

    Ok(TurnResult {
        collapse,
        gate_result,
        new_predicate,
        reinforced,
        reasoning,
    })
}

/// Quick helper for testing: process a turn with default paths.
pub async fn quick_turn(prompt: &str, response: &str, quality: f32) -> Result<TurnResult> {
    let state_path = Path::new("nstar_state.json");
    let receipts_path = Path::new("receipts.jsonl");

    let mut state = NstarState::load_or_create(state_path)?;

    let ins = TurnIns {
        prompt: prompt.to_string(),
        context: vec![],
        turn: state.total_turns + 1,
    };

    let outs = TurnOuts {
        response: response.to_string(),
        actions: vec![],
        quality,
        errors: vec![],
        operations: vec![],
    };

    process_turn(&ins, &outs, &mut state, state_path, receipts_path).await
}
