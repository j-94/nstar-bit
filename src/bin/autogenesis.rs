use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use nstar_bit::autogenesis::{
    adopt_state, canonical_symbol_candidates, compare_states, fork_state, health_check,
    init_state, load_state, monitor, process_turn, process_turn_with_delta, record_comparison,
    save_state, show_state, summarize_state_for_lm, SymbolExtraction, process_evidence_file,
};
use nstar_bit::lm::LmClient;
use nstar_bit::manifest;

#[derive(Parser, Debug)]
#[command(author, version, about = "Minimal graph autogenesis loop")]
struct Args {
    #[arg(long, default_value = "nstar-autogenesis/rust_state.json")]
    state: String,

    #[arg(long, default_value = "lm")]
    extractor: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init,
    Turn { message: String },
    Fork {
        #[arg(long)]
        output: String,
        #[arg(long)]
        proposed_change: String,
        #[arg(long, default_value = "")]
        reason: String,
    },
    Compare {
        #[arg(long)]
        other_state: String,
        #[arg(long, default_value = "")]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    Adopt {
        #[arg(long, default_value = "")]
        reason: String,
    },
    /// Ask the LM to generate a task list for the current fork's experiment.
    Plan {
        /// Fork proposal JSON file, or inline JSON string
        #[arg(long)]
        proposal: String,
        /// Write tasks to this file (one per line); if omitted, prints to stdout
        #[arg(long, default_value = "")]
        output: String,
    },
    /// Ask the LM to author a fork proposal. Optionally --execute to run fork+compare+adopt.
    Propose {
        /// Output path for the forked state (required when --execute is set)
        #[arg(long, default_value = "")]
        fork_output: String,
        /// Optional competing state to include in the LM's reasoning
        #[arg(long, default_value = "")]
        other_state: String,
        /// If set, immediately execute fork (and compare/adopt if applicable)
        #[arg(long)]
        execute: bool,
    },
    Show,
    Monitor {
        #[arg(long)]
        json: bool,
    },
    /// Run health invariant checks and output findings as JSON.
    Health,
    /// Ingest evidence payloads from a directory into the graph.
    Ingest {
        /// Directory containing JSON evidence payloads
        #[arg(long, default_value = "artifacts/evidence_inbox")]
        inbox: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let state_path = PathBuf::from(args.state);

    match args.command {
        Command::Init => {
            init_state(&state_path)?;
            println!("initialized: {}", state_path.display());
        }
        Command::Turn { message } => {
            let mut state = load_state(&state_path)?;

            // Load manifest if available — try deterministic dispatch first.
            // OMNI (LM) fires only when no manifest edge matches.
            let manifest_dispatch = manifest::find_and_load(&state_path);

            // Seed primitive nodes on first use
            if let Some(ref md) = manifest_dispatch {
                md.seed_primitives(&mut state);
            }

            let event = 'dispatch: {
                // 1. Try manifest deterministic dispatch
                if let Some(ref md) = manifest_dispatch {
                    if let Some(result) = md.try_dispatch(&message) {
                        eprintln!(
                            "[manifest] edge matched — deterministic dispatch (OMNI not fired)",
                        );
                        md.apply_ops(&mut state, &result);
                        // Still run process_turn for receipt + focus bookkeeping
                        break 'dispatch process_turn(&mut state, &message)?;
                    } else {
                        eprintln!("[manifest] no edge matched — OMNI fires (LM path)");
                    }
                }

                // 2. OMNI path: LM
                if args.extractor.eq_ignore_ascii_case("raw") {
                    process_turn(&mut state, &message)?
                } else if let Some(client) = LmClient::new() {
                    let raw_symbols = client.extract_turn_symbols(&message).await?;
                    let known_symbols = canonical_symbol_candidates(&state, 24);
                    let symbols = if known_symbols.is_empty() {
                        raw_symbols.clone()
                    } else {
                        client
                            .canonicalize_turn_symbols(&message, &raw_symbols, &known_symbols)
                            .await
                            .unwrap_or_else(|_| raw_symbols.clone())
                    };
                    let extraction = SymbolExtraction {
                        extractor: if known_symbols.is_empty() {
                            "lm".to_string()
                        } else {
                            "lm_canonical".to_string()
                        },
                        raw_text_sha256: nstar_bit::receipt::sha256_hex_str(&message),
                        raw_symbols,
                        symbols,
                    };
                    let summary = summarize_state_for_lm(&state);
                    let transition = client
                        .author_autogenesis_turn(&message, &extraction, &summary)
                        .await?;
                    process_turn_with_delta(
                        &mut state,
                        &message,
                        extraction,
                        transition.raw_response,
                        transition.delta,
                    )?
                } else {
                    process_turn(&mut state, &message)?
                }
            };

            save_state(&state_path, &state)?;
            println!(
                "turn={} symbols={} concepts={} relations={}",
                event.turn,
                event.symbols.len(),
                event.concept_ids.len(),
                event.relation_ids.len(),
            );
            println!(
                "discovered={} promoted={} archived={} rejected={}",
                event.discovered.len(),
                event.promoted.len(),
                event.archived.len(),
                event.transition.rejected_fields.len(),
            );
            println!(
                "focus={} seed_queue={} gate={}",
                event.active_core.len(),
                event.seeds.len(),
                event.gate.reason
            );

            // Void diagnostics placeholder — requires full manifest implementation.
        }
        Command::Fork {
            output,
            proposed_change,
            reason,
        } => {
            let state = load_state(&state_path)?;
            let forked = fork_state(&state, &proposed_change, &reason);
            let output_path = PathBuf::from(output);
            save_state(&output_path, &forked)?;
            println!(
                "forked run={} parent={} output={}",
                forked.run_lineage.run_id,
                forked.run_lineage.parent_run_id,
                output_path.display()
            );
        }
        Command::Compare {
            other_state,
            reason,
            json,
        } => {
            let mut state = load_state(&state_path)?;
            let other = load_state(&PathBuf::from(other_state))?;
            let receipt = compare_states(&state, &other, &reason);
            record_comparison(&mut state, receipt.clone());
            save_state(&state_path, &state)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            } else {
                println!("{}", receipt.summary);
            }
        }
        Command::Adopt { reason } => {
            let mut state = load_state(&state_path)?;
            adopt_state(&mut state, &reason);
            save_state(&state_path, &state)?;
            println!(
                "adopted run={} reason={}",
                state.run_lineage.run_id, reason
            );
        }
        Command::Plan { proposal, output } => {
            let state = load_state(&state_path)?;
            let summary = summarize_state_for_lm(&state);
            let proposal_obj: nstar_bit::autogenesis::LmForkProposal =
                serde_json::from_str(&proposal)
                    .or_else(|_| {
                        // treat as file path
                        std::fs::read_to_string(&proposal)
                            .map_err(|e| anyhow::anyhow!("{}", e))
                            .and_then(|s| serde_json::from_str(&s).map_err(|e| anyhow::anyhow!("{}", e)))
                    })
                    .unwrap_or_default();
            let client = LmClient::new().ok_or_else(|| anyhow::anyhow!("no API key available"))?;
            let tasks = client.author_epoch_tasks(&summary, &proposal_obj).await?;
            let text = tasks.join("\n");
            if output.is_empty() {
                println!("{}", text);
            } else {
                std::fs::write(&output, &text)?;
                println!("wrote {} tasks to {}", tasks.len(), output);
            }
        }
        Command::Propose {
            fork_output,
            other_state,
            execute,
        } => {
            let state = load_state(&state_path)?;
            let current_summary = summarize_state_for_lm(&state);

            let other_summary = if !other_state.is_empty() {
                let other = load_state(&PathBuf::from(&other_state))?;
                Some(summarize_state_for_lm(&other))
            } else {
                None
            };

            let client = LmClient::new().ok_or_else(|| anyhow::anyhow!("no API key available"))?;
            let proposal = client
                .author_fork_proposal(&current_summary, other_summary.as_ref())
                .await?;

            println!("{}", serde_json::to_string_pretty(&proposal)?);

            if execute {
                if fork_output.is_empty() {
                    bail!("--fork-output is required when --execute is set");
                }
                let forked = fork_state(&state, &proposal.proposed_change, &proposal.reason);
                let fork_path = PathBuf::from(&fork_output);
                save_state(&fork_path, &forked)?;
                println!(
                    "forked run={} parent={} output={}",
                    forked.run_lineage.run_id,
                    forked.run_lineage.parent_run_id,
                    fork_path.display()
                );

                if !other_state.is_empty() {
                    let mut base = load_state(&state_path)?;
                    let other = load_state(&PathBuf::from(&other_state))?;
                    let receipt = compare_states(&base, &other, &proposal.comparison_reason);
                    record_comparison(&mut base, receipt.clone());
                    save_state(&state_path, &base)?;
                    println!("{}", receipt.summary);
                }

                if proposal.should_adopt {
                    let mut fork_state_loaded = load_state(&fork_path)?;
                    adopt_state(&mut fork_state_loaded, &proposal.adoption_reason);
                    save_state(&fork_path, &fork_state_loaded)?;
                    println!(
                        "adopted run={} reason={}",
                        fork_state_loaded.run_lineage.run_id, proposal.adoption_reason
                    );
                }
            }
        }
        Command::Show => {
            let state = load_state(&state_path)?;
            println!("{}", show_state(&state));
        }
        Command::Monitor { json } => {
            let state = load_state(&state_path)?;
            let payload = monitor(&state);
            if json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("{}", show_state(&state));
            }
        }
        Command::Health => {
            let state = load_state(&state_path)?;
            let signal = health_check(&state);
            println!("{}", serde_json::to_string_pretty(&signal)?);
        }
        Command::Ingest { inbox } => {
            let mut state = load_state(&state_path)?;
            let inbox_path = PathBuf::from(&inbox);
            
            if !inbox_path.exists() {
                println!("Inbox directory doesn't exist: {}", inbox);
                return Ok(());
            }

            let mut processed = 0;
            let mut applied_total = 0;

            for entry in std::fs::read_dir(inbox_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                    println!("Ingesting {}", path.display());
                    match process_evidence_file(&mut state, path.to_str().unwrap()) {
                        Ok(applied_ids) => {
                            processed += 1;
                            applied_total += applied_ids.len();
                            println!("  Applied {} evidence relations", applied_ids.len());
                            // Delete the file after successful ingestion
                            std::fs::remove_file(path)?;
                        }
                        Err(e) => {
                            eprintln!("  Failed to process {}: {}", path.display(), e);
                        }
                    }
                }
            }
            
            if processed > 0 {
                save_state(&state_path, &state)?;
                println!("Ingestion complete. Processed {} files, applied {} relations.", processed, applied_total);
            } else {
                println!("No JSON payload files found in {}", inbox);
            }
        }
    }

    Ok(())
}
