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

const BACKUP_MODELS: &[&str] = &[
    "anthropic/claude-sonnet-4-6",
    "anthropic/claude-opus-4-6",
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
            key,
        })
    }

    /// Raw chat completion — send messages, get text back.
    pub async fn chat_raw(&self, system: &str, user: &str) -> Result<String> {
        let messages = vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": user}),
        ];

        // Print header for the metacognitive process
        println!("  [Meta Pass] Thinking... ");

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
                                                print!("{}", content);
                                                io::stdout().flush().unwrap();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    println!("\x1b[0m\n"); // Reset color and add newline
                    return Ok(full_text);
                }
                Ok(r) if r.status().as_u16() == 429 || r.status().as_u16() == 503 => {
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
                                                print!("{}", content);
                                                io::stdout().flush().unwrap();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    println!("\n"); // Format newline at end of stream
                    break;
                }
                Ok(r) if r.status().as_u16() == 429 || r.status().as_u16() == 503 => {
                    if attempts > BACKUP_MODELS.len() + 3 {
                        return Err(anyhow!("All models rate-limited after retries"));
                    }
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
