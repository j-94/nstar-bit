//! Receipts — hashed evidence of what happened.
//!
//! Every collapse produces a receipt: a cryptographic proof that
//! this specific computation occurred with these specific results.
//! Matches the receipt pattern from meta3-graph-core.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::collapse::Collapse;
use crate::gate::GateResult;

/// A receipt — proof that a turn was processed through the nstar-bit protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// Hash of the collapse
    pub collapse_hash: String,

    /// Hash of this receipt (chains with previous)
    pub receipt_hash: String,

    /// Previous receipt hash (forms a chain)
    pub prev_hash: String,

    /// Timestamp
    pub timestamp: String,

    /// Turn number
    pub turn: u64,

    /// How many predicates were active
    pub n: usize,

    /// Gate result summary
    pub gate_summary: String,

    /// Quality score
    pub quality: f32,

    /// New predicate discovered (name, or null)
    pub discovered: Option<String>,
}

impl Receipt {
    /// Create a receipt from a collapse and gate result.
    pub fn from_collapse(collapse: &Collapse, gate: &GateResult, prev_hash: &str) -> Self {
        let discovered = collapse.discovered.as_ref().map(|d| d.name.clone());

        let mut hasher = Sha256::new();
        hasher.update(collapse.hash.as_bytes());
        hasher.update(prev_hash.as_bytes());
        hasher.update(collapse.turn.to_le_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let receipt_hash = hash[..16].to_string();

        Receipt {
            collapse_hash: collapse.hash.clone(),
            receipt_hash,
            prev_hash: prev_hash.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            turn: collapse.turn,
            n: collapse.n,
            gate_summary: gate.summary(),
            quality: collapse.quality,
            discovered,
        }
    }

    /// Append this receipt to a JSONL file.
    pub fn append_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let line = serde_json::to_string(self)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}
