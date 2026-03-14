use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use nstar_bit::reduction::{append_receipt, ReducibleMapEngine};
use nstar_bit::state_sync::StateTransaction;
use std::env;
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "nstar",
    version = "0.1.0",
    about = "N★ Unified Executable Provenance Engine",
    long_about = "A unified multi-CLI over the N★ filesystem artifacts, network endpoints, and git history."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage the autogenesis baseline state
    State(StateCmd),
    /// Submit a turn signal to the LM extractor
    Turn(TurnCmd),
    /// Manage forks, proposals, and view epoch artifacts
    Epoch(EpochCmd),
    /// Run the canonical graph-first, simulation-first core
    Canonical(CanonicalCmd),
    /// Run the deterministic replay verifier
    Replay(ReplayCmd),
    /// Manage the headless Epistemic HTTP API
    Api(ApiCmd),
    /// Manage the continuous background supervisor daemon
    Supervisor(SupervisorCmd),
    /// Inspect JSON state schemas and graph structure artifacts
    Artifacts(ArtifactsCmd),
    /// Causal provenance tracking via Git
    Git(GitCmd),
    /// Synthesize history to generate timelines and narratives
    Archaeology(ArchaeologyCmd),
    /// Find and collapse redundant structure in the epistemic graph
    Reduce(ReduceCmd),
}

// ── state ──────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct StateCmd {
    #[command(subcommand)]
    pub action: StateAction,
}

#[derive(Subcommand)]
pub enum StateAction {
    /// Initialize the autogenesis state
    Init {
        #[arg(long)]
        state: Option<String>,
    },
    /// Print the current accumulated state summary
    Show {
        #[arg(long)]
        state: Option<String>,
    },
    /// Run health invariant checks
    Health {
        #[arg(long)]
        state: Option<String>,
    },
    /// Monitor the state
    Monitor {
        #[arg(long)]
        state: Option<String>,
    },
    /// Ingest evidence payloads from a directory
    Ingest {
        #[arg(long)]
        state: Option<String>,
    },
}

// ── turn ───────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct TurnCmd {
    /// The signal text to process
    pub text: String,
    #[arg(long)]
    pub state: Option<String>,
}

// ── epoch ──────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct EpochCmd {
    #[command(subcommand)]
    pub action: EpochAction,
}

#[derive(Subcommand)]
pub enum EpochAction {
    /// Ask LM to author a fork proposal
    Propose {
        #[arg(long)]
        state: Option<String>,
        /// Output path for the forked state (required when --execute is set)
        #[arg(long, default_value = "")]
        fork_output: String,
        /// Optional competing state to include in LM reasoning
        #[arg(long, default_value = "")]
        other_state: String,
        /// Immediately execute fork (and compare/adopt if applicable)
        #[arg(long)]
        execute: bool,
    },
    /// Compare the baseline state against a fork
    Compare {
        #[arg(long)]
        state: Option<String>,
        /// Fork state file to compare against
        #[arg(long)]
        other_state: String,
        #[arg(long, default_value = "")]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Adopt a proposed fork into baseline
    Adopt {
        #[arg(long)]
        state: Option<String>,
        #[arg(long, default_value = "")]
        reason: String,
    },
    /// Ask LM to generate a task list for the current fork's experiment
    Plan {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        proposal: String,
        #[arg(long, default_value = "")]
        output: String,
    },
    /// List epoch fork snapshots in epoch_logs/
    List,
    /// Show summary of a specific epoch fork file
    Show {
        /// Epoch number, or a path to a JSON file
        id: String,
    },
    /// Diff two epoch fork summaries
    Diff { id1: String, id2: String },
    /// Show the epoch comparison portfolio
    Portfolio,
}

// ── canonical ──────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct CanonicalCmd {
    #[command(subcommand)]
    pub action: CanonicalAction,
}

#[derive(Subcommand)]
pub enum CanonicalAction {
    /// Run a single canonical turn
    Run {
        prompt: String,
        #[arg(long)]
        state_file: Option<String>,
        #[arg(long)]
        receipts_file: Option<String>,
    },
    /// Start interactive REPL mode
    Interactive {
        #[arg(long)]
        state_file: Option<String>,
        #[arg(long)]
        receipts_file: Option<String>,
    },
    /// Print canonical state
    State {
        #[arg(long)]
        state_file: Option<String>,
    },
    /// Reset canonical state and receipts
    Reset {
        #[arg(long)]
        state_file: Option<String>,
        #[arg(long)]
        receipts_file: Option<String>,
    },
}

// ── replay ─────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct ReplayCmd {
    #[arg(long)]
    pub receipts_file: Option<String>,
    #[arg(long)]
    pub state_file: Option<String>,
    #[arg(long)]
    pub verbose: bool,
}

// ── api ────────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct ApiCmd {
    #[command(subcommand)]
    pub action: ApiAction,
}

#[derive(Subcommand)]
pub enum ApiAction {
    /// Start headless HTTP API (port 3000, state via NSTAR_STATE env)
    Serve {
        /// State file path — passed as NSTAR_STATE env var to the server
        #[arg(long)]
        state: Option<String>,
    },
    /// Show the live HTTP routes
    Routes,
}

// ── supervisor ─────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct SupervisorCmd {
    #[command(subcommand)]
    pub action: SupervisorAction,
}

#[derive(Subcommand)]
pub enum SupervisorAction {
    /// Start polling daemon for continuous loop
    Run {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        serve: Option<String>,
        #[arg(long)]
        poll_secs: Option<u64>,
        #[arg(long)]
        max_per_cycle: Option<usize>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        once: bool,
    },
}

// ── artifacts ──────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct ArtifactsCmd {
    #[command(subcommand)]
    pub action: ArtifactsAction,
}

#[derive(Subcommand)]
pub enum ArtifactsAction {
    /// List known JSON artifact files and their owning binary
    List,
    /// Display top-level key structure of a JSON file
    Schema { file: String },
    /// Extract graph metrics (concepts, relations) from a state JSON file
    Graph { file: String },
}

// ── git ────────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct GitCmd {
    #[command(subcommand)]
    pub action: GitAction,
}

#[derive(Subcommand)]
pub enum GitAction {
    /// Search git history for commits that introduced a concept or term
    Concept { name: String },
    /// Show the first commit where a concept was introduced
    Pivot { concept: String },
    /// Search commits that touched a specific term in a specific file
    Blame { file: String, term: String },
}

// ── archaeology ────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct ArchaeologyCmd {
    #[command(subcommand)]
    pub action: ArchaeologyAction,
}

#[derive(Subcommand)]
pub enum ArchaeologyAction {
    /// Generate evolutionary timeline from Git + epoch portfolio
    Timeline,
    /// Diff concept/relation counts between two epoch fork files
    Compare { epoch_a: String, epoch_b: String },
    /// Print the latest epoch thread brief
    Narrate,
}

// ── reduce ─────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct ReduceCmd {
    #[command(subcommand)]
    pub action: ReduceAction,
}

#[derive(Subcommand)]
pub enum ReduceAction {
    /// Scan graph for reduction candidates without mutating state
    Scan {
        #[arg(long)]
        state: String,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Execute a single reduction by candidate index from scan output
    Run {
        #[arg(long)]
        state: String,
        /// Index into scan candidate list (0-based)
        #[arg(long, default_value = "0")]
        index: usize,
        /// Receipts output file (appended)
        #[arg(long, default_value = "reduction_receipts.jsonl")]
        receipts: String,
        /// Confidence delta abort threshold (default 0.05)
        #[arg(long, default_value_t = 0.05)]
        threshold: f64,
    },
    /// Dry-run all scan candidates and print receipts (no state mutation)
    Dry {
        #[arg(long)]
        state: String,
        #[arg(long, default_value_t = 0.05)]
        threshold: f64,
    },
    /// Run all safe candidates and write receipts
    All {
        #[arg(long)]
        state: String,
        #[arg(long, default_value = "reduction_receipts.jsonl")]
        receipts: String,
        #[arg(long, default_value_t = 0.05)]
        threshold: f64,
        /// Cap on number of reductions per invocation
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

// ── shell-out helper ───────────────────────────────────────────────────────

fn run_binary(bin: &str, args: &[String]) -> Result<()> {
    let mut exe_path = env::current_exe()?;
    exe_path.pop();
    let bin_name = if cfg!(windows) {
        format!("{}.exe", bin)
    } else {
        bin.to_string()
    };
    exe_path.push(&bin_name);

    let mut cmd = if exe_path.exists() {
        Command::new(exe_path)
    } else {
        let mut c = Command::new("cargo");
        c.args(["run", "--bin", bin, "--"]);
        c
    };

    cmd.args(args);
    let status = cmd.status().with_context(|| format!("failed to execute {}", bin))?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

// ── read-only helpers ──────────────────────────────────────────────────────

fn epoch_path(id: &str) -> std::path::PathBuf {
    // Accept either a bare number ("9") or a full path
    if id.ends_with(".json") || id.contains('/') {
        std::path::PathBuf::from(id)
    } else {
        std::path::PathBuf::from(format!("epoch_logs/epoch{}_fork.json", id))
    }
}

fn load_json(path: &std::path::Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("invalid JSON in {}", path.display()))
}

fn print_state_summary(label: &str, v: &serde_json::Value) {
    let turn = v.get("turn").and_then(|x| x.as_u64()).unwrap_or(0);
    let concepts = v
        .get("concepts")
        .and_then(|x| x.as_object())
        .map(|m| m.len())
        .unwrap_or(0);
    let relations = v
        .get("relations")
        .and_then(|x| x.as_object())
        .map(|m| m.len())
        .unwrap_or(0);
    let version = v
        .get("version")
        .and_then(|x| x.as_str())
        .unwrap_or("—");
    println!(
        "{}: version={} turn={} concepts={} relations={}",
        label, version, turn, concepts, relations
    );
}

fn run_git(args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .context("git not found")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("git error: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

// ── main ───────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // ── state ──────────────────────────────────────────────────────────
        Commands::State(cmd) => {
            let mut args: Vec<String> = Vec::new();
            let (subcmd, state) = match cmd.action {
                StateAction::Init { state } => ("init", state),
                StateAction::Show { state } => ("show", state),
                StateAction::Health { state } => ("health", state),
                StateAction::Monitor { state } => ("monitor", state),
                StateAction::Ingest { state } => ("ingest", state),
            };
            if let Some(s) = state {
                args.extend(["--state".into(), s]);
            }
            args.push(subcmd.into());
            run_binary("autogenesis", &args)?;
        }

        // ── turn ───────────────────────────────────────────────────────────
        Commands::Turn(cmd) => {
            let mut args: Vec<String> = Vec::new();
            if let Some(s) = cmd.state {
                args.extend(["--state".into(), s]);
            }
            args.push("turn".into());
            args.push(cmd.text);
            run_binary("autogenesis", &args)?;
        }

        // ── epoch ──────────────────────────────────────────────────────────
        Commands::Epoch(cmd) => match cmd.action {
            EpochAction::Propose {
                state,
                fork_output,
                other_state,
                execute,
            } => {
                let mut args: Vec<String> = Vec::new();
                if let Some(s) = state {
                    args.extend(["--state".into(), s]);
                }
                args.push("propose".into());
                if !fork_output.is_empty() {
                    args.extend(["--fork-output".into(), fork_output]);
                }
                if !other_state.is_empty() {
                    args.extend(["--other-state".into(), other_state]);
                }
                if execute {
                    args.push("--execute".into());
                }
                run_binary("autogenesis", &args)?;
            }
            EpochAction::Compare {
                state,
                other_state,
                reason,
                json,
            } => {
                let mut args: Vec<String> = Vec::new();
                if let Some(s) = state {
                    args.extend(["--state".into(), s]);
                }
                args.push("compare".into());
                args.extend(["--other-state".into(), other_state]);
                if !reason.is_empty() {
                    args.extend(["--reason".into(), reason]);
                }
                if json {
                    args.push("--json".into());
                }
                run_binary("autogenesis", &args)?;
            }
            EpochAction::Adopt { state, reason } => {
                let mut args: Vec<String> = Vec::new();
                if let Some(s) = state {
                    args.extend(["--state".into(), s]);
                }
                args.push("adopt".into());
                if !reason.is_empty() {
                    args.extend(["--reason".into(), reason]);
                }
                run_binary("autogenesis", &args)?;
            }
            EpochAction::Plan {
                state,
                proposal,
                output,
            } => {
                let mut args: Vec<String> = Vec::new();
                if let Some(s) = state {
                    args.extend(["--state".into(), s]);
                }
                args.push("plan".into());
                args.extend(["--proposal".into(), proposal]);
                if !output.is_empty() {
                    args.extend(["--output".into(), output]);
                }
                run_binary("autogenesis", &args)?;
            }

            EpochAction::List => {
                let mut paths: Vec<_> = std::fs::read_dir("epoch_logs")
                    .context("cannot read epoch_logs/")?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("epoch") && n.ends_with("_fork.json"))
                            .unwrap_or(false)
                    })
                    .collect();
                paths.sort();
                println!("{} epoch fork snapshots:", paths.len());
                for p in &paths {
                    let fname = p.file_name().unwrap().to_str().unwrap();
                    if let Ok(v) = load_json(p) {
                        let turn = v.get("turn").and_then(|x| x.as_u64()).unwrap_or(0);
                        let concepts = v
                            .get("concepts")
                            .and_then(|x| x.as_object())
                            .map(|m| m.len())
                            .unwrap_or(0);
                        println!("  {}  turn={} concepts={}", fname, turn, concepts);
                    } else {
                        println!("  {}", fname);
                    }
                }
            }

            EpochAction::Show { id } => {
                let path = epoch_path(&id);
                let v = load_json(&path)?;
                print_state_summary(&path.display().to_string(), &v);
                // Print a few extra fields if present
                if let Some(summary) = v.get("latest_summary").and_then(|x| x.as_str()) {
                    println!("latest_summary: {}", summary);
                }
                if let Some(gate) = v.get("latest_gate") {
                    println!("latest_gate: {}", gate);
                }
            }

            EpochAction::Diff { id1, id2 } => {
                let p1 = epoch_path(&id1);
                let p2 = epoch_path(&id2);
                let v1 = load_json(&p1)?;
                let v2 = load_json(&p2)?;
                print_state_summary(&format!("A ({})", id1), &v1);
                print_state_summary(&format!("B ({})", id2), &v2);

                let concepts_a = v1
                    .get("concepts")
                    .and_then(|x| x.as_object())
                    .map(|m| m.len())
                    .unwrap_or(0) as i64;
                let concepts_b = v2
                    .get("concepts")
                    .and_then(|x| x.as_object())
                    .map(|m| m.len())
                    .unwrap_or(0) as i64;
                let relations_a = v1
                    .get("relations")
                    .and_then(|x| x.as_object())
                    .map(|m| m.len())
                    .unwrap_or(0) as i64;
                let relations_b = v2
                    .get("relations")
                    .and_then(|x| x.as_object())
                    .map(|m| m.len())
                    .unwrap_or(0) as i64;

                println!(
                    "delta: concepts {:+} relations {:+}",
                    concepts_b - concepts_a,
                    relations_b - relations_a
                );
            }

            EpochAction::Portfolio => {
                let path = std::path::Path::new("epoch_logs/portfolio.json");
                let raw = std::fs::read_to_string(path).context("epoch_logs/portfolio.json not found")?;
                let entries: Vec<serde_json::Value> = serde_json::from_str(&raw)?;
                println!("{} portfolio entries:", entries.len());
                for e in &entries {
                    let epoch = e.get("epoch").and_then(|x| x.as_u64()).unwrap_or(0);
                    let adopted = e.get("adopted").and_then(|x| x.as_bool()).unwrap_or(false);
                    let cd = e.get("concept_delta").and_then(|x| x.as_i64()).unwrap_or(0);
                    let rd = e.get("relation_delta").and_then(|x| x.as_i64()).unwrap_or(0);
                    let ts = e.get("ts").and_then(|x| x.as_str()).unwrap_or("?");
                    println!(
                        "  epoch={} adopted={} concepts{:+} relations{:+} ts={}",
                        epoch, adopted, cd, rd, ts
                    );
                }
            }
        },

        // ── canonical ──────────────────────────────────────────────────────
        Commands::Canonical(cmd) => {
            let mut args: Vec<String> = Vec::new();
            match cmd.action {
                CanonicalAction::Run {
                    prompt,
                    state_file,
                    receipts_file,
                } => {
                    if let Some(s) = state_file {
                        args.extend(["--state-file".into(), s]);
                    }
                    if let Some(r) = receipts_file {
                        args.extend(["--receipts-file".into(), r]);
                    }
                    args.push(prompt);
                }
                CanonicalAction::Interactive {
                    state_file,
                    receipts_file,
                } => {
                    if let Some(s) = state_file {
                        args.extend(["--state-file".into(), s]);
                    }
                    if let Some(r) = receipts_file {
                        args.extend(["--receipts-file".into(), r]);
                    }
                    args.push("--interactive".into());
                }
                CanonicalAction::State { state_file } => {
                    if let Some(s) = state_file {
                        args.extend(["--state-file".into(), s]);
                    }
                    args.push("--state".into());
                }
                CanonicalAction::Reset {
                    state_file,
                    receipts_file,
                } => {
                    if let Some(s) = state_file {
                        args.extend(["--state-file".into(), s]);
                    }
                    if let Some(r) = receipts_file {
                        args.extend(["--receipts-file".into(), r]);
                    }
                    args.push("--reset".into());
                }
            }
            run_binary("canonical", &args)?;
        }

        // ── replay ─────────────────────────────────────────────────────────
        Commands::Replay(cmd) => {
            let mut args: Vec<String> = Vec::new();
            if let Some(s) = cmd.state_file {
                args.extend(["--state-file".into(), s]);
            }
            if let Some(r) = cmd.receipts_file {
                args.extend(["--receipts-file".into(), r]);
            }
            if cmd.verbose {
                args.push("--verbose".into());
            }
            run_binary("replay", &args)?;
        }

        // ── api ────────────────────────────────────────────────────────────
        Commands::Api(cmd) => match cmd.action {
            ApiAction::Serve { state } => {
                let mut cmd = if {
                    let mut exe_path = env::current_exe()?;
                    exe_path.pop();
                    exe_path.push("serve");
                    exe_path.exists()
                } {
                    let mut exe_path = env::current_exe()?;
                    exe_path.pop();
                    exe_path.push("serve");
                    Command::new(exe_path)
                } else {
                    let mut c = Command::new("cargo");
                    c.args(["run", "--bin", "serve", "--"]);
                    c
                };
                if let Some(s) = state {
                    cmd.env("NSTAR_STATE", s);
                }
                let status = cmd.status().context("failed to execute serve")?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
            ApiAction::Routes => {
                println!("GET  /belief    — query a belief by source/target/relation");
                println!("POST /evidence  — inject an evidence delta");
                println!("POST /turn      — submit a turn signal (LM path)");
                println!("POST /synthesize — synthesize a concept");
                println!("GET  /manifest  — list manifest dispatch edges");
                println!("GET  /ws        — WebSocket live event stream");
            }
        },

        // ── supervisor ─────────────────────────────────────────────────────
        Commands::Supervisor(cmd) => {
            let mut args: Vec<String> = Vec::new();
            match cmd.action {
                SupervisorAction::Run {
                    state,
                    serve,
                    poll_secs,
                    max_per_cycle,
                    dry_run,
                    once,
                } => {
                    if let Some(s) = state {
                        args.extend(["--state".into(), s]);
                    }
                    if let Some(s) = serve {
                        args.extend(["--serve".into(), s]);
                    }
                    if let Some(p) = poll_secs {
                        args.extend(["--poll-secs".into(), p.to_string()]);
                    }
                    if let Some(m) = max_per_cycle {
                        args.extend(["--max-per-cycle".into(), m.to_string()]);
                    }
                    if dry_run {
                        args.push("--dry-run".into());
                    }
                    if once {
                        args.push("--once".into());
                    }
                }
            }
            run_binary("supervisor_daemon", &args)?;
        }

        // ── artifacts ──────────────────────────────────────────────────────
        Commands::Artifacts(cmd) => match cmd.action {
            ArtifactsAction::List => {
                let known: &[(&str, &str)] = &[
                    ("nstar-autogenesis/rust_state.json", "autogenesis (default)"),
                    ("baseline_state.json", "m1 epoch 1 baseline"),
                    ("lm_state.json", "m1 epoch 1 lm"),
                    ("baseline_e2_state.json", "m1 epoch 2 baseline"),
                    ("lm_e2_state.json", "m1 epoch 2 lm"),
                    ("baseline_metrics.json", "m1 epoch 1 metrics"),
                    ("lm_metrics.json", "m1 epoch 1 metrics"),
                    ("baseline_e2_metrics.json", "m1 epoch 2 metrics"),
                    ("lm_e2_metrics.json", "m1 epoch 2 metrics"),
                    ("nstar_canonical_state.json", "canonical / replay"),
                    ("canonical_receipts.jsonl", "canonical / replay"),
                    ("epoch_logs/portfolio.json", "autogenesis epoch receipts"),
                    ("epoch_logs/policy.json", "governance thresholds"),
                    ("epoch_logs/thread.md", "epoch brief log"),
                ];
                for (path, owner) in known {
                    let exists = std::path::Path::new(path).exists();
                    let mark = if exists { "✓" } else { "✗" };
                    println!("{} {:45}  {}", mark, path, owner);
                }
            }

            ArtifactsAction::Schema { file } => {
                let path = std::path::Path::new(&file);
                let v = load_json(path)?;
                match &v {
                    serde_json::Value::Object(map) => {
                        println!("Top-level keys ({}):", map.len());
                        for (k, val) in map {
                            let type_hint = match val {
                                serde_json::Value::Object(m) => {
                                    format!("object({} keys)", m.len())
                                }
                                serde_json::Value::Array(a) => format!("array({})", a.len()),
                                serde_json::Value::String(_) => "string".into(),
                                serde_json::Value::Number(_) => "number".into(),
                                serde_json::Value::Bool(_) => "bool".into(),
                                serde_json::Value::Null => "null".into(),
                            };
                            println!("  {}: {}", k, type_hint);
                        }
                    }
                    serde_json::Value::Array(a) => {
                        println!("Array of {} entries", a.len());
                        if let Some(first) = a.first().and_then(|x| x.as_object()) {
                            println!("First entry keys:");
                            for k in first.keys() {
                                println!("  {}", k);
                            }
                        }
                    }
                    _ => println!("{}", v),
                }
            }

            ArtifactsAction::Graph { file } => {
                let path = std::path::Path::new(&file);
                let v = load_json(path)?;
                print_state_summary(&file, &v);
                // Additional graph fields
                let events = v
                    .get("events")
                    .and_then(|x| x.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let evidence_log = v
                    .get("evidence_log")
                    .and_then(|x| x.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                if events > 0 || evidence_log > 0 {
                    println!("events={} evidence_log={}", events, evidence_log);
                }
            }
        },

        // ── git ────────────────────────────────────────────────────────────
        Commands::Git(cmd) => match cmd.action {
            GitAction::Concept { name } => {
                let out = run_git(&["log", "--oneline", "--all", "-S", &name])?;
                if out.trim().is_empty() {
                    println!("no commits found touching '{}'", name);
                } else {
                    print!("{}", out);
                }
            }
            GitAction::Pivot { concept } => {
                let out = run_git(&["log", "--oneline", "--all", "-S", &concept])?;
                let first = out.lines().last(); // oldest = last in reverse-chron output
                match first {
                    Some(line) => println!("first introduced: {}", line),
                    None => println!("no commits found for '{}'", concept),
                }
            }
            GitAction::Blame { file, term } => {
                let out = run_git(&["log", "--oneline", "-S", &term, "--", &file])?;
                if out.trim().is_empty() {
                    println!("no commits touching '{}' in {}", term, file);
                } else {
                    print!("{}", out);
                }
            }
        },

        // ── archaeology ────────────────────────────────────────────────────
        Commands::Archaeology(cmd) => match cmd.action {
            ArchaeologyAction::Timeline => {
                // Git log spine
                let git_log = run_git(&["log", "--oneline", "--reverse"])?;
                println!("=== git commit spine ===");
                for line in git_log.lines() {
                    println!("  {}", line);
                }

                // Epoch portfolio
                let portfolio_path = std::path::Path::new("epoch_logs/portfolio.json");
                if portfolio_path.exists() {
                    let raw = std::fs::read_to_string(portfolio_path)?;
                    let entries: Vec<serde_json::Value> = serde_json::from_str(&raw)?;
                    println!("\n=== epoch portfolio ({} entries) ===", entries.len());
                    for e in &entries {
                        let epoch = e.get("epoch").and_then(|x| x.as_u64()).unwrap_or(0);
                        let adopted = e.get("adopted").and_then(|x| x.as_bool()).unwrap_or(false);
                        let cd = e.get("concept_delta").and_then(|x| x.as_i64()).unwrap_or(0);
                        let ts = e.get("ts").and_then(|x| x.as_str()).unwrap_or("?");
                        println!(
                            "  epoch={:3}  adopted={}  concepts{:+}  {}",
                            epoch, adopted, cd, ts
                        );
                    }
                }
            }

            ArchaeologyAction::Compare { epoch_a, epoch_b } => {
                let pa = epoch_path(&epoch_a);
                let pb = epoch_path(&epoch_b);
                let va = load_json(&pa)?;
                let vb = load_json(&pb)?;

                print_state_summary(&format!("A ({})", epoch_a), &va);
                print_state_summary(&format!("B ({})", epoch_b), &vb);

                // Concept names diff
                let empty = serde_json::Map::new();
                let ca = va.get("concepts").and_then(|x| x.as_object()).unwrap_or(&empty);
                let cb = vb.get("concepts").and_then(|x| x.as_object()).unwrap_or(&empty);
                let added: Vec<&str> = cb.keys().filter(|k| !ca.contains_key(*k)).map(|k| k.as_str()).collect();
                let removed: Vec<&str> = ca.keys().filter(|k| !cb.contains_key(*k)).map(|k| k.as_str()).collect();
                if !added.is_empty() {
                    println!("concepts added ({}):", added.len());
                    for c in added.iter().take(20) {
                        println!("  + {}", c);
                    }
                    if added.len() > 20 {
                        println!("  … and {} more", added.len() - 20);
                    }
                }
                if !removed.is_empty() {
                    println!("concepts removed ({}):", removed.len());
                    for c in removed.iter().take(20) {
                        println!("  - {}", c);
                    }
                }
            }
            ArchaeologyAction::Narrate => {
                let thread_path = std::path::Path::new("epoch_logs/thread.md");
                if thread_path.exists() {
                    let content = std::fs::read_to_string(thread_path)?;
                    let lines: Vec<&str> = content.lines().collect();
                    let start = lines.len().saturating_sub(80);
                    for line in &lines[start..] {
                        println!("{}", line);
                    }
                } else {
                    println!("epoch_logs/thread.md not found — run an epoch first");
                }
            }
        },

        // ── reduce ─────────────────────────────────────────────────────────
        Commands::Reduce(cmd) => match cmd.action {
            ReduceAction::Scan { state, json } => {
                let tx = StateTransaction::begin(&state)?;
                let engine = ReducibleMapEngine::default();
                let candidates = engine.scan(&tx.state);
                if json {
                    println!("{}", serde_json::to_string_pretty(&candidates)?);
                } else {
                    println!("{} reduction candidates:", candidates.len());
                    for (i, c) in candidates.iter().enumerate() {
                        let kind = match &c.op {
                            nstar_bit::reduction::ReductionOp::AliasCollapse { canonical, absorbed } =>
                                format!("AliasCollapse  {} ← [{}]", canonical, absorbed.join(", ")),
                            nstar_bit::reduction::ReductionOp::ChainReduce { source, intermediary, target, .. } =>
                                format!("ChainReduce    {}→{}→{}", source, intermediary, target),
                            nstar_bit::reduction::ReductionOp::GhostPrune { concept, .. } =>
                                format!("GhostPrune     {}", concept),
                        };
                        println!("  [{i:3}] {kind}");
                        println!("        {}", c.rationale);
                    }
                }
            }

            ReduceAction::Run { state, index, receipts, threshold } => {
                let engine = ReducibleMapEngine {
                    delta_threshold: threshold,
                    ..Default::default()
                };
                let candidates = {
                    let tx = StateTransaction::begin(&state)?;
                    engine.scan(&tx.state)
                };
                let candidate = candidates.into_iter().nth(index)
                    .ok_or_else(|| anyhow::anyhow!("candidate index {index} out of range"))?;
                let mut tx = StateTransaction::begin(&state)?;
                let receipt = engine.execute(&mut tx.state, candidate.op);
                let aborted = receipt.aborted;
                let max_delta = receipt.max_delta;
                let rid = receipt.reduction_id.clone();
                append_receipt(&receipts, &receipt)?;
                if aborted {
                    println!("ABORTED  id={rid}  max_delta={max_delta:.4}  reason={}", receipt.abort_reason.as_deref().unwrap_or("?"));
                } else {
                    tx.commit()?;
                    println!("COMMITTED  id={rid}  max_delta={max_delta:.4}");
                }
            }

            ReduceAction::Dry { state, threshold } => {
                let engine = ReducibleMapEngine {
                    delta_threshold: threshold,
                    dry_run: true,
                    ..Default::default()
                };
                let tx = StateTransaction::begin(&state)?;
                let candidates = engine.scan(&tx.state);
                let total = candidates.len();
                println!("Dry-run: {total} candidates (state unchanged)");
                for c in candidates {
                    let mut state_clone = tx.state.clone();
                    let mut live_engine = ReducibleMapEngine {
                        delta_threshold: threshold,
                        ..Default::default()
                    };
                    let receipt = live_engine.execute(&mut state_clone, c.op);
                    let verdict = if receipt.aborted {
                        format!("WOULD_ABORT  delta={:.4}  {}", receipt.max_delta,
                            receipt.abort_reason.as_deref().unwrap_or(""))
                    } else {
                        format!("SAFE         delta={:.4}", receipt.max_delta)
                    };
                    println!("  {verdict}  — {}", c.rationale);
                }
            }

            ReduceAction::All { state, receipts, threshold, limit } => {
                let engine = ReducibleMapEngine {
                    delta_threshold: threshold,
                    ..Default::default()
                };
                let mut committed = 0;
                let mut aborted = 0;
                loop {
                    if committed + aborted >= limit {
                        break;
                    }
                    let candidates = {
                        let tx = StateTransaction::begin(&state)?;
                        engine.scan(&tx.state)
                    };
                    if candidates.is_empty() {
                        break;
                    }
                    let candidate = candidates.into_iter().next().unwrap();
                    let mut tx = StateTransaction::begin(&state)?;
                    let receipt = engine.execute(&mut tx.state, candidate.op);
                    if receipt.aborted {
                        aborted += 1;
                        append_receipt(&receipts, &receipt)?;
                        // Don't retry aborted ops in this run — move on.
                        break;
                    } else {
                        tx.commit()?;
                        append_receipt(&receipts, &receipt)?;
                        committed += 1;
                    }
                }
                println!("reduce all: committed={committed} aborted={aborted}  receipts={receipts}");
            }
        },
    }

    Ok(())
}
