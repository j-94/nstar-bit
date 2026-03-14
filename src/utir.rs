use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Universal Task IR - safe, auditable operation vocabulary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtirDocument {
    pub task_id: String,
    pub description: String,
    pub operations: Vec<Operation>,
    #[serde(default)]
    pub policy: Option<Policy>,
    #[serde(default)]
    pub bits_tracking: Option<BitsTracking>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub gamma_gate: f64,
    pub time_ms: u64,
    pub max_risk: f64,
    pub tiny_diff_loc: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitsTracking {
    pub track_all: bool,
    #[serde(default)]
    pub custom_bits: HashMap<String, String>,
}

/// Core operation types - the engine's limited vocabulary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Operation {
    #[serde(rename = "shell")]
    Shell {
        command: String,
        #[serde(default = "default_timeout")]
        timeout: String,
        #[serde(default)]
        working_dir: Option<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        allow_network: bool,
        #[serde(default = "default_true")]
        capture_output: bool,
    },

    #[serde(rename = "fs.read")]
    FsRead {
        path: String,
        #[serde(default = "default_encoding")]
        encoding: String,
        #[serde(default = "default_max_size")]
        max_size: String,
    },

    #[serde(rename = "fs.write")]
    FsWrite {
        path: String,
        content: String,
        #[serde(default = "default_mode")]
        mode: String,
        #[serde(default)]
        create_dirs: bool,
    },

    #[serde(rename = "http.get")]
    HttpGet {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_timeout")]
        timeout: String,
        #[serde(default = "default_max_response")]
        max_response_size: String,
    },

    #[serde(rename = "git.patch")]
    GitPatch {
        repo_path: String,
        patch_content: String,
        commit_message: String,
        author: String,
    },

    #[serde(rename = "assert.file_exists")]
    AssertFileExists { path: String },

    #[serde(rename = "assert.shell_success")]
    AssertShellSuccess {
        command: String,
        #[serde(default = "default_timeout")]
        timeout: String,
        #[serde(default)]
        expected_output: Option<String>,
    },

    /// Execute an operation and always continue (success is forced to true).
    /// Useful for expected-failure safety probes and negative tests.
    #[serde(rename = "attempt")]
    Attempt { operation: Box<Operation> },

    #[serde(rename = "sequence")]
    Sequence { steps: Vec<Operation> },

    #[serde(rename = "parallel")]
    Parallel {
        steps: Vec<Operation>,
        #[serde(default = "default_concurrency")]
        max_concurrency: u32,
    },

    #[serde(rename = "conditional")]
    Conditional {
        condition: Box<Operation>,
        then_op: Box<Operation>,
        else_op: Option<Box<Operation>>,
    },

    #[serde(rename = "retry")]
    Retry {
        operation: Box<Operation>,
        #[serde(default = "default_retry_attempts")]
        max_attempts: u32,
        #[serde(default = "default_backoff")]
        backoff: String,
    },
}

/// Meta² Bits system - reflexive memory (A,U,P,E,Δ,I,R,T)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bits {
    /// Alignment - task aligns with goal
    #[serde(rename = "A")]
    pub alignment: u8,
    /// Uncertainty - confidence in result
    #[serde(rename = "U")]
    pub uncertainty: u8,
    /// Permission - human approval needed
    #[serde(rename = "P")]
    pub permission: u8,
    /// Error - something went wrong
    #[serde(rename = "E")]
    pub error: u8,
    /// Delta - context changed, need refresh
    #[serde(rename = "Δ")]
    pub delta: u8,
    /// Interrupt - external signal received
    #[serde(rename = "I")]
    pub interrupt: u8,
    /// Recovery - recovering from error
    #[serde(rename = "R")]
    pub recovery: u8,
    /// Trust - output can be trusted
    #[serde(rename = "T")]
    pub trust: u8,
}

impl Default for Bits {
    fn default() -> Self {
        Self {
            alignment: 1,
            uncertainty: 0,
            permission: 0,
            error: 0,
            delta: 0,
            interrupt: 0,
            recovery: 0,
            trust: 1,
        }
    }
}

fn default_timeout() -> String {
    "30s".to_string()
}
fn default_encoding() -> String {
    "utf-8".to_string()
}
fn default_max_size() -> String {
    "10MB".to_string()
}
fn default_mode() -> String {
    "0644".to_string()
}
fn default_max_response() -> String {
    "10MB".to_string()
}
fn default_concurrency() -> u32 {
    4
}
fn default_retry_attempts() -> u32 {
    3
}
fn default_backoff() -> String {
    "1s".to_string()
}
fn default_true() -> bool {
    true
}
