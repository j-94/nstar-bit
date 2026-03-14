//! Reducible Map Engine
//!
//! Finds the minimal subgraph that preserves all evidence-backed confidence
//! scores. Three reduction classes:
//!
//!   AliasCollapse  — concepts with identical support sets merge into one
//!   ChainReduce    — A→B→C where B has no other connections becomes A→C
//!   GhostPrune     — nodes with zero dialogue evidence below mention floor
//!
//! Every reduction produces a `ReductionReceipt`. If any downstream
//! confidence delta exceeds `delta_threshold` the operation is aborted and
//! the receipt is written with `aborted = true`. Nothing is mutated until
//! the snapshot comparison passes.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::autogenesis::State;

// ── public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ReductionOp {
    AliasCollapse {
        /// The concept that survives.
        canonical: String,
        /// Concepts being absorbed into canonical.
        absorbed: Vec<String>,
    },
    ChainReduce {
        source: String,
        /// The intermediary node being removed (B in A→B→C).
        intermediary: String,
        target: String,
        /// Evidence IDs transferred from the two collapsed relations.
        inherited_evidence: Vec<String>,
        /// The new direct relation id that replaces the two-hop path.
        new_relation_id: String,
    },
    GhostPrune {
        concept: String,
        reason: GhostReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GhostReason {
    NoDialogueEvidence,
    BelowMentionThreshold { count: u64, threshold: u64 },
    StaleAccess { last_turn: u64, current_turn: u64, max_age: u64 },
}

/// Snapshot of all active relation confidence scores at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceSnapshot {
    /// relation_id → confidence at snapshot time.
    pub scores: HashMap<String, f64>,
    pub turn: u64,
}

/// Cryptographically identified record of one reduction attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReductionReceipt {
    /// SHA-256 of (before_snapshot JSON + operation JSON).
    pub reduction_id: String,
    pub operation: ReductionOp,
    pub before: ConfidenceSnapshot,
    pub after: ConfidenceSnapshot,
    /// Maximum |before - after| across all shared relation ids.
    pub max_delta: f64,
    /// True if max_delta > delta_threshold; state was NOT mutated.
    pub aborted: bool,
    pub abort_reason: Option<String>,
    pub ts: u64,
}

/// A candidate reduction with its rationale, before the engine commits it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReductionCandidate {
    pub op: ReductionOp,
    pub rationale: String,
}

// ── engine ────────────────────────────────────────────────────────────────────

pub struct ReducibleMapEngine {
    /// Abort threshold: any confidence delta > this aborts the reduction.
    pub delta_threshold: f64,
    /// Ghost prune: concepts with mention_count < this are candidates.
    pub ghost_mention_floor: u64,
    /// Ghost prune: concepts not seen for this many turns are candidates.
    pub ghost_age_floor: u64,
    /// If true, scan() returns candidates but execute() always aborts.
    pub dry_run: bool,
}

impl Default for ReducibleMapEngine {
    fn default() -> Self {
        Self {
            delta_threshold: 0.05,
            ghost_mention_floor: 3,
            ghost_age_floor: 50,
            dry_run: false,
        }
    }
}

impl ReducibleMapEngine {
    /// Scan state for reduction candidates. Does not mutate state.
    pub fn scan(&self, state: &State) -> Vec<ReductionCandidate> {
        let mut candidates: Vec<ReductionCandidate> = Vec::new();

        candidates.extend(self.scan_alias_collapse(state));
        candidates.extend(self.scan_chain_reduce(state));
        candidates.extend(self.scan_ghost_prune(state));

        candidates
    }

    /// Execute a single reduction. Returns a receipt.
    /// On abort (delta > threshold or dry_run), state is unchanged.
    pub fn execute(&self, state: &mut State, op: ReductionOp) -> ReductionReceipt {
        let before = self.snapshot(state);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let reduction_id = {
            let before_json = serde_json::to_string(&before).unwrap_or_default();
            let op_json = serde_json::to_string(&op).unwrap_or_default();
            let mut hasher = Sha256::new();
            hasher.update(before_json.as_bytes());
            hasher.update(op_json.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        if self.dry_run {
            return ReductionReceipt {
                reduction_id,
                operation: op,
                before: before.clone(),
                after: before,
                max_delta: 0.0,
                aborted: true,
                abort_reason: Some("dry_run".into()),
                ts,
            };
        }

        // Apply operation to a clone first, then snapshot the clone.
        let mut candidate_state = state.clone();
        let apply_result = self.apply_op(&mut candidate_state, &op);
        if let Err(e) = apply_result {
            return ReductionReceipt {
                reduction_id,
                operation: op,
                before: before.clone(),
                after: before,
                max_delta: 0.0,
                aborted: true,
                abort_reason: Some(format!("apply error: {e}")),
                ts,
            };
        }

        let after = self.snapshot(&candidate_state);
        let max_delta = self.max_delta(&before, &after);

        if max_delta > self.delta_threshold {
            return ReductionReceipt {
                reduction_id,
                operation: op,
                before,
                after,
                max_delta,
                aborted: true,
                abort_reason: Some(format!(
                    "max_delta {max_delta:.4} exceeds threshold {:.4}",
                    self.delta_threshold
                )),
                ts,
            };
        }

        // Delta is safe — commit the candidate state.
        *state = candidate_state;

        ReductionReceipt {
            reduction_id,
            operation: op,
            before,
            after,
            max_delta,
            aborted: false,
            abort_reason: None,
            ts,
        }
    }

    // ── snapshot ──────────────────────────────────────────────────────────────

    fn snapshot(&self, state: &State) -> ConfidenceSnapshot {
        let scores = state
            .relations
            .iter()
            .filter(|(_, r)| r.status != "archived")
            .map(|(id, r)| (id.clone(), r.confidence as f64))
            .collect();
        ConfidenceSnapshot { scores, turn: state.turn }
    }

    fn max_delta(&self, before: &ConfidenceSnapshot, after: &ConfidenceSnapshot) -> f64 {
        // Compare scores for all relations that exist in both snapshots.
        before
            .scores
            .iter()
            .filter_map(|(id, &b)| {
                after.scores.get(id).map(|&a| (b - a).abs())
            })
            .fold(0.0_f64, f64::max)
    }

    // ── scan passes ───────────────────────────────────────────────────────────

    fn scan_alias_collapse(&self, state: &State) -> Vec<ReductionCandidate> {
        // Group active concepts by their support_set signature.
        // Two concepts are alias candidates if every relation involving one
        // is also present (same source/target/relation type) for the other.
        let active_concepts: Vec<&str> = state
            .concepts
            .iter()
            .filter(|(_, c)| c.status != "archived")
            .map(|(id, _)| id.as_str())
            .collect();

        // Build per-concept support fingerprint: sorted set of evidence IDs
        // from all relations where this concept is source or target.
        let mut fingerprints: HashMap<&str, Vec<String>> = HashMap::new();
        for cid in &active_concepts {
            let mut ev_ids: Vec<String> = state
                .relations
                .values()
                .filter(|r| r.status != "archived")
                .filter(|r| r.source.as_str() == *cid || r.target.as_str() == *cid)
                .flat_map(|r| r.support_set.iter().cloned())
                .collect();
            ev_ids.sort();
            ev_ids.dedup();
            fingerprints.insert(cid, ev_ids);
        }

        // Find groups with identical, non-empty fingerprints.
        let mut groups: HashMap<Vec<String>, Vec<&str>> = HashMap::new();
        for (cid, fp) in &fingerprints {
            if !fp.is_empty() {
                groups.entry(fp.clone()).or_default().push(cid);
            }
        }

        let mut candidates = Vec::new();
        for (_, group) in groups {
            if group.len() < 2 {
                continue;
            }
            // Canonical = highest mention_count.
            let canonical = group
                .iter()
                .max_by_key(|&&id| {
                    state.concepts.get(id).map(|c| c.mention_count).unwrap_or(0)
                })
                .copied()
                .unwrap_or(group[0]);

            let absorbed: Vec<String> = group
                .iter()
                .filter(|&&id| id != canonical)
                .map(|&id| id.to_string())
                .collect();

            candidates.push(ReductionCandidate {
                op: ReductionOp::AliasCollapse {
                    canonical: canonical.to_string(),
                    absorbed: absorbed.clone(),
                },
                rationale: format!(
                    "{} shares identical support set with {}",
                    canonical,
                    absorbed.join(", ")
                ),
            });
        }
        candidates
    }

    fn scan_chain_reduce(&self, state: &State) -> Vec<ReductionCandidate> {
        // Find concept B where:
        //   - exactly one active relation has B as target (A→B)
        //   - exactly one active relation has B as source (B→C)
        //   - B has no dialogue evidence of its own
        //   - B has no aliases pointing to it from outside A and C
        let active_rels: Vec<&crate::autogenesis::RelationRecord> = state
            .relations
            .values()
            .filter(|r| r.status != "archived")
            .collect();

        let dialogue_ids: HashSet<&str> = state
            .evidence_log
            .iter()
            .filter(|e| e.origin == "dialogue")
            .map(|e| e.id.as_str())
            .collect();

        let mut candidates = Vec::new();

        for concept_id in state.concepts.keys() {
            let c = match state.concepts.get(concept_id) {
                Some(c) => c,
                None => continue,
            };
            if c.status == "archived" {
                continue;
            }

            let incoming: Vec<&crate::autogenesis::RelationRecord> = active_rels
                .iter()
                .filter(|r| &r.target == concept_id)
                .copied()
                .collect();
            let outgoing: Vec<&crate::autogenesis::RelationRecord> = active_rels
                .iter()
                .filter(|r| &r.source == concept_id)
                .copied()
                .collect();

            if incoming.len() != 1 || outgoing.len() != 1 {
                continue;
            }

            let a_to_b = incoming[0];
            let b_to_c = outgoing[0];

            // B must not have dialogue-originated evidence directly.
            let b_has_dialogue = a_to_b
                .support_set
                .iter()
                .chain(b_to_c.support_set.iter())
                .any(|ev_id| dialogue_ids.contains(ev_id.as_str()));
            // Only reduce if B is a pure structural relay with no dialogue grounding.
            if b_has_dialogue {
                continue;
            }

            let inherited: Vec<String> = a_to_b
                .support_set
                .iter()
                .chain(b_to_c.support_set.iter())
                .cloned()
                .collect();

            let new_relation_id = format!(
                "reduced_{}_{}",
                a_to_b.source, b_to_c.target
            );

            candidates.push(ReductionCandidate {
                op: ReductionOp::ChainReduce {
                    source: a_to_b.source.clone(),
                    intermediary: concept_id.clone(),
                    target: b_to_c.target.clone(),
                    inherited_evidence: inherited,
                    new_relation_id,
                },
                rationale: format!(
                    "{}→{}→{}: intermediary has no dialogue evidence and no other connections",
                    a_to_b.source, concept_id, b_to_c.target
                ),
            });
        }
        candidates
    }

    fn scan_ghost_prune(&self, state: &State) -> Vec<ReductionCandidate> {
        let dialogue_ids: HashSet<&str> = state
            .evidence_log
            .iter()
            .filter(|e| e.origin == "dialogue")
            .map(|e| e.id.as_str())
            .collect();

        // Which concepts are referenced by ANY active relation?
        let referenced: HashSet<&str> = state
            .relations
            .values()
            .filter(|r| r.status != "archived")
            .flat_map(|r| [r.source.as_str(), r.target.as_str()])
            .collect();

        let mut candidates = Vec::new();

        for (id, concept) in &state.concepts {
            if concept.status == "archived" {
                continue;
            }

            // Collect all evidence IDs that touch this concept (via its relations).
            let concept_ev_ids: HashSet<&str> = state
                .relations
                .values()
                .filter(|r| r.status != "archived" && (&r.source == id || &r.target == id))
                .flat_map(|r| r.support_set.iter().map(|s| s.as_str()))
                .collect();

            let has_dialogue = concept_ev_ids.iter().any(|ev_id| dialogue_ids.contains(ev_id));

            // Ghost class 1: no dialogue evidence at all.
            if !has_dialogue && !referenced.contains(id.as_str()) {
                candidates.push(ReductionCandidate {
                    op: ReductionOp::GhostPrune {
                        concept: id.clone(),
                        reason: GhostReason::NoDialogueEvidence,
                    },
                    rationale: format!(
                        "{} has zero dialogue evidence and no active relation references",
                        id
                    ),
                });
                continue;
            }

            // Ghost class 2: below mention threshold.
            if concept.mention_count < self.ghost_mention_floor && !has_dialogue {
                candidates.push(ReductionCandidate {
                    op: ReductionOp::GhostPrune {
                        concept: id.clone(),
                        reason: GhostReason::BelowMentionThreshold {
                            count: concept.mention_count,
                            threshold: self.ghost_mention_floor,
                        },
                    },
                    rationale: format!(
                        "{} mention_count={} < floor={}",
                        id, concept.mention_count, self.ghost_mention_floor
                    ),
                });
                continue;
            }

            // Ghost class 3: stale access.
            if concept.last_seen_turn + self.ghost_age_floor < state.turn && !has_dialogue {
                candidates.push(ReductionCandidate {
                    op: ReductionOp::GhostPrune {
                        concept: id.clone(),
                        reason: GhostReason::StaleAccess {
                            last_turn: concept.last_seen_turn,
                            current_turn: state.turn,
                            max_age: self.ghost_age_floor,
                        },
                    },
                    rationale: format!(
                        "{} last seen at turn {} ({} turns ago)",
                        id,
                        concept.last_seen_turn,
                        state.turn.saturating_sub(concept.last_seen_turn)
                    ),
                });
            }
        }
        candidates
    }

    // ── apply ─────────────────────────────────────────────────────────────────

    fn apply_op(&self, state: &mut State, op: &ReductionOp) -> Result<()> {
        match op {
            ReductionOp::AliasCollapse { canonical, absorbed } => {
                self.apply_alias_collapse(state, canonical, absorbed)
            }
            ReductionOp::ChainReduce {
                source,
                intermediary,
                target,
                inherited_evidence,
                new_relation_id,
            } => self.apply_chain_reduce(
                state,
                source,
                intermediary,
                target,
                inherited_evidence,
                new_relation_id,
            ),
            ReductionOp::GhostPrune { concept, .. } => {
                self.apply_ghost_prune(state, concept)
            }
        }
    }

    fn apply_alias_collapse(
        &self,
        state: &mut State,
        canonical: &str,
        absorbed: &[String],
    ) -> Result<()> {
        for abs_id in absorbed {
            // Redirect all relations pointing to/from absorbed → canonical.
            let rel_ids: Vec<String> = state.relations.keys().cloned().collect();
            for rel_id in rel_ids {
                if let Some(r) = state.relations.get_mut(&rel_id) {
                    if r.source == *abs_id {
                        r.source = canonical.to_string();
                    }
                    if r.target == *abs_id {
                        r.target = canonical.to_string();
                    }
                }
            }
            // Transfer aliases.
            if let Some(abs_concept) = state.concepts.get(abs_id) {
                let label = abs_concept.label.clone();
                let mc = abs_concept.mention_count;
                state.aliases.insert(abs_id.clone(), canonical.to_string());
                if let Some(can) = state.concepts.get_mut(canonical) {
                    can.aliases.push(abs_id.clone());
                    can.mention_count += mc;
                }
                // Suppress label to aliases list so it's not lost.
                if let Some(can) = state.concepts.get_mut(canonical) {
                    if !label.is_empty() && !can.aliases.contains(&label) {
                        can.aliases.push(label);
                    }
                }
            }
            // Archive the absorbed concept.
            if let Some(abs_concept) = state.concepts.get_mut(abs_id) {
                abs_concept.status = "archived".to_string();
            }
        }
        Ok(())
    }

    fn apply_chain_reduce(
        &self,
        state: &mut State,
        source: &str,
        intermediary: &str,
        target: &str,
        inherited_evidence: &[String],
        new_relation_id: &str,
    ) -> Result<()> {
        // Archive the two relations A→B and B→C.
        let rel_ids: Vec<String> = state.relations.keys().cloned().collect();
        let mut combined_ef = 0u64;
        let mut combined_ea = 0u64;
        for rel_id in &rel_ids {
            if let Some(r) = state.relations.get_mut(rel_id) {
                let is_ab = r.source == source && r.target == intermediary;
                let is_bc = r.source == intermediary && r.target == target;
                if is_ab || is_bc {
                    combined_ef += r.evidence_for;
                    combined_ea += r.evidence_against;
                    r.status = "archived".to_string();
                }
            }
        }

        // Create the direct A→C relation inheriting all evidence.
        let total = combined_ef + combined_ea;
        let new_conf = if total > 0 {
            combined_ef as f32 / total as f32
        } else {
            0.0
        };
        let new_rel = crate::autogenesis::RelationRecord {
            id: new_relation_id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            relation: "reduced_chain".to_string(),
            status: "active".to_string(),
            rationale: format!("chain reduction: {source}→{intermediary}→{target}"),
            confidence: new_conf,
            evidence_for: combined_ef,
            evidence_against: combined_ea,
            support_set: inherited_evidence.to_vec(),
            first_seen_turn: state.turn,
            last_updated_turn: state.turn,
            last_evidence_id: String::new(),
            last_confidence_delta: 0.0,
        };
        state.relations.insert(new_relation_id.to_string(), new_rel);

        // Archive the intermediary concept.
        if let Some(c) = state.concepts.get_mut(intermediary) {
            c.status = "archived".to_string();
        }
        Ok(())
    }

    fn apply_ghost_prune(&self, state: &mut State, concept: &str) -> Result<()> {
        // Archive the concept and all its relations.
        if let Some(c) = state.concepts.get_mut(concept) {
            c.status = "archived".to_string();
        }
        for r in state.relations.values_mut() {
            if r.source == concept || r.target == concept {
                r.status = "archived".to_string();
            }
        }
        Ok(())
    }
}

// ── append-only receipt log ───────────────────────────────────────────────────

pub fn append_receipt(receipts_path: &str, receipt: &ReductionReceipt) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(receipts_path)?;
    writeln!(f, "{}", serde_json::to_string(receipt)?)?;
    Ok(())
}
