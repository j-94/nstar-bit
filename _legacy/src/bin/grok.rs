//! System Grokker — uses n★ bit to analyze and understand a codebase.
//!
//! Instead of generating random tasks, this script points the autopoietic
//! core at a specific directory (like the Meta3 engine). It feeds files into 
//! the LM, asking it to analyze the architecture and physics of the code.
//!
//! As it processes files, the n★ collapse will discover specific predicates
//! about YOUR system: things like "Unsafe_Chaos_Mutation", "API_Boundary_Leak",
//! or "Missing_Ruliad_Context". The system grows a metacognitive
//! model of your codebase.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use nstar_bit::collapse::TurnIns;
use nstar_bit::lm::LmClient;
use nstar_bit::state::NstarState;
use nstar_bit::turn::process_turn;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: cargo run --bin grok <path_to_repo> [max_files]");
        return Ok(());
    }

    let target_dir = PathBuf::from(&args[1]);
    let max_files: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    if !target_dir.exists() {
        anyhow::bail!("Target directory does not exist: {:?}", target_dir);
    }

    let state_path = PathBuf::from("nstar_grok_state.json");
    let receipts_path = PathBuf::from("grok_receipts.jsonl");

    // Clean start for grokking session
    let _ = std::fs::remove_file(&state_path);
    let _ = std::fs::remove_file(&receipts_path);

    let mut state = NstarState::load_or_create(&state_path)?;
    let lm = LmClient::new().expect("API key required");

    println!("╔══════════════════════════════════════════════════╗");
    println!("║         N★ BIT — REPOSITORY GROKKER             ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  Target : {:<38} ║", truncate(&target_dir.display().to_string(), 38));
    println!("║  Limits : {:<38} ║", format!("Scanning up to {} files", max_files));
    println!("║                                                  ║");
    println!("║  Discovering domain-specific predicates from     ║");
    println!("║  your architecture, constraints, and physics.    ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    let files = gather_files(&target_dir, max_files)?;
    println!("Found {} files to process. Commencing ruliadic scan...\n", files.len());

    for (i, file_path) in files.iter().enumerate() {
        println!("━━━ Grokking file {}/{}: {} ━━━━━━━━━━", i + 1, files.len(), file_path.file_name().unwrap().to_string_lossy());

        let content = fs::read_to_string(file_path).unwrap_or_default();
        if content.is_empty() {
            continue;
        }

        // Generate the task for the LM
        let prompt = format!(
            "Analyze the architecture and logic of this file. Identify its role in the larger system, and point out any potential edge cases, tight coupling, or physical/security constraints.\n\nFile: {}\n\nContent:\n{}",
            file_path.display(),
            content
        );

        let outs = match lm.execute_task_simple(&prompt).await {
            Ok(o) => o,
            Err(e) => {
                println!("  ⚠ Execution failed: {}", e);
                continue;
            }
        };

        // The stream already printed the output to stdout.
        println!(); // Add a newline after the stream finishes

        if !outs.errors.is_empty() {
            println!("  Errors  : {:?}", outs.errors);
        }

        let ins = TurnIns {
            prompt: format!("Analyze file: {}", file_path.display()),
            context: vec![],
            turn: state.total_turns + 1,
        };

        match process_turn(&ins, &outs, &mut state, &state_path, &receipts_path).await {
            Ok(res) => {
                if let Some(ref p) = res.new_predicate {
                    println!("  ★ DISCOVERED PATTERN: {} [{:?}]", p.name, p.gate);
                }
                if !res.reinforced.is_empty() {
                    println!("  ↑ REINFORCED: {}", res.reinforced.join(", "));
                }
                if !res.gate_result.can_act {
                    println!("  ⛔ BLOCKED BY GATE: {}", res.gate_result.summary());
                }
            }
            Err(e) => {
                println!("  ⚠ Collapse failed: {}", e);
            }
        }
        println!();
    }

    // Final dashboard shows what it learned about your system
    println!("\n{}", state.summary());

    Ok(())
}

fn gather_files(dir: &Path, max: usize) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let iter = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    for entry in iter {
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        
        if (ext == "rs" || ext == "py" || ext == "js" || ext == "ts" || ext == "md") && !path.to_string_lossy().contains("target/") && !path.to_string_lossy().contains("node_modules/") {
            files.push(path.to_path_buf());
            if files.len() >= max {
                break;
            }
        }
    }
    
    // Sort to make it deterministic if desired, but here we just take the first N found
    Ok(files)
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ");
    if s.len() <= max {
        s
    } else {
        format!("{}…", &s[..max])
    }
}
