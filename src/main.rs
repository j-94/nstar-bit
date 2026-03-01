use anyhow::Result;
use clap::Parser;
use std::io::{self, Write};
use std::path::PathBuf;

use nstar_bit::collapse::{TurnIns, TurnOuts};
use nstar_bit::state::NstarState;
use nstar_bit::turn::process_turn;

#[derive(Parser, Debug)]
#[command(author, version, about = "N★ Bit — The Causal Collapse of Things")]
struct Args {
    /// Start in interactive REPL mode
    #[arg(short, long)]
    interactive: bool,

    /// Print the current accumulated state
    #[arg(short, long)]
    state: bool,

    /// Reset (delete) the state file and start fresh
    #[arg(long)]
    reset: bool,

    /// The user prompt to process
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let state_path = PathBuf::from("nstar_state.json");
    let receipts_path = PathBuf::from("receipts.jsonl");

    if args.reset {
        if state_path.exists() {
            std::fs::remove_file(&state_path)?;
            println!("Reset state: deleted {:?}", state_path);
        }
        if receipts_path.exists() {
            std::fs::remove_file(&receipts_path)?;
            println!("Reset receipts: deleted {:?}", receipts_path);
        }
        return Ok(());
    }

    let mut state = NstarState::load_or_create(&state_path)?;

    if args.state {
        println!("{}", state.summary());
        return Ok(());
    }

    if args.interactive {
        println!("N★ Bit Interactive Mode");
        println!("Type your task/prompt. The system will process it through the protocol.");
        println!("(Press Ctrl+C to exit)\n");

        loop {
            print!("User> ");
            io::stdout().flush()?;

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
                break;
            }

            let prompt = input.trim().to_string();
            run_single_turn(prompt, &mut state, &state_path, &receipts_path).await?;
        }
    } else if let Some(prompt) = args.prompt {
        run_single_turn(prompt, &mut state, &state_path, &receipts_path).await?;
    } else {
        println!("Usage: nstar-bit [PROMPT] | --interactive | --state");
    }

    Ok(())
}

async fn run_single_turn(
    prompt: String,
    state: &mut NstarState,
    state_path: &PathBuf,
    receipts_path: &PathBuf,
) -> Result<()> {
    println!("\n[Processing via LM...]");

    // In a real agent, we would call the LM to produce the response/actions.
    // For this demonstration, we mock the output so we focus on the META pipeline.
    let outs = TurnOuts {
        response: format!("Processed task: {}", prompt),
        actions: vec![],
        quality: 0.9,
        errors: vec![], // Add mock errors here to see it discover error predicates
    };

    let ins = TurnIns {
        prompt,
        context: vec![],
        turn: state.total_turns + 1,
    };

    match process_turn(&ins, &outs, state, state_path, receipts_path).await {
        Ok(res) => {
            println!("\n=== Turn Result ===");
            println!("Collapse Hash : {}", res.collapse.hash);
            println!("Dimensionality: {} active predicates", res.collapse.n);
            println!("Gates         : {}", res.gate_result.summary());
            if !res.reinforced.is_empty() {
                println!("Reinforced    : {}", res.reinforced.join(", "));
            }
            if let Some(new_pred) = res.new_predicate {
                println!(
                    "Discovered    : {} (Gate: {:?})",
                    new_pred.name, new_pred.gate
                );
            }
            println!("Reasoning     : {}\n", res.reasoning);
        }
        Err(e) => {
            eprintln!("Error processing turn: {}", e);
        }
    }

    Ok(())
}
