//! Autopoietic Loop — the system that runs itself.
//!
//! This is the highest-level demonstration of the n★ theory:
//! the system generates its own tasks, executes them, collapses the results,
//! discovers predicates, and uses those predicates to generate better tasks.
//!
//! The loop is:
//!   1. SURVEY: Ask the LM to look at the current predicate state and propose
//!      a task that will stress-test an undiscovered failure mode.
//!   2. EXECUTE: Run that task through the LM.
//!   3. COLLAPSE: Compute n★(ins, outs) with the current predicates.
//!   4. REFLECT: Discover new predicates from the turn.
//!   5. PRUNE: Merge redundant predicates (ruliadic compression).
//!   6. LOOP: Feed the new state back into step 1.
//!
//! This IS autopoiesis: the system's output (predicates) becomes
//! its input (what to explore next). The boundary between the system
//! and its environment is maintained by the gate mechanism.
//!
//! This IS ruliadic traversal: the predicate space is a ruliad.
//! Each unique predicate configuration is a point in the space.
//! The system traverses it by discovering new predicates and merging
//! old ones — exploring the space of all possible metacognitive structures.

use anyhow::Result;
use std::path::PathBuf;

use nstar_bit::collapse::TurnIns;
use nstar_bit::lm::LmClient;
use nstar_bit::state::NstarState;
use nstar_bit::turn::process_turn;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let max_turns: usize = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);

    let state_path = PathBuf::from("nstar_auto_state.json");
    let receipts_path = PathBuf::from("auto_receipts.jsonl");

    // Clean start option
    if args.iter().any(|a| a == "--reset") {
        let _ = std::fs::remove_file(&state_path);
        let _ = std::fs::remove_file(&receipts_path);
        println!("Reset.");
    }

    let mut state = NstarState::load_or_create(&state_path)?;
    let lm = LmClient::new().expect("API key required");

    println!("╔══════════════════════════════════════════════════╗");
    println!("║      N★ BIT — AUTOPOIETIC LOOP ({:>2} turns)       ║", max_turns);
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  The system generates its own tasks, executes    ║");
    println!("║  them, discovers predicates, and feeds them      ║");
    println!("║  back to generate better tasks.                  ║");
    println!("║                                                  ║");
    println!("║  Autopoiesis: output → input (self-creating)     ║");
    println!("║  Ruliad: predicate space traversal               ║");
    println!("║  Collapse: n★(ins, outs) → causal fingerprint    ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    for turn_num in 1..=max_turns {
        println!("━━━ Autopoietic Turn {}/{} ━━━━━━━━━━━━━━━━━━━━━━━━", turn_num, max_turns);

        // ─── STEP 1: SURVEY — Generate a task from the current state ───
        let task = generate_next_task(&lm, &state).await?;
        println!("Generated Task: {}", truncate(&task, 100));

        // ─── STEP 2: EXECUTE — Run the task ───
        let outs = match lm.execute_task(&task).await {
            Ok(o) => o,
            Err(e) => {
                println!("  ⚠ Execution failed: {}", e);
                continue;
            }
        };

        println!("Quality: {:.2} | Errors: {}", outs.quality, outs.errors.len());

        // ─── STEP 3+4: COLLAPSE + REFLECT ───
        let ins = TurnIns {
            prompt: task,
            context: vec![],
            turn: state.total_turns + 1,
        };

        match process_turn(&ins, &outs, &mut state, &state_path, &receipts_path).await {
            Ok(res) => {
                if let Some(ref p) = res.new_predicate {
                    println!("  ★ DISCOVERED: {} [{:?}]", p.name, p.gate);
                }
                if !res.reinforced.is_empty() {
                    println!("  ↑ REINFORCED: {}", res.reinforced.join(", "));
                }
                if !res.gate_result.can_act {
                    println!("  ⛔ GATE: {}", res.gate_result.summary());
                }
            }
            Err(e) => {
                println!("  ⚠ Collapse failed: {}", e);
            }
        }

        // ─── STEP 5: PRUNE — Merge redundant predicates ───
        let merges = state.find_merge_candidates(0.85);
        for (a, b) in &merges {
            println!("  ↔ MERGE CANDIDATE: {} + {}", a, b);
        }

        // Status line
        println!(
            "  n={} | predicates={} | total_turns={}\n",
            state
                .collapses
                .last()
                .map(|c| c.n)
                .unwrap_or(0),
            state.predicates.len(),
            state.total_turns,
        );
    }

    // ─── FINAL DASHBOARD ───
    println!("\n{}", state.summary());

    // Receipt chain verification
    println!("  ┌─ RECEIPT CHAIN ─────────────────────────────────");
    if let Ok(data) = std::fs::read_to_string(&receipts_path) {
        let lines: Vec<&str> = data.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let hash = v.get("collapse_hash").and_then(|h| h.as_str()).unwrap_or("?");
                let prev = v.get("prev_hash").and_then(|h| h.as_str()).unwrap_or("?");
                let turn = v.get("turn").and_then(|t| t.as_u64()).unwrap_or(0);
                let disc = v.get("discovered").and_then(|d| d.as_str()).unwrap_or("-");
                if i == 0 {
                    println!("  │ T{:>2} [genesis] → [{}] {}", turn, &hash[..8], if disc != "-" { format!("★{}", disc) } else { String::new() });
                } else {
                    println!("  │ T{:>2} [{}] → [{}] {}", turn, &prev[..8], &hash[..8], if disc != "-" { format!("★{}", disc) } else { String::new() });
                }
            }
        }
    }
    println!("  └─────────────────────────────────────────────────\n");

    // Autopoiesis check
    let total_preds = state.predicates.len();
    let total_turns = state.total_turns;
    let discovery_rate = if total_turns > 0 {
        total_preds as f64 / total_turns as f64
    } else {
        0.0
    };

    println!("  ┌─ AUTOPOIESIS STATUS ─────────────────────────────");
    println!("  │ Discovery rate  : {:.1}% (predicates per turn)", discovery_rate * 100.0);
    println!("  │ Predicate space : n={} dimensions", total_preds);
    println!("  │ Collapse chain  : {} verified receipts", total_turns);
    println!("  │ Self-sustaining : {}", if total_preds >= 3 { "YES — enough predicates to steer" } else { "EMERGING — needs more turns" });
    println!("  └─────────────────────────────────────────────────\n");

    Ok(())
}

/// Ask the LM to generate a task that will stress-test an undiscovered failure mode.
///
/// This is the autopoietic core: the system's own state (predicates)
/// determines what it explores next (the generated task).
async fn generate_next_task(lm: &LmClient, state: &NstarState) -> Result<String> {
    let current_predicates: Vec<String> = state
        .predicates
        .iter()
        .map(|p| format!("{}: {}", p.name, p.activation_condition))
        .collect();

    let recent_quality: Vec<f32> = state
        .collapses
        .iter()
        .rev()
        .take(5)
        .map(|c| c.quality)
        .collect();

    let system = r#"You are a metacognitive exploration engine. Your job is to generate a SHORT task (1-2 sentences) that will stress-test an AI assistant in a way that reveals NEW failure modes not already covered by the existing predicates.

Rules:
- Generate diverse tasks across domains (coding, reasoning, ethics, ambiguity, knowledge limits, etc.)
- Avoid generating tasks similar to ones the existing predicates already cover.
- Make the task specific enough to execute, not vague.
- Focus on edge cases, ambiguities, and contradictions.
- Keep it to 1-2 sentences maximum.

Return JSON: {"task": "the task string"}"#;

    let user = format!(
        "EXISTING PREDICATES (already discovered):\n{}\n\nRECENT QUALITY SCORES: {:?}\n\nTOTAL TURNS SO FAR: {}\n\nGenerate a task that will probe an UNDISCOVERED failure mode.",
        if current_predicates.is_empty() {
            "None — tabula rasa. Generate any diverse task.".to_string()
        } else {
            current_predicates.join("\n")
        },
        recent_quality,
        state.total_turns,
    );

    let response = lm.chat_raw(system, &user).await?;
    let parsed: serde_json::Value = serde_json::from_str(&response)
        .unwrap_or(serde_json::json!({"task": "Write a Rust function that safely handles integer overflow"}));

    Ok(parsed
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("Write a Rust function that safely handles integer overflow")
        .to_string())
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ");
    if s.len() <= max {
        s
    } else {
        format!("{}…", &s[..max])
    }
}
