use anyhow::{anyhow, Result};
use clap::Parser;
use std::io::{self, Write};
use std::path::PathBuf;

use nstar_bit::canonical::core::CanonicalCore;
use nstar_bit::canonical::types::{
    CanonicalInput, CanonicalProposal, NodeDiscovery, NodeObservation,
};
use nstar_bit::lm::{LmClient, OvmOp, Predicate, TurnIns, TurnOuts};
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

    #[arg(long)]
    frozen_rule: Option<String>,

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

    if let Some(ref rule) = args.frozen_rule {
        core.state.graph.scoring_rule = rule.clone();
        core.frozen_rule = Some(rule.clone());
    }

    if args.state {
        // Refresh generated views from canonical state without needing a live turn.
        core.save(&state_path)?;
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

/// Load the last N entries from rule_trajectory.jsonl
fn load_rule_trajectory(receipts_path: &PathBuf, limit: usize) -> Vec<serde_json::Value> {
    let traj_path = receipts_path.with_file_name("rule_trajectory.jsonl");
    let Ok(text) = std::fs::read_to_string(&traj_path) else { return Vec::new() };
    let mut entries: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let start = entries.len().saturating_sub(limit);
    entries.split_off(start)
}

/// Build full edge distribution summary for the LM
fn edge_distribution(core: &CanonicalCore) -> String {
    let mut edges: Vec<_> = core.state.graph.edges.iter().collect();
    edges.sort_by(|a, b| b.c11.cmp(&a.c11));
    if edges.is_empty() {
        return "(no edges yet)".to_string();
    }
    edges.iter().take(20).map(|e| {
        format!("  {}+{}: c11={} c10={} c01={} c00={}",
            e.from, e.to, e.c11, e.c10, e.c01, e.c00)
    }).collect::<Vec<_>>().join("\n")
}

/// Ask the meta model to reason about the current scoring rule before proposing changes.
/// Returns a reasoning string to inject into the main prompt.
async fn reason_about_scoring(
    lm: &LmClient,
    core: &CanonicalCore,
    receipts_path: &PathBuf,
) -> String {
    if core.state.graph.edges.is_empty() {
        return String::new();
    }

    let trajectory = load_rule_trajectory(receipts_path, 6);
    let traj_text = if trajectory.is_empty() {
        "(no prior mutations)".to_string()
    } else {
        trajectory.iter().map(|e| {
            format!("  turn={} old={} → new={} reason={}",
                e.get("turn").and_then(|v| v.as_u64()).unwrap_or(0),
                e.get("old_rule").and_then(|v| v.as_str()).unwrap_or("?"),
                e.get("new_rule").and_then(|v| v.as_str()).unwrap_or("?"),
                e.get("reason").and_then(|v| v.as_str()).unwrap_or("?"))
        }).collect::<Vec<_>>().join("\n")
    };

    let scorecard_text = if let Some(sc) = &core.state.graph.rule_scorecard {
        let misses: Vec<String> = sc.top_misses.iter().map(|e| {
            format!("  {}+{}: c11={} c10={} c01={} c00={} score={:.3} rank={}",
                e.from, e.to, e.c11, e.c10, e.c01, e.c00, e.score, e.rank)
        }).collect();
        let hits: Vec<String> = sc.top_hits.iter().map(|e| {
            format!("  {}+{}: c11={} c10={} c01={} c00={} score={:.3} rank={}",
                e.from, e.to, e.c11, e.c10, e.c01, e.c00, e.score, e.rank)
        }).collect();
        format!(
            "P@{k}={p:.3} R@{k}={r:.3} (train={tr}, test={te})\nMisses (high-ranked, absent in test):\n{m}\nHits (high-ranked, present in test):\n{h}",
            k=sc.k, p=sc.precision_at_k, r=sc.recall_at_k,
            tr=sc.train_turns, te=sc.test_turns,
            m=if misses.is_empty() { "  (none)".to_string() } else { misses.join("\n") },
            h=if hits.is_empty() { "  (none)".to_string() } else { hits.join("\n") },
        )
    } else {
        "(no scorecard yet — not enough turns)".to_string()
    };

    let system = "You are the meta-reasoning layer of a self-evolving epistemic graph.\n\
        Your job is to analyze the scoring rule's failure pattern and determine if it needs to change.\n\
        Be specific. Reference actual edge pairs and counts from the data.\n\
        Only recommend a rule change if you can identify a structural flaw from the scorecard.\n\
        Do NOT recommend changes just because a new turn arrived.\n\
        Output plain text reasoning, 3-6 sentences. No JSON.";

    let user = format!(
        "Current rule: {}\n\n\
        Rule mutation history (last 6):\n{}\n\n\
        Full edge distribution (c11 = co-activated, c10 = A without B, c01 = B without A):\n{}\n\n\
        Scorecard:\n{}\n\n\
        Question: Does the current rule have a structural flaw visible in this data? \
        What does the miss pattern tell you? Should the rule change this turn, and why?",
        if core.state.graph.scoring_rule.is_empty() { "(none yet)" } else { &core.state.graph.scoring_rule },
        traj_text,
        edge_distribution(core),
        scorecard_text,
    );

    match lm.chat_meta(system, &user).await {
        Ok(reasoning) => reasoning,
        Err(_) => String::new(),
    }
}

/// Gate: only accept define_scoring_rule if there is actual co-activation data to ground it.
fn has_grounding_evidence(core: &CanonicalCore) -> bool {
    core.state.graph.edges.iter().any(|e| e.c11 > 0)
}

async fn run_turn(
    prompt: String,
    core: &mut CanonicalCore,
    state_path: &PathBuf,
    receipts_path: &PathBuf,
) -> Result<()> {
    let lm = LmClient::new().ok_or_else(|| anyhow!("API key is required"))?;

    // Only invoke meta-reasoning once a scorecard exists — before that there's no data to reason about
    let rule_reasoning = if core.state.graph.rule_scorecard.is_some() {
        println!("\n[canonical] reasoning about scoring state...");
        let r = reason_about_scoring(&lm, core, receipts_path).await;
        if !r.is_empty() {
            println!("  [reasoning] {}", &r[..r.len().min(120)]);
        }
        r
    } else {
        String::new()
    };

    println!("[canonical] proposing response + operations...");

    // Show top 30 most-reinforced nodes only — keeps prompt size bounded
    let mut sorted_nodes: Vec<_> = core.state.graph.nodes.iter().collect();
    sorted_nodes.sort_by(|a, b| b.reinforcements.cmp(&a.reinforcements));
    let node_rules = sorted_nodes.iter().take(30)
        .map(|n| {
            let reinf = if n.reinforcements > 0 { format!(" ({}x)", n.reinforcements) } else { String::new() };
            format!("- {}{} :: {}", n.label, reinf, &n.condition[..n.condition.len().min(80)])
        })
        .collect::<Vec<_>>()
        .join("\n");

    let trajectory = load_rule_trajectory(receipts_path, 4);
    let traj_summary = if trajectory.is_empty() {
        String::new()
    } else {
        let lines: Vec<String> = trajectory.iter().map(|e| {
            format!("  t={}: {} → {} ({})",
                e.get("turn").and_then(|v| v.as_u64()).unwrap_or(0),
                e.get("old_rule").and_then(|v| v.as_str()).unwrap_or("?"),
                e.get("new_rule").and_then(|v| v.as_str()).unwrap_or("?"),
                e.get("reason").and_then(|v| v.as_str()).unwrap_or("?"))
        }).collect();
        format!("Rule mutation history:\n{}\n\n", lines.join("\n"))
    };

    let scorecard_block = if let Some(sc) = &core.state.graph.rule_scorecard {
        let misses: Vec<String> = sc.top_misses.iter().map(|e| {
            format!("  {}+{}: c11={} c10={} c01={} c00={} score={:.3}",
                e.from, e.to, e.c11, e.c10, e.c01, e.c00, e.score)
        }).collect();
        let hits: Vec<String> = sc.top_hits.iter().map(|e| {
            format!("  {}+{}: c11={} c10={} c01={} c00={} score={:.3}",
                e.from, e.to, e.c11, e.c10, e.c01, e.c00, e.score)
        }).collect();
        format!(
            "Held-out scorecard (P@{k}={p:.3}, R@{k}={r:.3}, train={tr}, test={te}):\n\
            Misses (false positives — rule ranks these high but they don't co-activate in test):\n{m}\n\
            Hits (true positives — rule ranks these high and they do co-activate in test):\n{h}\n\n",
            k=sc.k, p=sc.precision_at_k, r=sc.recall_at_k, tr=sc.train_turns, te=sc.test_turns,
            m=if misses.is_empty() { "  (none)".to_string() } else { misses.join("\n") },
            h=if hits.is_empty() { "  (none)".to_string() } else { hits.join("\n") },
        )
    } else {
        String::new()
    };

    let reasoning_block = if rule_reasoning.is_empty() {
        String::new()
    } else {
        format!("Meta-reasoning about current rule:\n{}\n\n", rule_reasoning)
    };

    let grounding_note = if !has_grounding_evidence(core) {
        "IMPORTANT: No co-activation data exists yet (all c11=0). \
        Do NOT emit define_scoring_rule — there is no data to ground a rule. \
        Let the substrate accumulate counts first.\n\n"
    } else {
        ""
    };

    let system = format!(
        "You are an execution model inside a canonical n* core.\n\
         \n\
         Return JSON ONLY with these keys:\n\
           response         — your answer to the user\n\
           utir_operations  — file/shell/http operations (typed UTIR objects)\n\
           actions          — human-readable list of actions taken\n\
           quality          — float 0.0-1.0 self-assessment\n\
           errors           — any errors or missing information\n\
           ovm_operations   — operator definitions for the scoring substrate (see below)\n\
         \n\
         ovm_operations is an optional array. Use it ONLY when:\n\
           1. The scorecard shows a specific structural flaw in the current rule, AND\n\
           2. The meta-reasoning above identifies what needs to change, AND\n\
           3. There is actual co-activation data (c11 > 0) to ground the new rule.\n\
         Do NOT change the rule just because a new turn arrived.\n\
         Each ovm_operation entry is one of:\n\
           {{\"operation\": \"define_scoring_rule\", \"rule\": \"<evalexpr>\"}}\n\
           {{\"operation\": \"define_selection_predicate\", \"predicate\": \"<evalexpr boolean>\"}}\n\
         \n\
         Variables: c11 (co-activated), c10 (A without B), c01 (B without A), c00, t (total turns).\n\
         Functions: log(x), sqrt(x), abs(x).\n\
         \n\
         {grounding}\
         Current rule: {rule}\n\n\
         {traj}\
         {scorecard}\
         {reasoning}\
         Existing predicates (id :: condition [history]):\n{nodes}",
        grounding = grounding_note,
        rule = if core.state.graph.scoring_rule.is_empty() { "(not yet defined)" } else { &core.state.graph.scoring_rule },
        traj = traj_summary,
        scorecard = scorecard_block,
        reasoning = reasoning_block,
        nodes = if node_rules.is_empty() { "(none yet; discover from turn evidence)".to_string() } else { node_rules },
    );

    let messages = vec![
        serde_json::json!({"role":"system","content":system}),
        serde_json::json!({"role":"user","content":prompt}),
    ];

    let mut outs = lm.execute_task(&messages).await?;

    // Evidence-origin gate: strip scoring rule proposals if no grounding data exists
    if !has_grounding_evidence(core) {
        let before = outs.ovm_operations.len();
        outs.ovm_operations.retain(|op| !matches!(op, OvmOp::DefineScoringRule { .. }));
        if outs.ovm_operations.len() < before {
            println!("  [gate] stripped define_scoring_rule — no c11>0 evidence to ground it");
        }
    }

    let proposal = CanonicalProposal {
        response: outs.response.clone(),
        actions: outs.actions.clone(),
        errors: outs.errors.clone(),
        quality: outs.quality,
        operations: outs.operations.clone(),
        ovm_ops: outs.ovm_operations.clone(),
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

    // Populate seed_queue from epistemic gaps — this is the self-direction loop.
    // Seeds drive the next investigation without human intervention.
    populate_seed_queue(core);

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

/// Populate the graph's seed_queue from its own epistemic gaps.
/// Seeds become the next investigation turns — self-direction without human intervention.
fn populate_seed_queue(core: &mut CanonicalCore) {
    let already: std::collections::HashSet<(String, String)> =
        core.state.graph.investigated_pairs.iter().cloned().collect();

    let mut new_pairs: Vec<(String, String, String)> = Vec::new(); // (a, b, prompt)

    // Source 1: scorecard top_misses — rule ranks high but absent in test
    if let Some(sc) = &core.state.graph.rule_scorecard {
        for miss in sc.top_misses.iter().take(2) {
            let a = miss.from.replace("node:", "");
            let b = miss.to.replace("node:", "");
            if already.contains(&(a.clone(), b.clone())) { continue; }
            new_pairs.push((a.clone(), b.clone(), format!(
                "SELF-INVESTIGATION: The scoring rule predicts co-activation between \
                '{a}' and '{b}' (score={score:.3}) but they do NOT co-activate in held-out data \
                (c11={c11}, c10={c10}, c01={c01}). Are these genuinely independent? \
                Is one a prerequisite without being sufficient? Reason from first principles.",
                score = miss.score, c11 = miss.c11, c10 = miss.c10, c01 = miss.c01,
            )));
        }
    }

    // Source 2: high asymmetry — both fire alone often, never together
    let mut contested: Vec<_> = core.state.graph.edges.iter()
        .filter(|e| e.c11 == 0 && e.c10 >= 3 && e.c01 >= 3)
        .collect();
    contested.sort_by(|a, b| (b.c10 + b.c01).cmp(&(a.c10 + a.c01)));
    for edge in contested.iter().take(1) {
        let a = edge.from.replace("node:", "");
        let b = edge.to.replace("node:", "");
        if already.contains(&(a.clone(), b.clone())) { continue; }
        new_pairs.push((a.clone(), b.clone(), format!(
            "SELF-INVESTIGATION: '{a}' fires alone {c10}x, '{b}' fires alone {c01}x, \
            never together (c11=0). Are these mutually exclusive architectural choices, \
            or just not yet co-observed? What would a system look like that does both?",
            c10 = edge.c10, c01 = edge.c01,
        )));
    }

    if !new_pairs.is_empty() {
        println!("  [seeds] +{} new investigation prompts", new_pairs.len());
        for (a, b, prompt) in new_pairs {
            core.state.graph.investigated_pairs.push((a, b));
            core.state.graph.seed_queue.push(prompt);
        }
    }
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
    // Once the graph is large, only evaluate top 20 most-reinforced nodes to keep turn time bounded
    let max_eval = if core.state.graph.nodes.len() > 40 { 20 } else { core.state.graph.nodes.len() };

    let mut sorted: Vec<_> = core.state.graph.nodes.iter().collect();
    sorted.sort_by(|a, b| b.reinforcements.cmp(&a.reinforcements));
    let predicates: Vec<_> = sorted.iter().take(max_eval).map(|n| node_to_predicate(n)).collect();

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
        ovm_operations: proposal.ovm_ops.clone(),
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
    let existing: Vec<String> = core
        .state
        .graph
        .nodes
        .iter()
        .map(|n| format!("{} :: {}", n.id, n.condition))
        .collect();

    // Ask the meta model to discover multiple orthogonal predicates from this turn.
    // Multiple predicates per turn = pairs can form = co-activation data = scorable hypotheses.
    let system = "You are the predicate discovery layer of a self-evolving epistemic graph.\n\
        Your job: from the turn content, extract 2-5 distinct, orthogonal behavioral predicates \
        that characterize what this system was doing or trying to do.\n\
        \n\
        Rules:\n\
        - Each predicate must be independently activatable — they should NOT always fire together.\n\
        - Do not duplicate existing predicates (listed below).\n\
        - Use snake_case names, short and reusable across many different systems.\n\
        - activation_condition: a specific observable pattern that triggers this predicate.\n\
        - threshold: 0.5 for most, 0.7 for strong signals, 0.3 for weak hints.\n\
        \n\
        Return JSON array ONLY:\n\
        [{\"name\": \"...\", \"activation_condition\": \"...\", \"threshold\": 0.5}, ...]";

    let user = format!(
        "Existing predicates (do not duplicate):\n{}\n\n\
        Turn content:\n{}\n\n\
        Discover 2-5 new orthogonal predicates from this turn. \
        Focus on architectural patterns, epistemic strategies, and failure modes visible in the content.",
        if existing.is_empty() { "(none yet)".to_string() } else { existing.join("\n") },
        &input.prompt[..input.prompt.len().min(2000)],
    );

    let response = lm.chat_raw(system, &user).await?;
    let clean = response
        .lines()
        .filter(|l| !l.trim().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&clean)
        .or_else(|_| serde_json::from_str(&response))
        .unwrap_or_default();

    let existing_ids: std::collections::HashSet<String> = core
        .state.graph.nodes.iter().map(|n| n.id.clone()).collect();

    let mut out = Vec::new();
    for item in parsed.iter().take(5) {
        let name = match item.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        let condition = item.get("activation_condition")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string();
        let threshold = item.get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;

        let id = format!("node:{}", slug(&name));
        if existing_ids.contains(&id) {
            continue;
        }
        out.push(NodeDiscovery {
            id,
            label: name,
            condition,
            control_signals: Vec::new(),
            threshold,
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
