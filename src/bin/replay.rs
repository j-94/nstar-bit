use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;

use nstar_bit::canonical::core::CanonicalCore;
use nstar_bit::canonical::types::CanonicalReceipt;
use nstar_bit::utir_exec::GuardConfig;

#[derive(Parser, Debug)]
#[command(author, version, about = "N* Canonical Deterministic Replay Verifier")]
struct Args {
    #[arg(long, default_value = "canonical_receipts.jsonl")]
    receipts_file: String,

    #[arg(long, default_value = "nstar_canonical_state.json")]
    state_file: String,

    #[arg(long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    let receipts_path = PathBuf::from(&args.receipts_file);
    let state_path = PathBuf::from(&args.state_file);
    if !receipts_path.exists() {
        return Err(anyhow!("Receipts file does not exist: {}", args.receipts_file));
    }

    let file = File::open(&receipts_path)?;
    let reader = BufReader::new(file);

    let mut recorded_receipts = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let receipt: CanonicalReceipt = serde_json::from_str(&line)?;
        recorded_receipts.push(receipt);
    }

    println!("Loaded {} receipts for deterministic replay verification.", recorded_receipts.len());

    let mut core = CanonicalCore::new();

    // We use a temporary receipts file for the replay turns
    let temp_receipts = std::env::temp_dir().join("nstar_replay_receipts.jsonl");
    if temp_receipts.exists() {
        std::fs::remove_file(&temp_receipts)?;
    }

    let guard = GuardConfig::from_env();
    let mut success_count = 0;
    // Track which turn numbers were verified so we can mark them in the source file.
    let mut verified_turns = std::collections::HashSet::<u64>::new();
    let mut verified_receipts = std::collections::HashSet::<(u64, String)>::new();

    for rec in recorded_receipts.iter() {
        if args.verbose {
            println!("REPLAY TURN {}: hash={}", rec.turn, rec.hash);
        }

        // Inject the criteria that existed BEFORE this turn
        core.state.graph.criteria = rec.criteria_before.clone();

        // Process the exact same inputs
        let result = core.process_turn(
            rec.recorded_input.clone(),
            rec.recorded_proposal.clone(),
            rec.recorded_observations.clone(),
            rec.recorded_discoveries.clone(),
            &guard,
            &temp_receipts,
        )?;

        if result.receipt.hash == rec.hash {
            success_count += 1;
            verified_turns.insert(rec.turn);
            verified_receipts.insert((rec.turn, rec.hash.clone()));
            if args.verbose {
                println!("  ✅ MATCH turn={}", rec.turn);
            }
        } else {
            println!("  ❌ MISMATCH in turn {}", rec.turn);
            println!("     Recorded: {}", rec.hash);
            println!("     Replayed: {}", result.receipt.hash);
        }
    }

    let total = recorded_receipts.len();
    println!("\nVerification Summary:");
    println!("Total Receipts: {}", total);
    println!("Matches:        {}", success_count);
    println!("Mismatches:     {}", total - success_count);

    // Write back deterministic: true for all verified receipts.
    if !verified_turns.is_empty() {
        let mut updated_lines = Vec::new();
        for mut rec in recorded_receipts.drain(..) {
            if verified_turns.contains(&rec.turn) {
                rec.deterministic = true;
            }
            updated_lines.push(serde_json::to_string(&rec)?);
        }
        std::fs::write(&receipts_path, updated_lines.join("\n") + "\n")?;
        println!("Wrote deterministic=true for {} verified receipt(s).", verified_turns.len());
    }

    if !verified_receipts.is_empty() {
        for rec in &mut core.state.receipts {
            if verified_receipts.contains(&(rec.turn, rec.hash.clone())) {
                rec.deterministic = true;
            }
        }

        if state_path.exists() {
            let mut persisted = CanonicalCore::load_or_create(&state_path)?;
            let mut updated = 0usize;
            for rec in &mut persisted.state.receipts {
                if verified_receipts.contains(&(rec.turn, rec.hash.clone())) && !rec.deterministic {
                    rec.deterministic = true;
                    updated += 1;
                }
            }

            if persisted.state.receipts.is_empty() && !core.state.receipts.is_empty() {
                core.save(&state_path)?;
                println!(
                    "Wrote replay-verified receipts into state file {}.",
                    state_path.display()
                );
            } else {
                persisted.save(&state_path)?;
                println!(
                    "Updated deterministic flags for {} receipt(s) in state file {}.",
                    updated,
                    state_path.display()
                );
            }
        } else {
            core.save(&state_path)?;
            println!(
                "Created state file {} from replayed canonical state.",
                state_path.display()
            );
        }
    }

    if success_count == total {
        println!("\nPASS: Deterministic replay verified {0}/{0}.", success_count);
    } else {
        println!("\nFAIL: Replay non-determinism detected.");
        if total > 0 {
            std::process::exit(1);
        }
    }

    if temp_receipts.exists() {
        let _ = std::fs::remove_file(&temp_receipts);
    }

    Ok(())
}
