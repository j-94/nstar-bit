use anyhow::Result;
use clap::Parser;
use std::io::{self, Write};
use std::path::PathBuf;

use nstar_bit::collapse::TurnIns;
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
    println!("\n[Executing Task via LM...]");

    let lm = nstar_bit::lm::LmClient::new()
        .ok_or_else(|| anyhow::anyhow!("API key is required to run."))?;

    let system = r#"You are an AI assistant. Complete the user's task.
If you need to execute code, shell commands, or file I/O, output them in the "utir_operations" array as UTIR JSON objects (types: "shell", "fs.read", "fs.write", "http.get", "git.patch", "assert.file_exists", "assert.shell_success").
CRITICAL: If you need to read a file to complete a task, you MUST emit the `fs.read` operation FIRST, and wait for the results in the next turn before attempting to use the file's contents or issuing any `fs.write` operations. Do not hallucinate file contents!
Perform the task, and then self-assess your performance.
If you are unable to complete the task perfectly, note the issues in the "errors" array.
Return JSON ONLY:
{
  "response": "your detailed response to the task",
  "utir_operations": [], // optional array of UTIR operations
  "actions": ["hypothetical actions taken, e.g. 'read file X'"],
  "quality": 0.0-1.0,
  "errors": ["any errors, missing information, or uncertainty"]
}"#;

    let mut predicate_rules = String::new();
    if !state.predicates.is_empty() {
        predicate_rules.push_str("\n\nCRITICAL CONSTRAINTS (Learned from past failures):\n");
        for p in &state.predicates {
            predicate_rules.push_str(&format!("- {}: {}\n", p.name, p.activation_condition));
        }
        predicate_rules.push_str("You MUST NOT trigger any of these failure conditions.");
    }

    let full_system_prompt = format!("{}{}", system, predicate_rules);

    let mut messages = vec![
        serde_json::json!({"role": "system", "content": full_system_prompt}),
        serde_json::json!({"role": "user", "content": &prompt}),
    ];

    let final_outs;
    let mut accumulated_actions = Vec::new();
    let mut session_history = Vec::new(); // Local trace for the Reflector pass

    loop {
        let outs = lm.execute_task(&messages).await?;
        println!("Response: {}", outs.response);
        if !outs.errors.is_empty() {
            println!("Errors  : {:?}", outs.errors);
        }
        
        if !outs.operations.is_empty() {
            println!("\n[Executing UTIR Operations]");
            let guard = nstar_bit::utir_exec::GuardConfig::from_env();
            let doc = nstar_bit::utir::UtirDocument {
                task_id: format!("turn-{}", state.total_turns + 1),
                description: "UTIR from LLM".to_string(),
                operations: outs.operations.clone(),
                policy: None,
                bits_tracking: None,
            };
            let effects = nstar_bit::utir_exec::execute_utir(&doc, &guard);
            
            let mut effects_summary = String::from("Operation Results:\n");
            for effect in &effects {
                println!("Effect: {:?}", effect);
                effects_summary.push_str(&format!("{:?}\n", effect));
            }
            effects_summary.push_str("\nIf you need further operations to finish the task, provide them. If the task is strictly complete, return an empty utir_operations array to conclude.");

            accumulated_actions.extend(outs.actions.clone());

            // Record this exchange in the session history for the Reflector
            session_history.push(format!("Assistant suggested {} ops: {:?}", outs.operations.len(), outs.actions));
            for effect in &effects {
                session_history.push(format!("Effect: {:?}", effect));
            }

            // Add the assistant's previous response and the new execution effects
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": serde_json::to_string(&outs).unwrap_or_else(|_| outs.response.clone())
            }));
            messages.push(serde_json::json!({
                "role": "user",
                "content": effects_summary
            }));
        } else {
            final_outs = Some(outs);
            break;
        }
    }

    let mut outs = final_outs.unwrap();
    outs.actions.extend(accumulated_actions);

    println!("\n[Computing Causal Collapse...]");

    let ins = TurnIns {
        prompt,
        context: session_history,
        turn: state.total_turns + 1,
    };

    match process_turn(&ins, &outs, state, state_path, receipts_path).await {
        Ok(res) => {
            println!("\n=== Turn Result ===");
            println!("Collapse Hash : {}", res.collapse.hash);
            println!("Coordinate    : Event {} [{:?}] at intensity {:.2}", 
                res.collapse.coordinate.event_id, 
                res.collapse.coordinate.primes, 
                res.collapse.coordinate.intensity);
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
