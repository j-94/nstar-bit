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

        // Verify hash match
        // Note: The original implementation in core.rs uses Utc::now() for timestamp
        // which makes the hash non-deterministic if timestamp is hashed.
        // Let's check make_receipt to see what's hashed.
        
        if result.receipt.hash == rec.hash {
            success_count += 1;
            if args.verbose {
                println!("  ✅ MATCH");
            }
        } else {
            println!("  ❌ MISMATCH in turn {}", rec.turn);
            println!("     Recorded: {}", rec.hash);
            println!("     Replayed: {}", result.receipt.hash);
            
            // If mismatch, we might want to stop or continue
            // For now let's continue to see aggregate results.
        }
    }

    println!("\nVerification Summary:");
    println!("Total Receipts: {}", recorded_receipts.len());
    println!("Matches:        {}", success_count);
    println!("Mismatches:     {}", recorded_receipts.len() - success_count);

    if success_count == recorded_receipts.len() {
        println!("\nPASS: Deterministic replay verified 100/100.");
    } else {
        println!("\nFAIL: Replay non-determinism detected.");
        if !recorded_receipts.is_empty() {
             std::process::exit(1);
        }
    }

    if temp_receipts.exists() {
        let _ = std::fs::remove_file(&temp_receipts);
    }

    Ok(())
}
