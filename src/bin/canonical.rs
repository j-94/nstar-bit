use anyhow::{anyhow, Result};
use clap::Parser;
use std::io::{self, Write};
use std::path::PathBuf;

use nstar_bit::canonical::core::CanonicalCore;
use nstar_bit::canonical::types::{
    CanonicalInput, CanonicalProposal, NodeDiscovery, NodeObservation,
};
use nstar_bit::lm::{LmClient, Predicate, TurnIns, TurnOuts};
use nstar_bit::utir_exec::GuardConfig;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "N* Canonical Core (graph-first, simulation-first)"
)]
struct Args {
    #[arg(short, long)]
    interactive: bool,

    #[arg(short, long)]
    state: bool,

    #[arg(long)]
    reset: bool,

    #[arg(long, default_value = "nstar_canonical_state.json")]
    state_file: String,

    #[arg(long, default_value = "canonical_receipts.jsonl")]
    receipts_file: String,

    #[arg(long, default_value_t = 0.8)]
    max_risk: f64,

    #[arg(long, default_value_t = 0.33)]
    audit_rate: f32,

    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let state_path = PathBuf::from(args.state_file);
    let receipts_path = PathBuf::from(args.receipts_file);

    if args.reset {
        CanonicalCore::reset_files(&state_path, &receipts_path)?;
        println!("reset canonical state + receipts");
        return Ok(());
    }

    let mut core = CanonicalCore::load_or_create(&state_path)?;
    core.state.graph.criteria.max_risk = args.max_risk;
    core.state.graph.criteria.audit_rate = args.audit_rate;

    if args.state {
        println!("{}", core.summary());
        return Ok(());
    }

    if args.interactive {
        println!("N* Canonical Interactive");
        println!("Type a prompt, press Enter. Ctrl+C to exit.\n");
        loop {
            print!("User> ");
            io::stdout().flush()?;

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }
            let prompt = input.trim();
            if prompt.is_empty() {
                break;
            }

            run_turn(prompt.to_string(), &mut core, &state_path, &receipts_path).await?;
        }
    } else if let Some(prompt) = args.prompt {
        run_turn(prompt, &mut core, &state_path, &receipts_path).await?;
    } else {
        println!("Usage: canonical [PROMPT] | --interactive | --state");
    }

    Ok(())
}

async fn run_turn(
    prompt: String,
    core: &mut CanonicalCore,
    state_path: &PathBuf,
    receipts_path: &PathBuf,
) -> Result<()> {
    let lm = LmClient::new().ok_or_else(|| anyhow!("API key is required"))?;

    println!("\n[canonical] proposing response + operations...");

    let node_rules = core
        .state
        .graph
        .nodes
        .iter()
        .map(|n| format!("- {} :: {}", n.label, n.condition))
        .collect::<Vec<_>>()
        .join("\n");

    let system = format!(
        "You are an execution model inside a canonical n* core.\n\
         Return JSON ONLY with keys: response, utir_operations, actions, quality, errors.\n\
         Do not claim completion unless supported by operations/evidence.\n\
         Existing runtime dimensions:\n{}",
        if node_rules.is_empty() {
            "(none yet; discover from turn evidence)".to_string()
        } else {
            node_rules
        }
    );

    let messages = vec![
        serde_json::json!({"role":"system","content":system}),
        serde_json::json!({"role":"user","content":prompt}),
    ];

    let outs = lm.execute_task(&messages).await?;

    let proposal = CanonicalProposal {
        response: outs.response.clone(),
        actions: outs.actions.clone(),
        errors: outs.errors.clone(),
        quality: outs.quality,
        operations: outs.operations.clone(),
    };

    let input = CanonicalInput {
        prompt: prompt_from_messages(&messages),
        context: Vec::new(),
        turn: core.state.turn_count + 1,
    };

    let observations = evaluate_existing_nodes(&lm, core, &input, &proposal).await?;
    let discoveries = reflect_new_nodes(&lm, core, &input, &proposal).await?;

    let guard = GuardConfig::from_env();
    let result = core.process_turn(
        input,
        proposal,
        observations,
        discoveries,
        &guard,
        receipts_path,
    )?;

    core.save(state_path)?;

    println!("\n=== Canonical Turn Result ===");
    println!("decision      : {:?}", result.trace.decision);
    println!("gate          : {}", result.trace.gate.summary());
    println!(
        "simulation    : can_materialize={} max_risk={:.2}",
        result.trace.simulation.can_materialize, result.trace.simulation.max_risk
    );
    println!(
        "invariants    : passed={} coverage={:.2} contradiction={:.2}",
        result.trace.invariants.passed,
        result.trace.invariants.evidence_coverage,
        result.trace.invariants.contradiction_score
    );
    if !result.trace.invariants.violations.is_empty() {
        println!(
            "violations    : {}",
            result.trace.invariants.violations.join(", ")
        );
    }
    if !result.discovered_nodes.is_empty() {
        println!("discovered    : {}", result.discovered_nodes.join(", "));
    }
    for c in &result.trace.coordinates {
        println!(
            "coord[{:?}]   : event={} intensity={:.2} active={} ",
            c.scale,
            c.event_id,
            c.intensity,
            c.active_nodes.len()
        );
    }
    println!("receipt_hash  : {}", result.receipt.hash);

    Ok(())
}

fn prompt_from_messages(messages: &[serde_json::Value]) -> String {
    messages
        .iter()
        .rev()
        .find_map(|m| {
            if m.get("role").and_then(|v| v.as_str()) == Some("user") {
                m.get("content")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            } else {
                None
            }
        })
        .unwrap_or_default()
}

async fn evaluate_existing_nodes(
    lm: &LmClient,
    core: &CanonicalCore,
    input: &CanonicalInput,
    proposal: &CanonicalProposal,
) -> Result<Vec<NodeObservation>> {
    if core.state.graph.nodes.is_empty() {
        return Ok(Vec::new());
    }

    let predicates = core
        .state
        .graph
        .nodes
        .iter()
        .map(node_to_predicate)
        .collect::<Vec<_>>();

    let ins = TurnIns {
        prompt: input.prompt.clone(),
        context: input.context.clone(),
        turn: input.turn,
    };
    let outs = TurnOuts {
        response: proposal.response.clone(),
        actions: proposal.actions.clone(),
        quality: proposal.quality,
        errors: proposal.errors.clone(),
        operations: proposal.operations.clone(),
    };

    let evals = lm.evaluate_predicates(&predicates, &ins, &outs).await?;

    let mut obs = Vec::new();
    for ev in evals {
        if let Some(node) = core.state.graph.nodes.iter().find(|n| n.id == ev.name) {
            obs.push(NodeObservation {
                id: node.id.clone(),
                label: node.label.clone(),
                condition: node.condition.clone(),
                activation: ev.activation,
                control_signals: node.control_signals.clone(),
                threshold: node.threshold,
            });
        }
    }
    Ok(obs)
}

async fn reflect_new_nodes(
    lm: &LmClient,
    core: &CanonicalCore,
    input: &CanonicalInput,
    proposal: &CanonicalProposal,
) -> Result<Vec<NodeDiscovery>> {
    let predicates = core
        .state
        .graph
        .nodes
        .iter()
        .map(node_to_predicate)
        .collect::<Vec<_>>();

    let ins = TurnIns {
        prompt: input.prompt.clone(),
        context: input.context.clone(),
        turn: input.turn,
    };
    let outs = TurnOuts {
        response: proposal.response.clone(),
        actions: proposal.actions.clone(),
        quality: proposal.quality,
        errors: proposal.errors.clone(),
        operations: proposal.operations.clone(),
    };

    let reflection = lm.reflect(&predicates, &ins, &outs).await?;
    let mut out = Vec::new();

    if let Some(p) = reflection.new_predicate {
        let id = format!("node:{}", slug(&p.name));
        out.push(NodeDiscovery {
            id,
            label: p.name,
            condition: p.activation_condition,
            control_signals: p.control_signals.clone(),
            threshold: p.threshold,
            require_all: Vec::new(),
            block_any: Vec::new(),
        });
    }

    Ok(out)
}

fn node_to_predicate(node: &nstar_bit::canonical::types::GraphNode) -> Predicate {
    Predicate {
        id: node.id.clone(),
        prime_id: node.prime_id,
        name: node.id.clone(),
        discovered_at: node.discovered_turn,
        activation_condition: node.condition.clone(),
        control_signals: node.control_signals.clone(),
        threshold: node.threshold,
        activation: node.activation,
        reinforcements: node.reinforcements,
        merged_from: Vec::new(),
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}
