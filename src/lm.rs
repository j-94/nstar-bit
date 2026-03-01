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

use crate::collapse::{TurnIns, TurnOuts};
use crate::predicate::{GateType, Predicate};

// ── Config (env-driven, same as one-engine) ──

const DEFAULT_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_MODEL: &str = "google/gemini-3-pro-preview";

const BACKUP_MODELS: &[&str] = &[
    "google/gemini-pro-1.5",
    "anthropic/claude-3.7-sonnet",
    "openai/gpt-4o",
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
    for key in &["ROUTER_API_KEY", "OPENROUTER_API_KEY"] {
        if let Ok(val) = std::env::var(key) {
            if !val.trim().is_empty() {
                return Some(val);
            }
        }
    }

    // Fallback: try macOS keychain
    if cfg!(target_os = "macos") {
        let output = std::process::Command::new("security")
            .args(&["find-generic-password", "-s", "OPENROUTER_API_KEY", "-w"])
            .output()
            .ok()?;
        
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !val.is_empty() {
                return Some(val);
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
    pub gate_type: String,
    pub threshold: f32,
    pub reason: String,
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

        let mut current_model = self.model.clone();
        let mut attempts = 0;

        loop {
            attempts += 1;
            let payload = json!({
                "model": current_model,
                "messages": messages,
                "temperature": 0.1,
                "response_format": { "type": "json_object" }
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
                    let body: Value = r.json().await?;
                    let content = body
                        .pointer("/choices/0/message/content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}")
                        .to_string();
                    return Ok(content);
                }
                Ok(r) if r.status().as_u16() == 429 || r.status().as_u16() == 503 => {
                    if attempts <= BACKUP_MODELS.len() {
                        current_model = BACKUP_MODELS[attempts - 1].to_string();
                        tokio::time::sleep(Duration::from_millis(500)).await;
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
                        tokio::time::sleep(Duration::from_millis(500)).await;
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

        let system = r#"You evaluate metacognitive predicates. Given a set of predicates with activation conditions, and a turn's input/output, score each predicate 0.0 to 1.0 based on how strongly the condition holds.

Return JSON: {"evaluations": [{"name": "...", "activation": 0.0-1.0, "reason": "brief"}]}"#;

        let user = format!(
            "PREDICATES:\n{}\n\nTURN INPUT:\n{}\n\nTURN OUTPUT:\n{}\n\nERRORS: {}\n\nScore each predicate.",
            serde_json::to_string_pretty(&predicate_list)?,
            ins.prompt,
            outs.response,
            outs.errors.join("; ")
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

        let evals: Vec<PredicateEvaluation> = parsed
            .get("evaluations")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

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

        let system = r#"You are a metacognitive reflection agent. After a turn completes, you analyze whether the existing predicates were sufficient or if a new dimension of awareness is needed.

Rules:
- Only propose a new predicate if the turn revealed a GENUINE failure mode not covered by existing predicates.
- Be conservative. Most turns don't need new predicates.
- If the turn went well, reinforce the predicates that helped (list their names).
- Rate the turn quality 0.0 to 1.0.

Return JSON:
{
  "turn_quality": 0.0-1.0,
  "new_predicate": null | {"name": "...", "activation_condition": "...", "gate_type": "halt|verify|escalate|simulate|none", "threshold": 0.0-1.0, "reason": "..."},
  "reinforced": ["predicate_names_that_helped"],
  "reasoning": "brief explanation"
}"#;

        let user = format!(
            "EXISTING PREDICATES: {:?}\n\nTURN INPUT:\n{}\n\nTURN OUTPUT:\n{}\n\nQUALITY SELF-ASSESSMENT: {}\nERRORS: {}\n\nReflect on this turn.",
            current_names,
            ins.prompt,
            outs.response,
            outs.quality,
            outs.errors.join("; ")
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

        Ok(ReflectionResult {
            turn_quality: parsed
                .get("turn_quality")
                .and_then(|v| v.as_f64())
                .unwrap_or(outs.quality as f64) as f32,
            new_predicate: parsed
                .get("new_predicate")
                .and_then(|v| {
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
        })
    }

    /// Execute a task as an agent and return the outputs.
    /// This is used for the "ACT" phase, before the "META" and "REFLECT" phases.
    pub async fn execute_task(&self, task: &str) -> Result<TurnOuts> {
        let system = r#"You are an AI assistant. Complete the user's task.
Perform the task, and then self-assess your performance.
If you are unable to complete the task perfectly, note the issues in the "errors" array.
Return JSON:
{
  "response": "your detailed response to the task",
  "actions": ["hypothetical actions taken, e.g. 'read file X'"],
  "quality": 0.0-1.0,
  "errors": ["any errors, missing information, or uncertainty"]
}"#;

        let response = self.chat_raw(system, task).await?;
        
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
                "response": response,
                "actions": [],
                "quality": 0.5,
                "errors": ["Failed to parse json response"]
            }));

        Ok(TurnOuts {
            response: parsed.get("response").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            actions: parsed.get("actions").and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            quality: parsed.get("quality").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
            errors: parsed.get("errors").and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
        })
    }
}

/// Convert an LM-proposed gate type string to our GateType enum.
pub fn parse_gate_type(s: &str) -> GateType {
    match s.to_lowercase().as_str() {
        "halt" => GateType::Halt,
        "verify" => GateType::Verify,
        "escalate" => GateType::Escalate,
        "simulate" => GateType::Simulate,
        _ => GateType::None,
    }
}
