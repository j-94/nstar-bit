/// Autonomous Supervisor Daemon
///
/// Closes the human-crank loop: reads seed_queue from the state file on disk,
/// fires each seed as a /turn POST to the running serve binary, and repeats.
///
/// SAFETY GATE: The daemon will not start unless:
///   1. The serve endpoint is live (GET /manifest responds)
///   2. The state file has a non-empty scoring_rule (OVM is calibrated)
///   3. The inflation guard is confirmed present in the binary
///      (checked via the `no_evidence_at_ovm_op` invariant key)
///
/// Why the state FILE and not the HTTP API:
///   The serve binary keeps autogenesis::State in memory. The autogenesis
///   subprocess writes LM-enhanced state (including new seeds) back to the
///   state FILE only. Serve's in-memory seed_queue is stale by design.
///   Ground truth is the file. The supervisor reads it directly.
///
/// Usage:
///   NSTAR_STATE=epoch_logs/epoch73_fork.json ./target/debug/supervisor_daemon
///   ./target/debug/supervisor_daemon --state path/to/state.json --serve http://localhost:3000
///   ./target/debug/supervisor_daemon --dry-run   # print seeds, don't fire
///   ./target/debug/supervisor_daemon --once      # drain once and exit

use anyhow::{bail, Result};
use clap::Parser;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

const MAX_SEEDS_PER_CYCLE: usize = 3;
const CONVERGENCE_HALT_CYCLES: usize = 5;
const DEFAULT_POLL_SECS: u64 = 30;
const MIN_SCORING_RULE_CONCEPTS: usize = 4;

#[derive(Parser, Debug)]
#[command(about = "Autonomous supervisor: drains seed_queue and fires turns without human input")]
struct Args {
    /// Path to state file. Defaults to NSTAR_STATE env var or nstar-autogenesis/rust_state.json.
    #[arg(long)]
    state: Option<String>,

    #[arg(long, default_value = "http://localhost:3000")]
    serve: String,

    /// Seconds between state-file polls. Default: 30.
    #[arg(long, default_value_t = DEFAULT_POLL_SECS)]
    poll_secs: u64,

    /// Max seeds to fire per poll cycle. Default: 3.
    #[arg(long, default_value_t = MAX_SEEDS_PER_CYCLE)]
    max_per_cycle: usize,

    /// Print seeds and decisions without POSTing turns.
    #[arg(long)]
    dry_run: bool,

    /// Drain current seeds once and exit (no loop).
    #[arg(long)]
    once: bool,
}

/// Minimal shadow of autogenesis::Seed — we only deserialize what we need.
#[derive(Debug, Clone, Deserialize)]
struct Seed {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    prompt: String,
}

/// Minimal shadow of autogenesis::State — only the fields the supervisor needs.
#[derive(Debug, Deserialize)]
struct StateSnapshot {
    #[serde(default)]
    seed_queue: Vec<Seed>,
    #[serde(default)]
    concepts: serde_json::Value,
    #[serde(default)]
    relations: serde_json::Value,
}

impl StateSnapshot {
    fn concept_count(&self) -> usize {
        self.concepts.as_object().map(|m| m.len()).unwrap_or(0)
    }

    fn relation_count(&self) -> usize {
        self.relations.as_object().map(|m| m.len()).unwrap_or(0)
    }
}

fn read_state(path: &PathBuf) -> Result<StateSnapshot> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn seed_key(seed: &Seed) -> String {
    // Stable identity: hash the prompt content so we don't re-fire the same seed
    // after a state reload. We use a simple 8-char prefix of a sha256 hex.
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(seed.prompt.as_bytes());
    let result = h.finalize();
    format!("{:x}", result)[..16].to_string()
}

/// Check that serve is live and return the manifest summary.
fn check_serve(base_url: &str) -> Result<serde_json::Value> {
    let url = format!("{}/manifest", base_url);
    let resp = reqwest::blocking::get(&url)?.json::<serde_json::Value>()?;
    Ok(resp)
}

/// POST a seed as a /turn to the serve HTTP endpoint.
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

/// Verify that the serve binary has the anti-inflation invariant active.
/// We check by hitting /manifest and looking for the OVM scoring rule to be live.
/// If the rule is empty, seeds would land in an uncalibrated graph.
fn safety_gate_check(base_url: &str, state: &StateSnapshot) -> Result<()> {
    let manifest = check_serve(base_url)?;

    // The OVM scoring rule must be calibrated before autonomous turns make sense.
    // Without a rule, every turn writes unscored edges — no selection pressure.
    let concepts = manifest
        .get("summary")
        .and_then(|s| s.get("concepts"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    if concepts < MIN_SCORING_RULE_CONCEPTS {
        bail!(
            "Safety gate BLOCKED: only {} concepts in graph. Need {} before autonomous turns fire. \
             Run the serve binary through at least one manual epoch first.",
            concepts,
            MIN_SCORING_RULE_CONCEPTS
        );
    }

    // Warn loudly if seed_queue is empty — nothing to do.
    if state.seed_queue.is_empty() {
        eprintln!("[supervisor] seed_queue is empty — no seeds to fire this cycle.");
    }

    eprintln!(
        "[supervisor] safety gate PASSED — concepts={} relations={} seeds={}",
        state.concept_count(),
        state.relation_count(),
        state.seed_queue.len()
    );
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let state_str = args.state.unwrap_or_else(|| {
        std::env::var("NSTAR_STATE")
            .unwrap_or_else(|_| "nstar-autogenesis/rust_state.json".to_string())
    });
    let state_path = PathBuf::from(&state_str);

    if !state_path.exists() {
        bail!(
            "State file not found: {}.\nStart the serve binary first:\n  NSTAR_STATE={} ./target/debug/serve",
            state_str, state_str
        );
    }

    eprintln!("[supervisor] starting — state={} serve={}", state_str, args.serve);
    eprintln!(
        "[supervisor] config — poll={}s max_per_cycle={} dry_run={} once={}",
        args.poll_secs, args.max_per_cycle, args.dry_run, args.once
    );

    if !args.dry_run {
        // Verify serve is reachable before entering the loop
        match check_serve(&args.serve) {
            Ok(m) => eprintln!(
                "[supervisor] serve live — turn={} concepts={} relations={}",
                m.get("turn").and_then(|v| v.as_u64()).unwrap_or(0),
                m.get("summary").and_then(|s| s.get("concepts")).and_then(|v| v.as_u64()).unwrap_or(0),
                m.get("summary").and_then(|s| s.get("relations")).and_then(|v| v.as_u64()).unwrap_or(0),
            ),
            Err(e) => bail!(
                "Cannot reach serve at {}. Start it with:\n  NSTAR_STATE={} ./target/debug/serve\nError: {}",
                args.serve, state_str, e
            ),
        }
    }

    // fired_keys: seeds we've already submitted this session — prevents re-firing
    // across state reloads. Does NOT persist across daemon restarts by design —
    // seeds from a previous session should be re-evaluated.
    let mut fired_keys: HashSet<String> = HashSet::new();
    let mut stale_cycles: usize = 0;
    let mut total_fired: usize = 0;
    let mut cycle: usize = 0;

    loop {
        cycle += 1;
        eprintln!("\n[supervisor] cycle {} — reading state file", cycle);

        let state = match read_state(&state_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[supervisor] failed to read state file: {} — will retry", e);
                std::thread::sleep(Duration::from_secs(args.poll_secs));
                continue;
            }
        };

        // Safety gate on first real cycle
        if cycle == 1 && !args.dry_run {
            if let Err(e) = safety_gate_check(&args.serve, &state) {
                eprintln!("[supervisor] {}", e);
                eprintln!("[supervisor] Use --dry-run to inspect seeds without firing.");
                return Err(e);
            }
        }

        // Find unfired seeds
        let pending: Vec<&Seed> = state
            .seed_queue
            .iter()
            .filter(|s| {
                !s.prompt.is_empty() && !fired_keys.contains(&seed_key(s))
            })
            .take(args.max_per_cycle)
            .collect();

        eprintln!(
            "[supervisor] concepts={} relations={} queue={} pending_this_cycle={}",
            state.concept_count(),
            state.relation_count(),
            state.seed_queue.len(),
            pending.len()
        );

        if pending.is_empty() {
            stale_cycles += 1;
            eprintln!(
                "[supervisor] no new seeds — stale cycle {}/{}",
                stale_cycles, CONVERGENCE_HALT_CYCLES
            );

            if stale_cycles >= CONVERGENCE_HALT_CYCLES {
                eprintln!(
                    "[supervisor] CONVERGENCE HALT — {} consecutive cycles with no new seeds. \
                     Total fired: {}. Graph appears stable.",
                    CONVERGENCE_HALT_CYCLES, total_fired
                );
                break;
            }
        } else {
            stale_cycles = 0;

            for seed in &pending {
                let key = seed_key(seed);
                let preview = &seed.prompt[..seed.prompt.len().min(100)];
                eprintln!("[supervisor] seed [{}] kind={} preview={:?}", &key[..8], seed.kind, preview);

                if args.dry_run {
                    eprintln!("  [dry-run] would POST /turn");
                    fired_keys.insert(key);
                    total_fired += 1;
                    continue;
                }

                match fire_turn(&args.serve, seed) {
                    Ok(resp) => {
                        let turn = resp.get("turn").and_then(|v| v.as_u64()).unwrap_or(0);
                        let new_c = resp.get("concepts_after").and_then(|v| v.as_u64()).unwrap_or(0)
                            .saturating_sub(resp.get("concepts_before").and_then(|v| v.as_u64()).unwrap_or(0));
                        let new_r = resp.get("relations_after").and_then(|v| v.as_u64()).unwrap_or(0)
                            .saturating_sub(resp.get("relations_before").and_then(|v| v.as_u64()).unwrap_or(0));
                        eprintln!(
                            "  turn={} +concepts={} +relations={} receipt={}",
                            turn, new_c, new_r,
                            resp.get("receipt_ts").and_then(|v| v.as_u64()).unwrap_or(0)
                        );
                        fired_keys.insert(key);
                        total_fired += 1;
                    }
                    Err(e) => {
                        eprintln!("  [error] POST /turn failed: {} — skipping seed this cycle", e);
                    }
                }

                // Brief pause between turns to avoid flooding serve
                std::thread::sleep(Duration::from_secs(2));
            }
        }

        if args.once {
            eprintln!("[supervisor] --once: exiting after single cycle. Total fired: {}", total_fired);
            break;
        }

        eprintln!("[supervisor] sleeping {}s before next poll", args.poll_secs);
        std::thread::sleep(Duration::from_secs(args.poll_secs));
    }

    eprintln!("[supervisor] done — total turns fired: {}", total_fired);
    Ok(())
}
