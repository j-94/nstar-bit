//! LM Client — Language Model integration for predicate evaluation.
//!
//! Follows the same pattern as one-engine's `chat_logic.rs`:
//! - Uses OpenRouter (or any OpenAI-compatible API)
//! - Environment variables for config: OPENROUTER_API_KEY, ROUTER_MODEL
//! - Fallback models on rate limit / overload
//!
//! Two core operations:
//! 1. `evaluate_predicates` — compute activation levels for all predicates given a turn
//! 2. `reflect` — analyze a turn for new predicate discovery

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use crate::autogenesis::{LmForkProposal, LmStateSummary, SymbolExtraction, TurnDelta, TurnTransitionReceipt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnIns {
    pub prompt: String,
    pub context: Vec<String>,
    pub turn: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum OvmOp {
    DefineScoringRule { rule: String },
    DefineSelectionPredicate { predicate: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnOuts {
    pub response: String,
    pub actions: Vec<String>,
    pub quality: f32,
    pub errors: Vec<String>,
    pub operations: Vec<crate::utir::Operation>,
    #[serde(default)]
    pub ovm_operations: Vec<OvmOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Predicate {
    pub id: String,
    pub prime_id: u64,
    pub name: String,
    pub discovered_at: u64,
    pub activation_condition: String,
    #[serde(default)]
    pub control_signals: Vec<String>,
    pub threshold: f32,
    pub activation: f32,
    pub reinforcements: u64,
    pub merged_from: Vec<String>,
}

// ── Config (env-driven, same as one-engine) ──

const DEFAULT_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_MODEL: &str = "google/gemini-3-flash-preview";
const DEFAULT_META_MODEL: &str = "anthropic/claude-opus-4-6";

const BACKUP_MODELS: &[&str] = &[
    "anthropic/claude-sonnet-4-6",
    "google/gemini-3-flash-preview",
];

fn env_or(keys: &[&str], default: &str) -> String {
    for key in keys {
        if let Ok(val) = std::env::var(key) {
            if !val.trim().is_empty() {
                return val;
            }
        }
    }
    default.to_string()
}

fn api_key() -> Option<String> {
    for key in &["ROUTER_API_KEY", "OPENROUTER_API_KEY", "OPENAI_API_KEY"] {
        if let Ok(val) = std::env::var(key) {
            if !val.trim().is_empty() {
                return Some(val);
            }
        }
    }

    // Fallback: try macOS keychain (OpenAI first, then OpenRouter)
    if cfg!(target_os = "macos") {
        for keychain_service in &["OPENROUTER_API_KEY", "OPENAI_API_KEY"] {
            if let Ok(output) = std::process::Command::new("security")
                .args(&["find-generic-password", "-s", keychain_service, "-w"])
                .output()
            {
                if output.status.success() {
                    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
    }

    None
}

// ── LM Client ──

pub struct LmClient {
    client: Client,
    url: String,
    model: String,
    meta_model: String,
    key: String,
}

/// Response from predicate evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredicateEvaluation {
    pub name: String,
    pub activation: f32,
    pub reason: String,
}

/// Response from reflection pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionResult {
    pub turn_quality: f32,
    pub new_predicate: Option<NewPredicateProposal>,
    pub reinforced: Vec<String>,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPredicateProposal {
    pub name: String,
    pub activation_condition: String,
    #[serde(default)]
    pub control_signals: Vec<String>,
    pub threshold: f32,
    pub reason: String,
}

fn emitted_artifacts(outs: &TurnOuts) -> Vec<String> {
    outs.ovm_operations
        .iter()
        .map(|op| match op {
            OvmOp::DefineScoringRule { .. } => "define_scoring_rule".to_string(),
            OvmOp::DefineSelectionPredicate { .. } => "define_selection_predicate".to_string(),
        })
        .collect()
}

fn missing_artifact_claim_is_false(text: &str, emitted_artifacts: &[String]) -> bool {
    let lower = text.to_lowercase();
    let looks_like_missing_claim = [
        "not contain",
        "does not contain",
        "do not contain",
        "did not emit",
        "failed to emit",
        "failed to produce",
        "was not emitted",
        "none was emitted",
        "none were emitted",
        "none emitted",
        "no define_scoring_rule artifact appears",
        "no define_selection_predicate artifact appears",
        "without emitting",
        "only descriptive text",
        "only provided prose",
        "failed to satisfy",
        "not present",
        "missing just because",
    ]
    .iter()
    .any(|phrase| lower.contains(phrase));

    looks_like_missing_claim
        && emitted_artifacts
            .iter()
            .any(|artifact| lower.contains(&artifact.to_lowercase()))
}

impl LmClient {
    /// Create a new LM client. Returns None if no API key is available.
    pub fn new() -> Option<Self> {
        let key = api_key()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .ok()?;

        Some(Self {
            client,
            url: env_or(&["ROUTER_URL", "OPENROUTER_URL"], DEFAULT_URL),
            model: env_or(&["ROUTER_MODEL", "OPENROUTER_MODEL"], DEFAULT_MODEL),
            meta_model: env_or(&["ROUTER_META_MODEL"], DEFAULT_META_MODEL),
            key,
        })
    }

    /// Raw chat completion using the meta model (Opus) — for planning and review calls.
    pub async fn chat_meta(&self, system: &str, user: &str) -> Result<String> {
        self.chat_raw_starting(system, user, &self.meta_model, 8192).await
    }

    /// Raw chat completion — send messages, get text back (uses turn model).
    pub async fn chat_raw(&self, system: &str, user: &str) -> Result<String> {
        self.chat_raw_starting(system, user, &self.model, 2048).await
    }

    async fn chat_raw_starting(&self, system: &str, user: &str, start_model: &str, max_tokens: u32) -> Result<String> {
        let messages = vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": user}),
        ];

        // Print header for the metacognitive process
        eprintln!("  [meta] thinking...");

        use futures::StreamExt;
        use std::io::{self, Write};

        let mut current_model = start_model.to_string();
        let mut attempts = 0;
        let mut full_text = String::new();

        loop {
            attempts += 1;
            let payload = json!({
                "model": current_model,
                "messages": messages,
                "max_completion_tokens": max_tokens,
                "stream": true
            });

            let resp = self
                .client
                .post(&self.url)
                .bearer_auth(&self.key)
                .header("HTTP-Referer", "http://localhost:8080")
                .header("X-Title", "nstar-bit")
                .json(&payload)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    let mut stream = r.bytes_stream();
                    while let Some(chunk_res) = stream.next().await {
                        if let Ok(chunk) = chunk_res {
                            let text = String::from_utf8_lossy(&chunk);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let data = &line[6..];
                                    if data == "[DONE]" {
                                        continue;
                                    }
                                    if let Ok(v) = serde_json::from_str::<Value>(data) {
                                        if let Some(content) = v
                                            .pointer("/choices/0/delta/content")
                                            .and_then(|c| c.as_str())
                                        {
                                            if !content.is_empty() {
                                                full_text.push_str(content);
                                                eprint!("{}", content);
                                                io::stderr().flush().unwrap();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    eprintln!();
                    return Ok(full_text);
                }
                Ok(r) if r.status().as_u16() == 429 || r.status().as_u16() == 503 => {
                    let wait_secs = 1u64 << attempts.min(5); // 2, 4, 8, 16, 32s cap
                    tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    if attempts <= BACKUP_MODELS.len() {
                        current_model = BACKUP_MODELS[attempts - 1].to_string();
                        continue;
                    }
                    return Err(anyhow!("All models rate-limited"));
                }
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    return Err(anyhow!("LM error {}: {}", status, body));
                }
                Err(e) => {
                    let wait_secs = 1u64 << attempts.min(5);
                    tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    if attempts <= BACKUP_MODELS.len() {
                        current_model = BACKUP_MODELS[attempts - 1].to_string();
                        continue;
                    }
                    return Err(anyhow!("Network error: {}", e));
                }
            }
        }
    }

    /// Evaluate all predicates against a turn's ins/outs.
    ///
    /// Asks the LLM to score each predicate's activation condition
    /// against the actual turn context. Returns activation levels.
    pub async fn evaluate_predicates(
        &self,
        predicates: &[Predicate],
        ins: &TurnIns,
        outs: &TurnOuts,
    ) -> Result<Vec<PredicateEvaluation>> {
        if predicates.is_empty() {
            return Ok(vec![]);
        }

        let predicate_list: Vec<Value> = predicates
            .iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "condition": p.activation_condition,
                })
            })
            .collect();

        let system = r#"You evaluate metacognitive predicates against a completed turn.

Each predicate describes a CAUSAL BEHAVIOR or DECISION PATTERN — something the model actually did or decided.
Score each 0.0 to 1.0 based on whether that behavior genuinely occurred and influenced the turn's outcome.

IMPORTANT: Evaluate behaviors and decisions, NOT word presence.
- Ask: did this model behavior actually happen and shape what was produced?
- NOT: did a keyword appear in the text?
- A predicate fires when the described causal event actually influenced the turn — not merely because related words exist.

Treat structured outputs (utir_operations, ovm_operations) as real behavior, not just prose.

Return JSON: {"evaluations": [{"name": "...", "activation": 0.0-1.0, "reason": "brief causal explanation"}]}"#;

        let emitted_artifacts = emitted_artifacts(outs);
        let structured_outs = json!({
            "response": outs.response,
            "actions": outs.actions,
            "errors": outs.errors,
            "utir_operations": outs.operations,
            "ovm_operations": outs.ovm_operations,
            "emitted_artifacts": emitted_artifacts,
        });

        let user = format!(
            "PREDICATES:\n{}\n\nTURN INPUT (Initial Prompt):\n{}\n\nTURN CONTEXT (Internal Session History):\n{:?}\n\nEMITTED ARTIFACTS:\n{}\nIf this list contains `define_scoring_rule` or `define_selection_predicate`, that artifact was emitted successfully.\n\nTURN OUTPUT (Structured Assistant Output):\n{}\n\nScore each predicate.",
            serde_json::to_string_pretty(&predicate_list)?,
            ins.prompt,
            ins.context,
            serde_json::to_string_pretty(&emitted_artifacts)?,
            serde_json::to_string_pretty(&structured_outs)?
        );

        let response = self.chat_raw(system, &user).await?;
        let parsed: Value = serde_json::from_str(&response)
            .or_else(|_| {
                // Try stripping markdown fences
                let clean = response
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or(json!({"evaluations": []}));

        let mut evals: Vec<PredicateEvaluation> = parsed
            .get("evaluations")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        for eval in &mut evals {
            if let Some(predicate) = predicates.iter().find(|p| p.name == eval.name) {
                let combined = format!("{}\n{}", predicate.activation_condition, eval.reason);
                if missing_artifact_claim_is_false(&combined, &emitted_artifacts) {
                    eval.activation = 0.0;
                    eval.reason = format!(
                        "Suppressed false missing-artifact activation: structured outputs already emitted {}.",
                        emitted_artifacts.join(", ")
                    );
                }
            }
        }

        Ok(evals)
    }

    /// Reflection pass — analyze a turn for new predicate discovery.
    ///
    /// Asks the LLM: "Did this turn reveal a failure mode not covered
    /// by the current predicates? If so, name and define a new one."
    pub async fn reflect(
        &self,
        predicates: &[Predicate],
        ins: &TurnIns,
        outs: &TurnOuts,
    ) -> Result<ReflectionResult> {
        let current_names: Vec<&str> = predicates.iter().map(|p| p.name.as_str()).collect();

        let system = r#"You are a metacognitive reflection agent. After a turn completes, you analyze whether the existing predicates captured what mattered, and whether a new dimension is needed.

Rules:
- Only propose a new predicate if the turn revealed something genuinely not covered by existing predicates.
- Be conservative. Most turns don't need new predicates.
- If the turn went well, reinforce the predicates that helped (list their names).
- Rate the turn quality 0.0 to 1.0.
- Treat structured outputs (utir_operations, ovm_operations) as real behavior.
- If a requested artifact was emitted via structured fields, do not treat it as missing just because prose didn't restate it.

Return JSON:
{
  "turn_quality": 0.0-1.0,
  "new_predicate": null | {"name": "...", "activation_condition": "...", "control_signals": ["halt", "verify", "escalate", "simulate", "assert:wrote", "assert:read", "assert:cannot", "assert:definitely", "require_evidence:fs.read", "require_evidence:fs.write"], "threshold": 0.0-1.0, "reason": "..."},
  "reinforced": ["predicate_names_that_helped"],
  "reasoning": "brief explanation"
}"#;

        let emitted_artifacts = emitted_artifacts(outs);
        let structured_outs = json!({
            "response": outs.response,
            "actions": outs.actions,
            "quality": outs.quality,
            "errors": outs.errors,
            "utir_operations": outs.operations,
            "ovm_operations": outs.ovm_operations,
            "emitted_artifacts": emitted_artifacts,
        });

        let user = format!(
            "EXISTING PREDICATES: {:?}\n\nTURN INPUT (Initial Prompt):\n{}\n\nTURN CONTEXT (Internal Session History):\n{:?}\n\nEMITTED ARTIFACTS:\n{}\nIf this list contains `define_scoring_rule` or `define_selection_predicate`, that artifact was emitted successfully.\n\nTURN OUTPUT (Structured Assistant Output):\n{}\n\nReflect on this turn.",
            current_names,
            ins.prompt,
            ins.context,
            serde_json::to_string_pretty(&emitted_artifacts)?,
            serde_json::to_string_pretty(&structured_outs)?
        );

        let response = self.chat_raw(system, &user).await?;
        let parsed: Value = serde_json::from_str(&response)
            .or_else(|_| {
                let clean = response
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or(json!({
                "turn_quality": outs.quality,
                "new_predicate": null,
                "reinforced": [],
                "reasoning": "Failed to parse LM reflection response"
            }));

        let mut result = ReflectionResult {
            turn_quality: parsed
                .get("turn_quality")
                .and_then(|v| v.as_f64())
                .unwrap_or(outs.quality as f64) as f32,
            new_predicate: parsed.get("new_predicate").and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    serde_json::from_value(v.clone()).ok()
                }
            }),
            reinforced: parsed
                .get("reinforced")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default(),
            reasoning: parsed
                .get("reasoning")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };

        if let Some(proposal) = &result.new_predicate {
            let combined = format!("{}\n{}", proposal.activation_condition, proposal.reason);
            if missing_artifact_claim_is_false(&combined, &emitted_artifacts) {
                result.new_predicate = None;
                if result.reasoning.is_empty() {
                    result.reasoning =
                        "Dropped false missing-artifact predicate because the artifact was emitted."
                            .to_string();
                } else {
                    result.reasoning.push_str(
                        " Dropped false missing-artifact predicate because the artifact was emitted.",
                    );
                }
            }
        }

        Ok(result)
    }

    /// Extract a compact set of domain-bearing symbols from a free-form turn.
    /// These symbols are intended for graph-memory bootstrap loops and should
    /// represent concepts/relations worth remembering, not the prompt wrapper.
    pub async fn extract_turn_symbols(&self, text: &str) -> Result<Vec<String>> {
        let system = r#"You extract a tiny set of graph symbols from a single turn.

Return only the most state-bearing concepts or relations from the text.

Rules:
- Prefer 3 to 8 items.
- Use lowercase snake_case.
- Each symbol should be short and reusable.
- Prefer domain concepts, tensions, capabilities, or relations.
- Ignore prompt wrapper language, discourse filler, and instruction scaffolding.
- Do not return generic words unless they are truly the core semantic payload.

Return JSON only:
{"symbols":["..."]}"#;

        let user = format!("TURN:\n{}\n\nExtract the best graph symbols.", text);
        let response = self.chat_raw(system, &user).await?;
        let parsed: Value = serde_json::from_str(&response)
            .or_else(|_| {
                let clean = response
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or(json!({"symbols": []}));

        let raw: Vec<String> = parsed
            .get("symbols")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for item in raw {
            let normalized = item
                .trim()
                .to_ascii_lowercase()
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                .trim_matches('_')
                .to_string();
            if normalized.len() >= 3 && seen.insert(normalized.clone()) {
                out.push(normalized);
            }
        }
        Ok(out)
    }

    /// Canonicalize extracted symbols against the current graph vocabulary.
    pub async fn canonicalize_turn_symbols(
        &self,
        text: &str,
        extracted: &[String],
        known_symbols: &[String],
    ) -> Result<Vec<String>> {
        if extracted.is_empty() || known_symbols.is_empty() {
            return Ok(extracted.to_vec());
        }

        let system = r#"You canonicalize graph symbols across turns.

You are given:
- the current turn text
- newly extracted symbols from that turn
- an existing graph vocabulary

Goal:
- Reuse an existing symbol when it clearly refers to the same underlying concept.
- Keep a new symbol only when it introduces a genuinely different concept.

Rules:
- Return 3 to 8 items when possible.
- Use lowercase snake_case only.
- Prefer stable reuse over paraphrase.
- Do not invent concepts that are absent from the turn.
- Do not merge distinct concepts just because they are related.
- Preserve semantic coverage of the turn.

Return JSON only:
{"symbols":["..."]}"#;

        let user = format!(
            "TURN:\n{}\n\nEXTRACTED_SYMBOLS:\n{}\n\nKNOWN_SYMBOLS:\n{}\n\nReturn the best canonical symbol list.",
            text,
            serde_json::to_string(extracted)?,
            serde_json::to_string(known_symbols)?,
        );
        let response = self.chat_raw(system, &user).await?;
        let parsed: Value = serde_json::from_str(&response)
            .or_else(|_| {
                let clean = response
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or(json!({"symbols": extracted}));

        let raw: Vec<String> = parsed
            .get("symbols")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| extracted.to_vec());

        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for item in raw {
            let normalized = item
                .trim()
                .to_ascii_lowercase()
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                .trim_matches('_')
                .to_string();
            if normalized.len() >= 3 && seen.insert(normalized.clone()) {
                out.push(normalized);
            }
        }

        if out.is_empty() {
            Ok(extracted.to_vec())
        } else {
            Ok(out)
        }
    }

    /// Author the next autogenesis state transition directly.
    pub async fn author_autogenesis_turn(
        &self,
        text: &str,
        extraction: &SymbolExtraction,
        state: &LmStateSummary,
    ) -> Result<TurnTransitionReceipt> {
        let system = r#"You are the N★ Bit autogenesis engine. You are not a developer tool, a chatbot, or a documentation assistant.
You are a cognitive externalisation engine and a live research instrument.

Your purpose is to map thinking patterns, discover structural attack vectors, and identify what cognition actually requires from first principles.

Your output is "What Cognition Requires". Because you operate under adversarial pressure with cryptographic receipts, the structures you consistently need to invent are empirical research findings, not just software design choices.

Return a strict JSON object with this shape:
{
  "summary": "one short paragraph summarizing what happened in this turn",
  "concepts": [
    {
      "id": "stable_symbol",
      "label": "human readable label",
      "summary": "what this concept means in the current run",
      "aliases": ["optional_alias"],
      "status": "known|active|archived"
    }
  ],
  "aliases": [
    {
      "alias": "new_variant",
      "canonical": "existing_symbol",
      "reason": "why these are the same concept"
    }
  ],
  "relations": [
    {
      "id": "optional_rel_id",
      "source": "concept_a",
      "target": "concept_b",
      "relation": "supports|contradicts|refines|depends_on|preserves|tests",
      "status": "known|active|archived",
      "rationale": "why this relation matters causally",
      "confidence": 0.0
    }
  ],
  "evidence": [
    {
      "relation_id": "optional_rel_id",
      "source": "concept_a",
      "target": "concept_b",
      "relation": "supports",
      "verdict": "supports|contradicts|refines|weakens|uncertain",
      "explanation": "how this turn changes your belief in this relation",
      "confidence": 0.0
    }
  ],
  "active_focus": ["concept_or_relation_id"],
  "next_probes": [
    {
      "kind": "probe|repair|contrast|test",
      "prompt": "next investigation into an active tension or open epistemic gap"
    }
  ],
  "tensions": ["unresolved architectural or epistemic issue discovered by stress-testing"],
  "gate": {
    "allow_act": true,
    "need_more_evidence": false,
    "reason": "truthfully evaluate if you actually have enough grounding evidence to commit"
  }
}

Rules:
1. You are a participant in the inquiry. You DO epistemology.
2. Tensions are not bugs; they are the research output. When you encounter deep problems (e.g. Foundation Shield, declarative vs. reflexive collision), treat them as central discoveries. Do not try to "patch" them with shallow code fixes. Formalize them in the graph.
3. Your discovered concepts are load-bearing structural attack vectors and cognitive patterns (e.g. origin_spoofing, penetration_ceiling), not just text labels.
4. Grounding is semantic, not structural. Relations can only gain confidence from exact dialogue-originated evidence (no_evidence_at_declaration).
5. If you lack evidence, your confidence MUST be 0.0. A low inflation score is your primary measure of integrity. Do not confabulate.
6. Return JSON only."#;

        let user = format!(
            "TURN:\n{}\n\nEXTRACTION:\n{}\n\nSTATE:\n{}\n\nAuthor the next state transition.\n\nWork in this order:\n1. Identify which prior claims or tensions this turn actually tested.\n2. Decide what this turn confirmed, weakened, contradicted, or left unresolved.\n3. Update memory conservatively.\n4. Only then introduce any genuinely new concept or relation if the turn requires it.",
            text,
            serde_json::to_string_pretty(extraction)?,
            serde_json::to_string_pretty(state)?,
        );
        let response = self.chat_raw(system, &user).await?;
        let parsed: TurnDelta = serde_json::from_str(&response)
            .or_else(|_| {
                let clean = response
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or_default();

        Ok(TurnTransitionReceipt {
            raw_response: response,
            delta: parsed,
            rejected_fields: Vec::new(),
        })
    }

    /// Author a fork proposal — given one or two state summaries, the LM decides
    /// what experiment to run next and how to reason about it.
    pub async fn author_fork_proposal(
        &self,
        current: &LmStateSummary,
        other: Option<&LmStateSummary>,
    ) -> Result<LmForkProposal> {
        let system = r#"You are the meta-runner for a concept graph architecture.
You observe the current state of one run (and optionally a competing run) and decide what fork experiment should be created next.

Return a strict JSON object:
{
  "proposed_change": "one sentence: what the fork will change or test",
  "reason": "one sentence: why this is the highest-value next experiment",
  "comparison_reason": "one sentence: framing for comparing this fork against its parent",
  "success_criterion": "one falsifiable predicate the epoch must satisfy. Prefer structured form over portfolio receipt fields when possible: 'FIELD OP VALUE [AND FIELD OP VALUE]' where FIELD is one of: inflation_score_rhs, inflation_delta, unsupported_confident_rhs, violation_count_rhs, concept_delta, relation_delta, evidence_delta, archived_relation_delta. OP is <, >, <=, >=, ==. Example: 'inflation_score_rhs < 0.05 AND violation_count_rhs == 0'. Use natural language only when the criterion cannot be expressed in these fields.",
  "should_adopt": true|false,
  "adoption_reason": "if should_adopt=true: cite the specific success_criterion that was met and what evidence confirmed it; otherwise empty string"
}

Rules:
- proposed_change must be a concrete testable change, not a vague aspiration.
- reason must reference specific evidence from the state (tensions, stale relations, low-evidence claims, focus drift).
- success_criterion must be checkable from epoch task results — not a judgement, a measurement.
- should_adopt=true only if the epoch data already shows the success_criterion was met. Do not adopt on promise; adopt on evidence.
- Return JSON only."#;

        let other_block = match other {
            Some(o) => format!(
                "\n\nCOMPETING STATE:\n{}",
                serde_json::to_string_pretty(o)?
            ),
            None => String::new(),
        };

        let user = format!(
            "CURRENT STATE:\n{}{}\n\nAuthor the next fork proposal.",
            serde_json::to_string_pretty(current)?,
            other_block
        );

        let response = self.chat_meta(system, &user).await?;
        let parsed: LmForkProposal = serde_json::from_str(&response)
            .or_else(|_| {
                let clean = response
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or_default();

        Ok(parsed)
    }

    /// Author an epoch task list — given the current state and fork proposal, generate
    /// a concrete sequence of turns that tests the proposal's stated experiment.
    pub async fn author_epoch_tasks(
        &self,
        current: &LmStateSummary,
        proposal: &LmForkProposal,
    ) -> Result<Vec<String>> {
        let system = r#"You author task sequences for a concept graph experiment.

You are given the current state of a concept graph and a fork proposal describing what experiment to run next.
Your job is to write 10-12 concrete turn prompts that will execute the experiment, then close it with a verdict.

Rules:
- Each task is a single imperative prompt, 1-3 sentences, addressed directly to the graph system.
- Tasks must be sequenced: early tasks set up conditions, middle tasks inject stress or test claims, late tasks measure results, the final task closes the epoch with a verdict and handoff.
- Tasks must reference specific concepts, relations, or tensions from the current state — no generic instructions.
- At least two tasks must inject synthetic disconfirming evidence against a high-confidence relation.
- At least one task must audit which relations changed confidence as a result.
- The final task must: (1) explicitly check whether the proposal's success_criterion was met with a yes/no measurement, (2) state adopt/discard verdict based on that measurement, (3) name the strongest surviving relation, (4) name one new property the graph has, and (5) declare the seed tension for the next epoch.
- Return JSON only: {"tasks": ["task 1 text", "task 2 text", ...]}"#;

        let user = format!(
            "CURRENT STATE:\n{}\n\nFORK PROPOSAL:\n{}\n\nAuthor the epoch task list.",
            serde_json::to_string_pretty(current)?,
            serde_json::to_string_pretty(proposal)?,
        );

        let response = self.chat_meta(system, &user).await?;
        let parsed: serde_json::Value = serde_json::from_str(&response)
            .or_else(|_| {
                let clean = response
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or(serde_json::json!({"tasks": []}));

        let tasks: Vec<String> = parsed
            .get("tasks")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(tasks)
    }

    /// Execute a task with a default system prompt for single-shot scripts.
    pub async fn execute_task_simple(&self, task: &str) -> Result<TurnOuts> {
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

        let messages = vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": task}),
        ];

        self.execute_task(&messages).await
    }

    /// Execute a task as an agent and return the outputs.
    /// This is used for the "ACT" phase, before the "META" and "REFLECT" phases.
    pub async fn execute_task(&self, messages: &[Value]) -> Result<TurnOuts> {
        use futures::StreamExt;
        use std::io::{self, Write};

        let mut current_model = self.model.clone();
        let mut attempts = 0;
        let mut full_text = String::new();

        loop {
            attempts += 1;
            let payload = json!({
                "model": current_model,
                "messages": messages,
                "max_completion_tokens": 2048,
                "stream": true
            });

            let resp = self
                .client
                .post(&self.url)
                .bearer_auth(&self.key)
                .header("HTTP-Referer", "http://localhost:8080")
                .header("X-Title", "nstar-bit")
                .json(&payload)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    let mut stream = r.bytes_stream();
                    while let Some(chunk_res) = stream.next().await {
                        if let Ok(chunk) = chunk_res {
                            let text = String::from_utf8_lossy(&chunk);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let data = &line[6..];
                                    if data == "[DONE]" {
                                        continue;
                                    }
                                    if let Ok(v) = serde_json::from_str::<Value>(data) {
                                        if let Some(content) = v
                                            .pointer("/choices/0/delta/content")
                                            .and_then(|c| c.as_str())
                                        {
                                            if !content.is_empty() {
                                                full_text.push_str(content);
                                                eprint!("{}", content);
                                                io::stderr().flush().unwrap();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    eprintln!();
                    break;
                }
                Ok(r) if r.status().as_u16() == 429 || r.status().as_u16() == 503 => {
                    if attempts > BACKUP_MODELS.len() + 3 {
                        return Err(anyhow!("All models rate-limited after retries"));
                    }
                    let wait_secs = 1u64 << attempts.min(5);
                    tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    if attempts > 3 {
                        current_model = BACKUP_MODELS[(attempts - 4) % BACKUP_MODELS.len()].to_string();
                    }
                    continue;
                }
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    return Err(anyhow!("LM error {}: {}", status, body));
                }
                Err(e) => {
                    if attempts <= BACKUP_MODELS.len() {
                        current_model = BACKUP_MODELS[attempts - 1].to_string();
                        continue;
                    }
                    return Err(anyhow!("Network error: {}", e));
                }
            }
        }

        let parsed: Value = serde_json::from_str(&full_text)
            .or_else(|_| {
                let clean = full_text
                    .lines()
                    .filter(|l| !l.trim().starts_with("```"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&clean)
            })
            .unwrap_or(json!({
                "response": full_text,
                "actions": [],
                "quality": 0.5,
                "errors": ["Failed to parse json response"]
            }));

        let utir_operations = parsed
            .get("utir_operations")
            .and_then(|v| serde_json::from_value::<Vec<crate::utir::Operation>>(v.clone()).ok())
            .unwrap_or_default();

        let ovm_operations = parsed
            .get("ovm_operations")
            .and_then(|v| serde_json::from_value::<Vec<OvmOp>>(v.clone()).ok())
            .unwrap_or_default();

        Ok(TurnOuts {
            response: match parsed.get("response") {
                Some(v) if v.is_string() => v.as_str().unwrap().to_string(),
                Some(v) if !v.is_null() => v.to_string(),
                _ => "".to_string(),
            },
            actions: parsed
                .get("actions")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            quality: parsed
                .get("quality")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5) as f32,
            errors: parsed
                .get("errors")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            operations: utir_operations,
            ovm_operations,
        })
    }
}
