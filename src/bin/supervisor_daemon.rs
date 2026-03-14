use anyhow::{bail, Result};
use clap::Parser;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use nstar_bit::state_sync::StateTransaction;
use nstar_bit::autogenesis::{State, Seed};

const MAX_SEEDS_PER_CYCLE: usize = 3;
const CONVERGENCE_HALT_CYCLES: usize = 5;
const DEFAULT_POLL_SECS: u64 = 30;
const MIN_SCORING_RULE_CONCEPTS: usize = 4;
const SPRAWL_LIMIT: usize = 50;
const GC_INTERVAL: usize = 5;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    state: Option<String>,
    #[arg(long, default_value = "http://localhost:3000")]
    serve: String,
    #[arg(long, default_value_t = DEFAULT_POLL_SECS)]
    poll_secs: u64,
    #[arg(long, default_value_t = MAX_SEEDS_PER_CYCLE)]
    max_per_cycle: usize,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    once: bool,
}

fn seed_key(seed: &Seed) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(seed.prompt.as_bytes());
    let result = h.finalize();
    format!("{:x}", result)[..16].to_string()
}

fn scan_and_report_compression(state: &State) {
    eprintln!("\n[supervisor] Scanning for compression candidates...");
    let mut candidates = 0;
    
    // Scan relations for failure signals
    // state.relations is a BTreeMap<String, RelationRecord>
    for rel in state.relations.values() {
        if rel.status != "archived" && rel.evidence_against > rel.evidence_for {
            candidates += 1;
            eprintln!(
                "  [candidate] id={} (for={}, against={})",
                rel.id, rel.evidence_for, rel.evidence_against
            );
        }
    }
    
    if candidates == 0 {
        eprintln!("  [status] No compression candidates found.");
    } else {
        eprintln!("  [status] Found {} candidates ready for archival.", candidates);
    }
}

fn fire_turn(base_url: &str, seed: &Seed) -> Result<serde_json::Value> {
    let client = reqwest::blocking::Client::new();
    let payload = serde_json::json!({
        "text": seed.prompt,
        "domain": if seed.kind.is_empty() { "signal" } else { &seed.kind },
        "source": "supervisor",
    });
    let resp = client
        .post(format!("{}/turn", base_url))
        .json(&payload)
        .timeout(Duration::from_secs(60))
        .send()?
        .json::<serde_json::Value>()?;
    Ok(resp)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let state_str = args.state.clone().unwrap_or_else(|| {
        std::env::var("NSTAR_STATE")
            .unwrap_or_else(|_| "nstar-autogenesis/rust_state.json".to_string())
    });
    let state_path = PathBuf::from(&state_str);

    let mut fired_keys: HashSet<String> = HashSet::new();
    let mut cycle: usize = 0;

    loop {
        cycle += 1;
        eprintln!("\n[supervisor] cycle {} — reading state file", cycle);

        // Transactional mutation
        let mut tx = StateTransaction::begin(&state_path)?;
        let mut pending = Vec::new();
        
        // Remove seeds so they aren't processed twice
        tx.state.seed_queue.retain(|s| {
            if !s.prompt.is_empty() && !fired_keys.contains(&seed_key(s)) && pending.len() < args.max_per_cycle {
                pending.push(s.clone());
                fired_keys.insert(seed_key(s));
                false
            } else {
                true
            }
        });
        tx.commit()?; 

        if pending.is_empty() {
             eprintln!("[supervisor] no pending seeds");
        } else {
            for seed in &pending {
                match fire_turn(&args.serve, seed) {
                    Ok(_) => eprintln!("[supervisor] fired seed"),
                    Err(e) => eprintln!("[error] turn failed: {}", e),
                }
            }
        }

        if args.once { break; }
        std::thread::sleep(Duration::from_secs(args.poll_secs));
    }
    Ok(())
}
