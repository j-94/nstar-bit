//! Receipts — hashed evidence of what happened.
//!
//! Every collapse produces a receipt: a cryptographic proof that
//! this specific computation occurred with these specific results.
//! Matches the receipt pattern from meta3-graph-core.


use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};



#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum Effect {
    #[serde(rename = "write_file")]
    WriteFile {
        path: String,
        bytes: usize,
        sha256: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "read_file")]
    ReadFile {
        path: String,
        bytes: usize,
        sha256: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "http_get")]
    HttpGet {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
        bytes: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        sha256: Option<String>,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "git_patch")]
    GitPatch {
        repo_path: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "assert")]
    Assert {
        assert_kind: String,
        ok: bool,
        message: String,
    },
    #[serde(rename = "blocked")]
    Blocked {
        op: String,
        reason: String,
    },
    #[serde(rename = "exec")]
    Exec {
        cmd: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdout_sha256: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stderr_sha256: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn sha256_hex_str(s: &str) -> String {
    sha256_hex_bytes(s.as_bytes())
}


