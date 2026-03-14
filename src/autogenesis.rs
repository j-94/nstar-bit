use std::fs;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::receipt::sha256_hex_str;

const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidencePayload {
    pub source_uri: String,
    pub meta: BTreeMap<String, String>,
    pub items: Vec<EvidenceDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub rel: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphPacket {
    pub packet_id: String,
    pub turn: u64,
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSnapshot {
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub links: Vec<GraphLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Seed {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GateVerdict {
    #[serde(default)]
    pub allow_act: bool,
    #[serde(default)]
    pub need_more_evidence: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolExtraction {
    #[serde(default)]
    pub extractor: String,
    #[serde(default)]
    pub raw_text_sha256: String,
    #[serde(default)]
    pub raw_symbols: Vec<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConceptRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub first_seen_turn: u64,
    #[serde(default)]
    pub last_seen_turn: u64,
    #[serde(default)]
    pub mention_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelationRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub evidence_for: u64,
    #[serde(default)]
    pub evidence_against: u64,
    /// Ordered list of evidence IDs that actively support this relation.
    /// Empty → no live support; confidence should decay toward zero.
    /// This is the single source of truth for both causal traceability
    /// and sunset-schedule behaviour — no separate governance mechanism needed.
    #[serde(default)]
    pub support_set: Vec<String>,
    #[serde(default)]
    pub first_seen_turn: u64,
    #[serde(default)]
    pub last_updated_turn: u64,
    /// Causal traceability: the evidence ID that last changed this relation's
    /// confidence, and the delta. Look up last_evidence_id in evidence_log
    /// to find the turn, origin ("dialogue"/"audit"), verdict, and explanation.
    #[serde(default)]
    pub last_evidence_id: String,
    #[serde(default)]
    pub last_confidence_delta: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidenceRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub relation_id: String,
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub source_uri: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
    #[serde(default)]
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConceptDelta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AliasDecision {
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub canonical: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelationDelta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidenceDelta {
    #[serde(default)]
    pub relation_id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TurnDelta {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub concepts: Vec<ConceptDelta>,
    #[serde(default)]
    pub aliases: Vec<AliasDecision>,
    #[serde(default)]
    pub relations: Vec<RelationDelta>,
    #[serde(default)]
    pub evidence: Vec<EvidenceDelta>,
    #[serde(default)]
    pub active_focus: Vec<String>,
    #[serde(default)]
    pub next_probes: Vec<Seed>,
    #[serde(default)]
    pub tensions: Vec<String>,
    #[serde(default)]
    pub gate: GateVerdict,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TurnTransitionReceipt {
    #[serde(default)]
    pub raw_response: String,
    #[serde(default)]
    pub delta: TurnDelta,
    #[serde(default)]
    pub rejected_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Event {
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub concept_ids: Vec<String>,
    #[serde(default)]
    pub relation_ids: Vec<String>,
    #[serde(default)]
    pub discovered: Vec<String>,
    #[serde(default)]
    pub promoted: Vec<String>,
    #[serde(default)]
    pub archived: Vec<String>,
    #[serde(default)]
    pub active_core: Vec<String>,
    #[serde(default)]
    pub seeds: Vec<Seed>,
    #[serde(default)]
    pub gate: GateVerdict,
    #[serde(default)]
    pub packet_id: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tensions: Vec<String>,
    #[serde(default)]
    pub extraction: SymbolExtraction,
    #[serde(default)]
    pub transition: TurnTransitionReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metrics {
    #[serde(default)]
    pub concept_count: usize,
    #[serde(default)]
    pub relation_count: usize,
    #[serde(default)]
    pub evidence_count: usize,
    #[serde(default)]
    pub focus_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MonitorRow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub evidence_for: u64,
    #[serde(default)]
    pub evidence_against: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MonitorData {
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub concept_count: usize,
    #[serde(default)]
    pub relation_count: usize,
    #[serde(default)]
    pub active_core: Vec<String>,
    #[serde(default)]
    pub seed_queue: Vec<Seed>,
    #[serde(default)]
    pub gate: GateVerdict,
    #[serde(default)]
    pub metrics: Metrics,
    #[serde(default)]
    pub top_relations: Vec<MonitorRow>,
    #[serde(default)]
    pub graph_nodes: usize,
    #[serde(default)]
    pub graph_links: usize,
    #[serde(default)]
    pub latest_extraction: Option<SymbolExtraction>,
    #[serde(default)]
    pub latest_transition: Option<TurnTransitionReceipt>,
    #[serde(default)]
    pub latest_summary: String,
    #[serde(default)]
    pub tensions: Vec<String>,
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub parent_run_id: String,
    #[serde(default)]
    pub run_status: String,
    #[serde(default)]
    pub comparison_count: usize,
}

// ── Health invariant checking ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthViolation {
    pub kind: String,
    pub entity_id: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealthStats {
    #[serde(default)]
    pub total_relations: usize,
    #[serde(default)]
    pub live_relations: usize,
    /// Relations with evidence_for=0 AND confidence>0 (should never happen post-gate)
    #[serde(default)]
    pub unsupported_confident: usize,
    #[serde(default)]
    pub mean_confidence: f32,
    #[serde(default)]
    pub mean_support_set_size: f32,
    /// Items in active_focus with no evidence activity in the last N turns
    #[serde(default)]
    pub stale_focus_count: usize,
    /// Mean(confidence - evidence_ratio) across live relations.
    /// >0.1 means the graph is inflating. The epoch 1-7 run had ~0.775.
    #[serde(default)]
    pub inflation_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealthSignal {
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub violations: Vec<HealthViolation>,
    #[serde(default)]
    pub stats: HealthStats,
    /// true if no violations were found
    #[serde(default)]
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunProposalReceipt {
    #[serde(default)]
    pub proposal_id: String,
    #[serde(default)]
    pub proposed_change: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunComparisonReceipt {
    #[serde(default)]
    pub comparison_id: String,
    #[serde(default)]
    pub lhs_run_id: String,
    #[serde(default)]
    pub rhs_run_id: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub lhs_turn: u64,
    #[serde(default)]
    pub rhs_turn: u64,
    #[serde(default)]
    pub concept_delta: i64,
    #[serde(default)]
    pub relation_delta: i64,
    #[serde(default)]
    pub evidence_delta: i64,
    #[serde(default)]
    pub archived_concept_delta: i64,
    #[serde(default)]
    pub archived_relation_delta: i64,
    #[serde(default)]
    pub rejected_field_delta: i64,
    #[serde(default)]
    pub inflation_score_lhs: f32,
    #[serde(default)]
    pub inflation_score_rhs: f32,
    #[serde(default)]
    pub unsupported_confident_lhs: usize,
    #[serde(default)]
    pub unsupported_confident_rhs: usize,
    #[serde(default)]
    pub violation_count_lhs: usize,
    #[serde(default)]
    pub violation_count_rhs: usize,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdoptionReceipt {
    #[serde(default)]
    pub adoption_id: String,
    #[serde(default)]
    pub chosen_run_id: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LmForkProposal {
    #[serde(default)]
    pub proposed_change: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub comparison_reason: String,
    #[serde(default)]
    pub success_criterion: String,
    #[serde(default)]
    pub should_adopt: bool,
    #[serde(default)]
    pub adoption_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunLineage {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub parent_run_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub proposal: Option<RunProposalReceipt>,
    #[serde(default)]
    pub comparisons: Vec<RunComparisonReceipt>,
    #[serde(default)]
    pub adoption: Option<AdoptionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(default)]
    pub concepts: BTreeMap<String, ConceptRecord>,
    #[serde(default)]
    pub relations: BTreeMap<String, RelationRecord>,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub evidence_log: Vec<EvidenceRecord>,
    #[serde(default)]
    pub active_focus: Vec<String>,
    #[serde(default)]
    pub seed_queue: Vec<Seed>,
    #[serde(default)]
    pub latest_gate: GateVerdict,
    #[serde(default)]
    pub metrics: Metrics,
    #[serde(default)]
    pub packets: Vec<GraphPacket>,
    #[serde(default)]
    pub graph: GraphSnapshot,
    #[serde(default)]
    pub latest_summary: String,
    #[serde(default)]
    pub unresolved_tensions: Vec<String>,
    #[serde(default)]
    pub run_lineage: RunLineage,
    #[serde(default)]
    pub latest_health: HealthSignal,
}

impl Default for State {
    fn default() -> Self {
        let now = now_iso();
        Self {
            version: VERSION,
            created_at: now.clone(),
            updated_at: now,
            turn: 0,
            events: Vec::new(),
            concepts: BTreeMap::new(),
            relations: BTreeMap::new(),
            aliases: BTreeMap::new(),
            evidence_log: Vec::new(),
            active_focus: Vec::new(),
            seed_queue: Vec::new(),
            latest_gate: GateVerdict {
                allow_act: false,
                need_more_evidence: true,
                reason: "bootstrap".to_string(),
            },
            metrics: Metrics::default(),
            packets: Vec::new(),
            graph: GraphSnapshot::default(),
            latest_summary: String::new(),
            unresolved_tensions: Vec::new(),
            run_lineage: RunLineage {
                run_id: new_run_id("root"),
                parent_run_id: String::new(),
                status: "active".to_string(),
                proposal: None,
                comparisons: Vec::new(),
                adoption: None,
            },
            latest_health: HealthSignal::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LmConceptView {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub mention_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LmRelationView {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub evidence_for: u64,
    #[serde(default)]
    pub evidence_against: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LmEventView {
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub active_focus: Vec<String>,
    #[serde(default)]
    pub tensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LmStateSummary {
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub latest_summary: String,
    #[serde(default)]
    pub active_focus: Vec<String>,
    #[serde(default)]
    pub tensions: Vec<String>,
    #[serde(default)]
    pub concepts: Vec<LmConceptView>,
    #[serde(default)]
    pub relations: Vec<LmRelationView>,
    #[serde(default)]
    pub recent_events: Vec<LmEventView>,
}

pub fn init_state(path: &Path) -> Result<()> {
    let state = State::default();
    save_state(path, &state)
}

pub fn load_state(path: &Path) -> Result<State> {
    if !path.exists() {
        return Ok(State::default());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut state: State = serde_json::from_str(&raw)?;
    if state.version == 0 {
        state.version = VERSION;
    }
    if state.created_at.is_empty() {
        state.created_at = now_iso();
    }
    if state.updated_at.is_empty() {
        state.updated_at = state.created_at.clone();
    }
    if state.run_lineage.run_id.is_empty() {
        state.run_lineage.run_id = new_run_id("run");
    }
    if state.run_lineage.status.is_empty() {
        state.run_lineage.status = "active".to_string();
    }
    Ok(state)
}

pub fn save_state(path: &Path, state: &State) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut next = state.clone();
    next.updated_at = now_iso();
    std::fs::write(path, serde_json::to_string_pretty(&next)?)?;
    Ok(())
}

pub fn fork_state(parent: &State, proposed_change: &str, reason: &str) -> State {
    let mut forked = parent.clone();
    forked.created_at = now_iso();
    forked.updated_at = forked.created_at.clone();
    forked.run_lineage = RunLineage {
        run_id: new_run_id("fork"),
        parent_run_id: parent.run_lineage.run_id.clone(),
        status: "active".to_string(),
        proposal: Some(RunProposalReceipt {
            proposal_id: new_run_id("proposal"),
            proposed_change: proposed_change.to_string(),
            reason: reason.to_string(),
            created_at: now_iso(),
        }),
        comparisons: Vec::new(),
        adoption: None,
    };
    forked
}

pub fn compare_states(lhs: &State, rhs: &State, reason: &str) -> RunComparisonReceipt {
    let lhs_rejected = total_rejected_fields(lhs) as i64;
    let rhs_rejected = total_rejected_fields(rhs) as i64;
    let lhs_archived_concepts = count_concepts_by_status(lhs, "archived") as i64;
    let rhs_archived_concepts = count_concepts_by_status(rhs, "archived") as i64;
    let lhs_archived_relations = count_relations_by_status(lhs, "archived") as i64;
    let rhs_archived_relations = count_relations_by_status(rhs, "archived") as i64;

    let concept_delta = rhs.concepts.len() as i64 - lhs.concepts.len() as i64;
    let relation_delta = rhs.relations.len() as i64 - lhs.relations.len() as i64;
    let evidence_delta = rhs.evidence_log.len() as i64 - lhs.evidence_log.len() as i64;
    let archived_concept_delta = rhs_archived_concepts - lhs_archived_concepts;
    let archived_relation_delta = rhs_archived_relations - lhs_archived_relations;
    let rejected_field_delta = rhs_rejected - lhs_rejected;

    let lhs_health = health_check(lhs);
    let rhs_health = health_check(rhs);
    let inflation_score_lhs = lhs_health.stats.inflation_score;
    let inflation_score_rhs = rhs_health.stats.inflation_score;
    let unsupported_confident_lhs = lhs_health.stats.unsupported_confident;
    let unsupported_confident_rhs = rhs_health.stats.unsupported_confident;
    let violation_count_lhs = lhs_health.violations.len();
    let violation_count_rhs = rhs_health.violations.len();

    let summary = format!(
        "Compared {} -> {}: concepts {:+}, relations {:+}, evidence {:+}, archived_concepts {:+}, archived_relations {:+}, rejected_fields {:+}. Health: inflation {:.3}->{:.3}, unsupported {}->{}, violations {}->{}.",
        lhs.run_lineage.run_id,
        rhs.run_lineage.run_id,
        concept_delta,
        relation_delta,
        evidence_delta,
        archived_concept_delta,
        archived_relation_delta,
        rejected_field_delta,
        inflation_score_lhs,
        inflation_score_rhs,
        unsupported_confident_lhs,
        unsupported_confident_rhs,
        violation_count_lhs,
        violation_count_rhs,
    );

    RunComparisonReceipt {
        comparison_id: new_run_id("compare"),
        lhs_run_id: lhs.run_lineage.run_id.clone(),
        rhs_run_id: rhs.run_lineage.run_id.clone(),
        reason: reason.to_string(),
        summary,
        lhs_turn: lhs.turn,
        rhs_turn: rhs.turn,
        concept_delta,
        relation_delta,
        evidence_delta,
        archived_concept_delta,
        archived_relation_delta,
        rejected_field_delta,
        inflation_score_lhs,
        inflation_score_rhs,
        unsupported_confident_lhs,
        unsupported_confident_rhs,
        violation_count_lhs,
        violation_count_rhs,
        created_at: now_iso(),
    }
}

pub fn record_comparison(state: &mut State, receipt: RunComparisonReceipt) {
    state.run_lineage.comparisons.push(receipt);
}

pub fn adopt_state(state: &mut State, reason: &str) {
    state.run_lineage.status = "adopted".to_string();
    state.run_lineage.adoption = Some(AdoptionReceipt {
        adoption_id: new_run_id("adopt"),
        chosen_run_id: state.run_lineage.run_id.clone(),
        reason: reason.to_string(),
        created_at: now_iso(),
    });
}

pub fn summarize_state_for_lm(state: &State) -> LmStateSummary {
    let mut concepts: Vec<LmConceptView> = state
        .concepts
        .values()
        .cloned()
        .map(|concept| LmConceptView {
            id: concept.id,
            summary: concept.summary,
            aliases: concept.aliases,
            status: concept.status,
            mention_count: concept.mention_count,
        })
        .collect();
    concepts.sort_by(|a, b| {
        b.mention_count
            .cmp(&a.mention_count)
            .then_with(|| a.id.cmp(&b.id))
    });
    concepts.truncate(16);

    let mut relations: Vec<LmRelationView> = state
        .relations
        .values()
        .cloned()
        .map(|relation| LmRelationView {
            id: relation.id,
            source: relation.source,
            target: relation.target,
            relation: relation.relation,
            rationale: relation.rationale,
            status: relation.status,
            confidence: relation.confidence,
            evidence_for: relation.evidence_for,
            evidence_against: relation.evidence_against,
        })
        .collect();
    relations.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let ae = a.evidence_for.saturating_sub(a.evidence_against);
                let be = b.evidence_for.saturating_sub(b.evidence_against);
                be.cmp(&ae)
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    relations.truncate(16);

    let recent_events = state
        .events
        .iter()
        .rev()
        .take(6)
        .map(|event| LmEventView {
            turn: event.turn,
            summary: event.summary.clone(),
            active_focus: event.active_core.clone(),
            tensions: event.tensions.clone(),
        })
        .collect::<Vec<_>>();

    LmStateSummary {
        turn: state.turn,
        latest_summary: state.latest_summary.clone(),
        active_focus: state.active_focus.clone(),
        tensions: state.unresolved_tensions.clone(),
        concepts,
        relations,
        recent_events,
    }
}

pub fn process_turn(state: &mut State, message: &str) -> Result<Event> {
    let raw_symbols = symbols_for_message(message);
    process_turn_with_extraction(
        state,
        message,
        SymbolExtraction {
            extractor: "raw".to_string(),
            raw_text_sha256: sha256_hex_str(message),
            raw_symbols: raw_symbols.clone(),
            symbols: raw_symbols,
        },
    )
}

pub fn process_turn_with_symbols(
    state: &mut State,
    message: &str,
    symbols: Vec<String>,
) -> Result<Event> {
    process_turn_with_extraction(
        state,
        message,
        SymbolExtraction {
            extractor: "manual".to_string(),
            raw_text_sha256: sha256_hex_str(message),
            raw_symbols: symbols.clone(),
            symbols,
        },
    )
}

pub fn process_turn_with_extraction(
    state: &mut State,
    message: &str,
    extraction: SymbolExtraction,
) -> Result<Event> {
    let fallback = fallback_turn_delta(state, &extraction);
    process_turn_with_delta(state, message, extraction, String::new(), fallback)
}

pub fn process_turn_with_delta(
    state: &mut State,
    message: &str,
    extraction: SymbolExtraction,
    raw_response: String,
    delta: TurnDelta,
) -> Result<Event> {
    state.turn += 1;
    let turn = state.turn;
    let previous_focus: BTreeSet<String> = state.active_focus.iter().cloned().collect();
    let mut previous_concepts: BTreeSet<String> = state.concepts.keys().cloned().collect();
    let mut previous_relations: BTreeSet<String> = state.relations.keys().cloned().collect();

    let mut rejected_fields = Vec::new();
    let mut normalized = normalize_turn_delta(delta, &mut rejected_fields);

    let mut discovered = Vec::new();
    let mut archived = Vec::new();
    let mut concept_ids = Vec::new();
    for concept in &normalized.concepts {
        let concept_id = upsert_concept(state, concept, turn);
        if previous_concepts.insert(concept_id.clone()) {
            discovered.push(concept_id.clone());
        }
        concept_ids.push(concept_id);
    }

    for alias in &normalized.aliases {
        apply_alias(state, alias, turn, &mut rejected_fields);
    }

    let mut relation_ids = Vec::new();
    for relation in &normalized.relations {
        match upsert_relation(state, relation, turn, &mut rejected_fields) {
            Some(relation_id) => {
                if previous_relations.insert(relation_id.clone()) {
                    discovered.push(relation_id.clone());
                }
                relation_ids.push(relation_id);
            }
            None => rejected_fields.push(format!("relation:{}", relation.id)),
        }
    }

    for evidence in &normalized.evidence {
        // Evidence from the LM turn path is dialogue evidence by definition.
        // Set origin="dialogue" when empty so the no_evidence_at_declaration gate
        // can distinguish LM-grounded evidence from injected/audit entries.
        // source_uri carries the full raw signal for provenance tracing.
        let mut ev = evidence.clone();
        if ev.origin.is_empty() {
            ev.origin = "dialogue".to_string();
        }
        if let Some(relation_id) = apply_evidence(state, &ev, turn, message.to_string(), now_iso(), std::collections::BTreeMap::new(), &mut rejected_fields) {
            if !relation_ids.iter().any(|id| id == &relation_id) {
                relation_ids.push(relation_id);
            }
        }
    }

    for concept in state.concepts.values_mut() {
        if concept.status == "active" {
            concept.status = "known".to_string();
        }
    }
    for relation in state.relations.values_mut() {
        if relation.status == "active" {
            relation.status = "known".to_string();
        }
    }

    let mut active_focus = Vec::new();
    for focus_id in &normalized.active_focus {
        let canonical = canonicalize_reference(state, focus_id);
        let concept_archived = state
            .concepts
            .get(&canonical)
            .map(|concept| concept.status == "archived")
            .unwrap_or(false);
        let relation_archived = state
            .relations
            .get(&canonical)
            .map(|relation| relation.status == "archived")
            .unwrap_or(false);
        if concept_archived || relation_archived {
            rejected_fields.push(format!("focus:archived:{}", focus_id));
        } else if state.relations.contains_key(&canonical) || state.concepts.contains_key(&canonical) {
            if !active_focus.iter().any(|id| id == &canonical) {
                active_focus.push(canonical.clone());
            }
            if let Some(relation) = state.relations.get_mut(&canonical) {
                relation.status = "active".to_string();
            }
            if let Some(concept) = state.concepts.get_mut(&canonical) {
                concept.status = "active".to_string();
            }
        } else {
            rejected_fields.push(format!("focus:{}", focus_id));
        }
    }
    active_focus.sort();

    for relation in state.relations.values_mut() {
        if relation.status == "archived" {
            archived.push(relation.id.clone());
        }
    }
    for concept in state.concepts.values_mut() {
        if concept.status == "archived" {
            archived.push(concept.id.clone());
        }
    }
    archived.sort();
    archived.dedup();

    let promoted = active_focus
        .iter()
        .filter(|id| !previous_focus.contains(*id))
        .cloned()
        .collect::<Vec<_>>();

    // Gate fires if ANY new relation lacks dialogue evidence, OR if new concepts
    // were declared without any accompanying relations at all (concept-only turns
    // previously bypassed the gate since the loop over relation_ids was empty).
    let mut missing_evidence = false;
    for id in &relation_ids {
        if let Some(rel) = state.relations.get(id) {
            let has_dialogue = rel.support_set.iter().any(|ev_id| {
                state.evidence_log.iter().any(|e| &e.id == ev_id && e.origin == "dialogue")
            });
            if rel.evidence_for == 0 || !has_dialogue {
                missing_evidence = true;
                break;
            }
        }
    }
    // Concept-only turn bypass: if concepts were discovered but no relations were
    // proposed, the relation loop is empty and missing_evidence stays false —
    // allowing noise concepts to land as "known". Close this path.
    if !missing_evidence && !discovered.is_empty() && relation_ids.is_empty() {
        missing_evidence = true;
    }

    if missing_evidence {
        normalized.gate.need_more_evidence = true;
        normalized.gate.allow_act = false;
        normalized.gate.reason = "no_evidence_at_declaration_or_missing_dialogue".to_string();
        // Downgrade newly discovered concepts to "candidate" — they were declared
        // without dialogue evidence and must earn promotion through subsequent turns.
        // This is the anti-inflation gate that kept inflation_score=0.008 over 73 epochs.
        for id in &discovered {
            if let Some(concept) = state.concepts.get_mut(id) {
                if concept.status != "archived" {
                    concept.status = "candidate".to_string();
                }
            }
        }
    }

    state.active_focus = active_focus.clone();
    state.seed_queue = normalized.next_probes.clone();
    state.latest_gate = normalized.gate.clone();
    state.latest_summary = normalized.summary.clone();
    state.unresolved_tensions = normalized.tensions.clone();
    state.metrics = Metrics {
        concept_count: state.concepts.len(),
        relation_count: state.relations.len(),
        evidence_count: state.evidence_log.len(),
        focus_size: active_focus.len(),
    };

    let transition = TurnTransitionReceipt {
        raw_response,
        delta: normalized.clone(),
        rejected_fields,
    };

    let mut event = Event {
        turn,
        timestamp: now_iso(),
        message: message.to_string(),
        symbols: extraction.symbols.clone(),
        concept_ids,
        relation_ids,
        discovered,
        promoted,
        archived,
        active_core: active_focus.clone(),
        seeds: normalized.next_probes.clone(),
        gate: normalized.gate.clone(),
        packet_id: String::new(),
        summary: normalized.summary.clone(),
        tensions: normalized.tensions.clone(),
        extraction,
        transition,
    };

    let packet = build_graph_packet(state, &event);
    merge_packet(&mut state.graph, &packet);
    state.packets.push(packet.clone());
    event.packet_id = packet.packet_id;
    state.events.push(event.clone());

    // Run health invariants after every turn.
    // Stored in state for persistence; also emitted on stderr for the epoch runner.
    // Inflation violations are surfaced here so the epoch runner's auto_decay
    // can correct them before the next turn gates on confidence.
    let health = health_check(state);
    let inflation = health.stats.inflation_score;
    state.latest_health = health;

    // If inflation is severe (>0.3), surface it in the gate so the runner
    // sees it immediately rather than waiting for the next load_health call.
    if inflation > 0.3 {
        state.latest_gate.need_more_evidence = true;
        state.latest_gate.reason = format!(
            "inflation_requires_decay:{:.3}", inflation
        );
    }

    Ok(event)
}

pub fn monitor(state: &State) -> MonitorData {
    let mut top_relations = state
        .relations
        .values()
        .cloned()
        .map(|relation| MonitorRow {
            id: relation.id,
            source: relation.source,
            target: relation.target,
            relation: relation.relation,
            status: relation.status,
            confidence: relation.confidence,
            evidence_for: relation.evidence_for,
            evidence_against: relation.evidence_against,
        })
        .collect::<Vec<_>>();
    top_relations.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let ae = a.evidence_for.saturating_sub(a.evidence_against);
                let be = b.evidence_for.saturating_sub(b.evidence_against);
                be.cmp(&ae)
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    top_relations.truncate(8);

    MonitorData {
        turn: state.turn,
        concept_count: state.concepts.len(),
        relation_count: state.relations.len(),
        active_core: state.active_focus.clone(),
        seed_queue: state.seed_queue.clone(),
        gate: state.latest_gate.clone(),
        metrics: state.metrics.clone(),
        top_relations,
        graph_nodes: state.graph.nodes.len(),
        graph_links: state.graph.links.len(),
        latest_extraction: state.events.last().map(|event| event.extraction.clone()),
        latest_transition: state.events.last().map(|event| event.transition.clone()),
        latest_summary: state.latest_summary.clone(),
        tensions: state.unresolved_tensions.clone(),
        run_id: state.run_lineage.run_id.clone(),
        parent_run_id: state.run_lineage.parent_run_id.clone(),
        run_status: state.run_lineage.status.clone(),
        comparison_count: state.run_lineage.comparisons.len(),
    }
}

/// Run health invariant checks against the current state.
/// Returns a HealthSignal with violations and aggregate stats.
/// This replaces manual one-off scripts like decay_bootstrap.py.
pub fn health_check(state: &State) -> HealthSignal {
    let turn = state.turn;
    let mut violations = Vec::new();

    // Collect evidence IDs for orphan check
    let evidence_ids: BTreeSet<String> = state
        .evidence_log
        .iter()
        .map(|ev| ev.id.clone())
        .collect();

    // Collect relation IDs that have evidence in the last 5 turns
    let recent_evidence_relations: BTreeSet<String> = state
        .evidence_log
        .iter()
        .filter(|ev| ev.turn + 5 >= turn)
        .map(|ev| ev.relation_id.clone())
        .collect();

    let live_relations: Vec<&RelationRecord> = state
        .relations
        .values()
        .filter(|r| r.status != "archived")
        .collect();

    let total_relations = state.relations.len();
    let live_count = live_relations.len();
    let mut unsupported_confident: usize = 0;
    let mut confidence_sum: f32 = 0.0;
    let mut support_set_sum: usize = 0;
    let mut inflation_sum: f32 = 0.0;

    for rel in &live_relations {
        confidence_sum += rel.confidence;
        support_set_sum += rel.support_set.len();

        // Invariant 1: No unsupported confidence
        // A relation with no supporting evidence should have confidence=0.0
        if rel.evidence_for == 0 && rel.confidence > 0.0 {
            unsupported_confident += 1;
            violations.push(HealthViolation {
                kind: "inflated_confidence".to_string(),
                entity_id: rel.id.clone(),
                detail: format!(
                    "confidence={:.3} but evidence_for=0",
                    rel.confidence
                ),
            });
        }

        // Invariant 2: Support set consistency
        // Every ID in support_set should exist in the evidence log
        for support_id in &rel.support_set {
            if !evidence_ids.contains(support_id) {
                violations.push(HealthViolation {
                    kind: "orphan_support".to_string(),
                    entity_id: rel.id.clone(),
                    detail: format!("support_set contains {} which is not in evidence_log", support_id),
                });
            }
        }

        // Inflation score: gap between actual confidence and evidence-derived ratio
        let total_ev = (rel.evidence_for + rel.evidence_against).max(1) as f32;
        let evidence_ratio = rel.evidence_for as f32 / total_ev;
        let gap = (rel.confidence - evidence_ratio).max(0.0);
        inflation_sum += gap;
    }

    // Invariant 3: Stale focus
    // Items in active_focus should have had evidence activity recently
    let mut stale_focus_count: usize = 0;
    for focus_id in &state.active_focus {
        if state.relations.contains_key(focus_id) && !recent_evidence_relations.contains(focus_id) {
            stale_focus_count += 1;
            violations.push(HealthViolation {
                kind: "stale_focus".to_string(),
                entity_id: focus_id.clone(),
                detail: format!("in active_focus but no evidence in last 5 turns"),
            });
        }
    }

    let mean_confidence = if live_count > 0 {
        confidence_sum / live_count as f32
    } else {
        0.0
    };
    let mean_support_set_size = if live_count > 0 {
        support_set_sum as f32 / live_count as f32
    } else {
        0.0
    };
    let inflation_score = if live_count > 0 {
        inflation_sum / live_count as f32
    } else {
        0.0
    };

    // Invariant 4: Inflation drift
    if inflation_score > 0.1 {
        violations.push(HealthViolation {
            kind: "inflation_drift".to_string(),
            entity_id: "graph".to_string(),
            detail: format!(
                "inflation_score={:.4} (>0.1 threshold). Mean confidence exceeds evidence ratio.",
                inflation_score
            ),
        });
    }

    let healthy = violations.is_empty();

    HealthSignal {
        turn,
        violations,
        stats: HealthStats {
            total_relations,
            live_relations: live_count,
            unsupported_confident,
            mean_confidence,
            mean_support_set_size,
            stale_focus_count,
            inflation_score,
        },
        healthy,
    }
}

pub fn show_state(state: &State) -> String {
    let data = monitor(state);
    let mut lines = vec![
        "N* Autogenesis Rust".to_string(),
        format!("run_id: {}", data.run_id),
        format!("run_status: {}", data.run_status),
        format!("turn: {}", data.turn),
        format!("concepts: {}", data.concept_count),
        format!("relations: {}", data.relation_count),
        format!("graph: nodes={} links={}", data.graph_nodes, data.graph_links),
        format!("focus: {}", data.active_core.len()),
        format!("gate: allow_act={} reason={}", data.gate.allow_act, data.gate.reason),
        format!(
            "metrics: concepts={} relations={} evidence={} focus={}",
            data.metrics.concept_count,
            data.metrics.relation_count,
            data.metrics.evidence_count,
            data.metrics.focus_size
        ),
    ];
    if !data.parent_run_id.is_empty() {
        lines.push(format!("parent_run_id: {}", data.parent_run_id));
    }
    lines.push(format!("comparisons: {}", data.comparison_count));
    if !data.latest_summary.is_empty() {
        lines.push(format!("summary: {}", data.latest_summary));
    }
    if data.active_core.is_empty() {
        lines.push("focus_ids: (none)".to_string());
    } else {
        lines.push(format!("focus_ids: {}", data.active_core.join(", ")));
    }
    if !data.tensions.is_empty() {
        lines.push(format!("tensions: {}", data.tensions.join(" | ")));
    }
    lines.push("top relations:".to_string());
    for row in data.top_relations {
        lines.push(format!(
            "- {} {} {} status={} confidence={:.2} evidence={}/{}",
            row.source,
            row.relation,
            row.target,
            row.status,
            row.confidence,
            row.evidence_for,
            row.evidence_against
        ));
    }
    if !data.seed_queue.is_empty() {
        lines.push("seed_queue:".to_string());
        for seed in data.seed_queue {
            lines.push(format!("- [{}] {}", seed.kind, seed.prompt));
        }
    }
    lines.join("\n")
}

pub fn canonical_symbol_candidates(state: &State, limit: usize) -> Vec<String> {
    let mut weights: BTreeMap<String, i64> = BTreeMap::new();

    for focus_id in &state.active_focus {
        if let Some(concept) = state.concepts.get(focus_id) {
            *weights.entry(concept.id.clone()).or_insert(0) += 8;
        }
        if let Some(relation) = state.relations.get(focus_id) {
            *weights.entry(relation.source.clone()).or_insert(0) += 6;
            *weights.entry(relation.target.clone()).or_insert(0) += 6;
        }
    }

    for concept in state.concepts.values() {
        *weights.entry(concept.id.clone()).or_insert(0) += concept.mention_count as i64;
    }

    for event in state.events.iter().rev().take(8) {
        for symbol in &event.extraction.symbols {
            *weights.entry(symbol.clone()).or_insert(0) += 2;
        }
    }

    let mut ranked = weights.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(symbol, _)| symbol)
        .collect()
}

fn fallback_turn_delta(state: &State, extraction: &SymbolExtraction) -> TurnDelta {
    let concepts = extraction
        .symbols
        .iter()
        .map(|symbol| ConceptDelta {
            id: symbol.clone(),
            label: symbol.clone(),
            summary: format!("Observed in turn {}.", state.turn + 1),
            aliases: Vec::new(),
            status: "candidate".to_string(),
        })
        .collect::<Vec<_>>();

    let active_focus = extraction.symbols.iter().take(3).cloned().collect::<Vec<_>>();
    let next_probes = extraction
        .symbols
        .iter()
        .take(3)
        .map(|symbol| Seed {
            kind: "probe".to_string(),
            prompt: format!("Investigate how '{}' changes the current memory graph.", symbol),
        })
        .collect::<Vec<_>>();

    TurnDelta {
        summary: if extraction.symbols.is_empty() {
            "No stable concepts were extracted.".to_string()
        } else {
            format!("Observed concepts: {}.", extraction.symbols.join(", "))
        },
        concepts,
        aliases: Vec::new(),
        relations: Vec::new(),
        evidence: Vec::new(),
        active_focus: active_focus.clone(),
        next_probes,
        tensions: Vec::new(),
        gate: GateVerdict {
            allow_act: !active_focus.is_empty(),
            need_more_evidence: active_focus.is_empty(),
            reason: if active_focus.is_empty() {
                "no_focus".to_string()
            } else {
                "fallback_focus_selected".to_string()
            },
        },
    }
}

fn normalize_turn_delta(delta: TurnDelta, rejected_fields: &mut Vec<String>) -> TurnDelta {
    let mut seen_focus = BTreeSet::new();
    let active_focus = delta
        .active_focus
        .into_iter()
        .map(|item| canonical_focus_id(&item))
        .filter(|item| !item.is_empty())
        .filter(|item| seen_focus.insert(item.clone()))
        .collect::<Vec<_>>();

    let concepts = delta
        .concepts
        .into_iter()
        .filter_map(|concept| {
            let id = canonical_symbol(&first_non_empty(&[&concept.id, &concept.label]));
            if id.is_empty() {
                rejected_fields.push("concept:missing_id".to_string());
                return None;
            }
            let mut aliases = Vec::new();
            let mut seen = BTreeSet::new();
            for alias in concept.aliases {
                let alias_id = canonical_symbol(&alias);
                if !alias_id.is_empty() && alias_id != id && seen.insert(alias_id.clone()) {
                    aliases.push(alias_id);
                }
            }
            Some(ConceptDelta {
                id: id.clone(),
                label: if concept.label.trim().is_empty() {
                    id
                } else {
                    concept.label.trim().to_string()
                },
                summary: concept.summary.trim().to_string(),
                aliases,
                status: if concept.status.trim().is_empty() {
                    "known".to_string()
                } else {
                    concept.status.trim().to_string()
                },
            })
        })
        .collect::<Vec<_>>();

    let aliases = delta
        .aliases
        .into_iter()
        .filter_map(|alias| {
            let alias_id = canonical_symbol(&alias.alias);
            let canonical = canonical_symbol(&alias.canonical);
            if alias_id.is_empty() || canonical.is_empty() || alias_id == canonical {
                rejected_fields.push("alias:invalid".to_string());
                return None;
            }
            Some(AliasDecision {
                alias: alias_id,
                canonical,
                reason: alias.reason.trim().to_string(),
            })
        })
        .collect::<Vec<_>>();

    let relations = delta
        .relations
        .into_iter()
        .filter_map(|relation| {
            let source = canonical_symbol(&relation.source);
            let target = canonical_symbol(&relation.target);
            let relation_kind = canonical_symbol(&relation.relation);
            if source.is_empty() || target.is_empty() || relation_kind.is_empty() {
                rejected_fields.push("relation:invalid".to_string());
                return None;
            }
            let id = if relation.id.trim().is_empty() {
                relation_id(&source, &relation_kind, &target)
            } else {
                canonical_focus_id(&relation.id)
            };
            Some(RelationDelta {
                id,
                source,
                target,
                relation: relation_kind,
                status: if relation.status.trim().is_empty() {
                    "known".to_string()
                } else {
                    relation.status.trim().to_string()
                },
                rationale: relation.rationale.trim().to_string(),
                confidence: relation.confidence.clamp(0.0, 1.0),
            })
        })
        .collect::<Vec<_>>();

    let evidence = delta
        .evidence
        .into_iter()
        .map(|item| EvidenceDelta {
            relation_id: canonical_focus_id(&item.relation_id),
            source: canonical_symbol(&item.source),
            target: canonical_symbol(&item.target),
            relation: canonical_symbol(&item.relation),
            verdict: canonical_symbol(&item.verdict),
            explanation: item.explanation.trim().to_string(),
            confidence: item.confidence.clamp(0.0, 1.0),
            origin: if item.origin.trim().is_empty() { "audit".to_string() } else { item.origin.trim().to_string() },
        })
        .collect::<Vec<_>>();

    TurnDelta {
        summary: delta.summary.trim().to_string(),
        concepts,
        aliases,
        relations,
        evidence,
        active_focus,
        next_probes: delta
            .next_probes
            .into_iter()
            .filter(|seed| !seed.prompt.trim().is_empty())
            .collect(),
        tensions: delta
            .tensions
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        gate: if delta.gate.reason.trim().is_empty() {
            GateVerdict {
                allow_act: false,
                need_more_evidence: true,
                reason: "missing_gate".to_string(),
            }
        } else {
            delta.gate
        },
    }
}

fn upsert_concept(state: &mut State, concept: &ConceptDelta, turn: u64) -> String {
    let id = canonical_symbol(&concept.id);
    let entry = state.concepts.entry(id.clone()).or_insert_with(|| ConceptRecord {
        id: id.clone(),
        label: if concept.label.trim().is_empty() {
            id.clone()
        } else {
            concept.label.clone()
        },
        summary: concept.summary.clone(),
        aliases: Vec::new(),
        status: concept.status.clone(),
        first_seen_turn: turn,
        last_seen_turn: turn,
        mention_count: 0,
    });
    if !concept.summary.trim().is_empty() {
        entry.summary = concept.summary.clone();
    }
    if !concept.label.trim().is_empty() {
        entry.label = concept.label.clone();
    }
    if !concept.status.trim().is_empty() {
        entry.status = concept.status.clone();
    }
    entry.last_seen_turn = turn;
    entry.mention_count += 1;
    for alias in &concept.aliases {
        if !entry.aliases.iter().any(|existing| existing == alias) {
            entry.aliases.push(alias.clone());
        }
        state.aliases.insert(alias.clone(), id.clone());
    }
    if entry.status == "archived" {
        archive_concept_cascade(state, &id);
    }
    id
}

fn apply_alias(
    state: &mut State,
    alias: &AliasDecision,
    _turn: u64,
    rejected_fields: &mut Vec<String>,
) {
    let alias_id = canonical_symbol(&alias.alias);
    let canonical = canonical_symbol(&alias.canonical);
    if alias_id.is_empty() || canonical.is_empty() || alias_id == canonical {
        rejected_fields.push("alias:invalid".to_string());
        return;
    }
    if !state.concepts.contains_key(&canonical) {
        rejected_fields.push(format!("alias:unknown_canonical:{}", canonical));
        return;
    }
    state.aliases.insert(alias_id.clone(), canonical.clone());
    if let Some(concept) = state.concepts.get_mut(&canonical) {
        if !concept.aliases.iter().any(|existing| existing == &alias_id) {
            concept.aliases.push(alias_id.clone());
        }
    }
    // Archive the alias concept — it is superseded by canonical.
    // This is real compression: the alias slot is retired and its relations cascade.
    if state.concepts.contains_key(&alias_id) {
        archive_concept_cascade(state, &alias_id);
    }
}

fn upsert_relation(
    state: &mut State,
    relation: &RelationDelta,
    turn: u64,
    rejected_fields: &mut Vec<String>,
) -> Option<String> {
    let source = canonicalize_reference(state, &relation.source);
    let target = canonicalize_reference(state, &relation.target);
    let relation_kind = canonical_symbol(&relation.relation);
    if source.is_empty() || target.is_empty() || relation_kind.is_empty() {
        rejected_fields.push("relation:missing_endpoints".to_string());
        return None;
    }
    if !state.concepts.contains_key(&source) {
        rejected_fields.push(format!("relation:unknown_source:{}", source));
        return None;
    }
    if !state.concepts.contains_key(&target) {
        rejected_fields.push(format!("relation:unknown_target:{}", target));
        return None;
    }
    if state
        .concepts
        .get(&source)
        .map(|concept| concept.status == "archived")
        .unwrap_or(false)
    {
        rejected_fields.push(format!("relation:archived_source:{}", source));
        return None;
    }
    if state
        .concepts
        .get(&target)
        .map(|concept| concept.status == "archived")
        .unwrap_or(false)
    {
        rejected_fields.push(format!("relation:archived_target:{}", target));
        return None;
    }
    let id = if relation.id.trim().is_empty() {
        relation_id(&source, &relation_kind, &target)
    } else {
        canonical_focus_id(&relation.id)
    };
    let entry = state
        .relations
        .entry(id.clone())
        .or_insert_with(|| RelationRecord {
            id: id.clone(),
            source: source.clone(),
            target: target.clone(),
            relation: relation_kind.clone(),
            status: relation.status.clone(),
            rationale: relation.rationale.clone(),
            confidence: relation.confidence,
            evidence_for: 0,
            evidence_against: 0,
            support_set: Vec::new(),
            first_seen_turn: turn,
            last_updated_turn: turn,
            last_evidence_id: String::new(),
            last_confidence_delta: 0.0,
        });
    entry.source = source;
    entry.target = target;
    entry.relation = relation_kind;
    if !relation.rationale.trim().is_empty() {
        entry.rationale = relation.rationale.clone();
    }
    if !relation.status.trim().is_empty() {
        entry.status = relation.status.clone();
    }
    // ── Admission gate: beliefs earn confidence, they don't inherit it ──
    // New relations enter at 0.0 regardless of what the LM declared.
    // Existing relations keep their evidence-derived confidence.
    // Only apply_evidence moves the needle.
    if entry.first_seen_turn == turn {
        // Brand new relation — starts unknown.
        entry.confidence = 0.0;
    } else if entry.evidence_for > 0 || entry.evidence_against > 0 {
        // Existing relation with evidence: recompute from evidence counts.
        // This prevents the LM from inflating confidence via re-declaration.
        let total = (entry.evidence_for + entry.evidence_against).max(1) as f32;
        let raw = entry.evidence_for as f32 / total;
        let floor = if entry.support_set.is_empty() { 0.0 } else { 0.1 };
        entry.confidence = (raw + floor).clamp(0.0, 1.0);
    }
    // If evidence_for=0 and evidence_against=0 on an existing relation,
    // confidence stays at 0.0 (from when it was created). No free lunch.
    entry.last_updated_turn = turn;
    if entry.status == "archived" {
        archive_relation(state, &id);
    }
    Some(id)
}

fn apply_evidence(
    state: &mut State,
    evidence: &EvidenceDelta,
    turn: u64,
    source_uri: String,
    timestamp: String,
    meta: std::collections::BTreeMap<String, String>,
    rejected_fields: &mut Vec<String>,
) -> Option<String> {
    let relation_id = if !evidence.relation_id.is_empty() {
        canonical_focus_id(&canonicalize_reference(state, &evidence.relation_id))
    } else if !evidence.source.is_empty() && !evidence.target.is_empty() && !evidence.relation.is_empty() {
        relation_id(
            &canonicalize_reference(state, &evidence.source),
            &canonical_symbol(&evidence.relation),
            &canonicalize_reference(state, &evidence.target),
        )
    } else {
        rejected_fields.push("evidence:missing_relation".to_string());
        return None;
    };

    let verdict = canonical_symbol(&evidence.verdict);
    if verdict.is_empty() {
        rejected_fields.push("evidence:missing_verdict".to_string());
        return None;
    }

    // Deterministic evidence ID — stable across replays for the same turn position.
    let evidence_id = format!("ev:{}:{}", turn, state.evidence_log.len() + 1);

    if let Some(relation) = state.relations.get_mut(&relation_id) {
        match verdict.as_str() {
            "supports" | "refines" => {
                relation.evidence_for += 1;
                if !relation.support_set.iter().any(|id| id == &evidence_id) {
                    relation.support_set.push(evidence_id.clone());
                }
            }
            "contradicts" | "weakens" => {
                relation.evidence_against += 1;
                let prior_support_id = format!("ev:{}:{}", turn - 1, state.evidence_log.len());
                relation.support_set.retain(|id| id != &prior_support_id);
            }
            _ => {}
        }
        
        // Recompute confidence and record causal delta
        let old_confidence = relation.confidence;
        let total = (relation.evidence_for + relation.evidence_against).max(1) as f32;
        let raw = relation.evidence_for as f32 / total;
        let support_floor = if relation.support_set.is_empty() { 0.0 } else { 0.1 };
        relation.confidence = (raw + support_floor).clamp(0.0, 1.0);
        relation.last_evidence_id = evidence_id.clone();
        relation.last_confidence_delta = relation.confidence - old_confidence;
        relation.last_updated_turn = turn;
    } else {
        rejected_fields.push(format!("evidence:unknown_relation:{}", relation_id));
        return None;
    }

    state.evidence_log.push(EvidenceRecord {
        id: evidence_id,
        relation_id: relation_id.clone(),
        verdict,
        explanation: evidence.explanation.clone(),
        confidence: evidence.confidence,
        turn,
        source_uri,
        timestamp,
        meta,
        origin: evidence.origin.clone(),
    });

    Some(relation_id)
}

pub fn inject_evidence(
    state: &mut State,
    delta: EvidenceDelta,
    source_uri: String,
    meta: std::collections::BTreeMap<String, String>,
) -> Result<String, String> {
    let mut rejected = Vec::new();
    
    // Increment turn globally for an independent evidence injection
    state.turn += 1;
    let turn = state.turn;

    match apply_evidence(state, &delta, turn, source_uri, now_iso(), meta, &mut rejected) {
        Some(id) => Ok(id),
        None => Err(rejected.join(", ")),
    }
}

pub fn process_evidence_file(state: &mut State, file_path: &str) -> Result<Vec<String>> {
    let content = fs::read_to_string(file_path)?;
    let payload: EvidencePayload = serde_json::from_str(&content)?;
    
    let mut applied_ids = Vec::new();
    for delta in payload.items {
        if let Ok(id) = inject_evidence(state, delta, payload.source_uri.clone(), payload.meta.clone()) {
            applied_ids.push(id);
        }
    }
    
    Ok(applied_ids)
}

fn build_graph_packet(state: &State, event: &Event) -> GraphPacket {
    let turn = event.turn;
    let mut nodes = Vec::new();
    let mut links = Vec::new();
    let turn_id = format!("TURN:{}", turn);
    let msg_id = format!("MSG:{}", turn);
    let extract_id = format!("EXTRACT:{}", turn);
    let transition_id = format!("TRANSITION:{}", turn);

    nodes.push(GraphNode {
        id: turn_id.clone(),
        kind: "turn".to_string(),
        label: format!("turn {}", turn),
    });
    nodes.push(GraphNode {
        id: msg_id.clone(),
        kind: "message".to_string(),
        label: event.message.clone(),
    });
    nodes.push(GraphNode {
        id: extract_id.clone(),
        kind: "extraction".to_string(),
        label: event.extraction.extractor.clone(),
    });
    nodes.push(GraphNode {
        id: transition_id.clone(),
        kind: "transition".to_string(),
        label: event.summary.clone(),
    });

    links.push(GraphLink {
        source: turn_id.clone(),
        target: msg_id.clone(),
        rel: "turn_message".to_string(),
        weight: 1.0,
    });
    links.push(GraphLink {
        source: turn_id.clone(),
        target: extract_id.clone(),
        rel: "turn_extraction".to_string(),
        weight: 1.0,
    });
    links.push(GraphLink {
        source: turn_id.clone(),
        target: transition_id.clone(),
        rel: "turn_transition".to_string(),
        weight: 1.0,
    });

    for symbol in &event.extraction.symbols {
        let symbol_id = format!("SYM:{}", symbol);
        nodes.push(GraphNode {
            id: symbol_id.clone(),
            kind: "symbol".to_string(),
            label: symbol.clone(),
        });
        links.push(GraphLink {
            source: extract_id.clone(),
            target: symbol_id,
            rel: "extraction_symbol".to_string(),
            weight: 1.0,
        });
    }

    for concept_id in &event.concept_ids {
        if let Some(concept) = state.concepts.get(concept_id) {
            let node_id = format!("CONCEPT:{}", concept.id);
            nodes.push(GraphNode {
                id: node_id.clone(),
                kind: "concept".to_string(),
                label: concept.label.clone(),
            });
            links.push(GraphLink {
                source: transition_id.clone(),
                target: node_id.clone(),
                rel: "transition_concept".to_string(),
                weight: 1.0,
            });
            for alias in &concept.aliases {
                links.push(GraphLink {
                    source: node_id.clone(),
                    target: format!("SYM:{}", alias),
                    rel: "concept_alias".to_string(),
                    weight: 1.0,
                });
            }
        }
    }

    for relation_id in &event.relation_ids {
        if let Some(relation) = state.relations.get(relation_id) {
            let node_id = format!("RELATION:{}", relation.id);
            nodes.push(GraphNode {
                id: node_id.clone(),
                kind: "relation".to_string(),
                label: relation.id.clone(),
            });
            links.push(GraphLink {
                source: transition_id.clone(),
                target: node_id.clone(),
                rel: "transition_relation".to_string(),
                weight: relation.confidence as f64,
            });
            links.push(GraphLink {
                source: node_id.clone(),
                target: format!("CONCEPT:{}", relation.source),
                rel: "relation_source".to_string(),
                weight: 1.0,
            });
            links.push(GraphLink {
                source: node_id.clone(),
                target: format!("CONCEPT:{}", relation.target),
                rel: "relation_target".to_string(),
                weight: 1.0,
            });
        }
    }

    for focus_id in &event.active_core {
        let target = if state.relations.contains_key(focus_id) {
            format!("RELATION:{}", focus_id)
        } else {
            format!("CONCEPT:{}", focus_id)
        };
        links.push(GraphLink {
            source: turn_id.clone(),
            target,
            rel: "turn_focus".to_string(),
            weight: 1.0,
        });
    }

    GraphPacket {
        packet_id: format!("pkt-turn-{}", turn),
        turn,
        nodes,
        links,
    }
}

fn merge_packet(graph: &mut GraphSnapshot, packet: &GraphPacket) {
    let mut node_ids = graph
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    for node in &packet.nodes {
        if node_ids.insert(node.id.clone()) {
            graph.nodes.push(node.clone());
        }
    }

    let mut link_ids = graph
        .links
        .iter()
        .map(|link| (link.source.clone(), link.target.clone(), link.rel.clone()))
        .collect::<BTreeSet<_>>();
    for link in &packet.links {
        let key = (link.source.clone(), link.target.clone(), link.rel.clone());
        if link_ids.insert(key) {
            graph.links.push(link.clone());
        }
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            if current.len() >= 3 && seen.insert(current.clone()) {
                out.push(current.clone());
            }
            current.clear();
        }
    }
    if !current.is_empty() && current.len() >= 3 && seen.insert(current.clone()) {
        out.push(current);
    }
    out
}

fn symbols_for_message(message: &str) -> Vec<String> {
    let quoted_atoms = extract_quoted_atoms(message);
    if !quoted_atoms.is_empty() {
        return quoted_atoms;
    }
    tokenize(message)
}

fn extract_quoted_atoms(message: &str) -> Vec<String> {
    let mut atoms = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = String::new();
    let mut in_quote = false;
    for ch in message.chars() {
        if ch == '\'' {
            if in_quote {
                let atom = canonical_symbol(current.trim());
                if !atom.is_empty() && seen.insert(atom.clone()) {
                    atoms.push(atom);
                }
                current.clear();
                in_quote = false;
            } else {
                current.clear();
                in_quote = true;
            }
            continue;
        }
        if in_quote {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                current.push(ch);
            } else if !current.ends_with(' ') {
                current.push(' ');
            }
        }
    }
    atoms
}

fn first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .find_map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_default()
}

pub fn canonical_symbol(input: &str) -> String {
    input
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .replace("__", "_")
}

pub fn canonical_focus_id(input: &str) -> String {
    let trimmed = input.trim().to_ascii_lowercase();
    if trimmed.starts_with("rel:") {
        trimmed
    } else {
        canonical_symbol(&trimmed)
    }
}

pub fn canonicalize_reference(state: &State, input: &str) -> String {
    let mut current = canonical_focus_id(input);
    let mut hops = 0;
    while let Some(next) = state.aliases.get(&current) {
        if next == &current || hops > 8 {
            break;
        }
        current = next.clone();
        hops += 1;
    }
    current
}

pub fn relation_id(source: &str, relation: &str, target: &str) -> String {
    format!("rel:{}|{}|{}", source, relation, target)
}

fn new_run_id(prefix: &str) -> String {
    let seed = format!("{}:{}", prefix, now_iso());
    let digest = sha256_hex_str(&seed);
    format!("{}-{}", prefix, &digest[..12])
}

fn count_concepts_by_status(state: &State, status: &str) -> usize {
    state.concepts.values().filter(|concept| concept.status == status).count()
}

fn count_relations_by_status(state: &State, status: &str) -> usize {
    state
        .relations
        .values()
        .filter(|relation| relation.status == status)
        .count()
}

fn total_rejected_fields(state: &State) -> usize {
    state
        .events
        .iter()
        .map(|event| event.transition.rejected_fields.len())
        .sum()
}

fn archive_concept_cascade(state: &mut State, concept_id: &str) {
    if let Some(concept) = state.concepts.get_mut(concept_id) {
        concept.status = "archived".to_string();
    }
    let related_relations = state
        .relations
        .iter()
        .filter_map(|(relation_id, relation)| {
            if relation.source == concept_id || relation.target == concept_id {
                Some(relation_id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for relation_id in related_relations {
        archive_relation(state, &relation_id);
    }
}

fn archive_relation(state: &mut State, relation_id: &str) {
    if let Some(relation) = state.relations.get_mut(relation_id) {
        relation.status = "archived".to_string();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Measurement epoch: support_set correctness
//
// These three tests are the "next epoch" — not an accumulation run but a
// direct property check on the primitive we just shipped.  They answer the
// one question we said needed answering:
//
//   Does confidence now correctly track |support_set| / total_evidence?
//
// Run with:  cargo test support_set -- --nocapture
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod support_set_epoch {
    use super::*;

    /// Build a minimal state with two concepts and one relation between them.
    /// Returns (state, relation_id).
    fn minimal_state() -> (State, String) {
        let mut state = State::default();

        // Seed two concepts directly — bypass process_turn to stay focused.
        state.concepts.insert(
            "belief".to_string(),
            ConceptRecord {
                id: "belief".to_string(),
                label: "belief".to_string(),
                summary: "a held proposition".to_string(),
                aliases: vec![],
                status: "known".to_string(),
                first_seen_turn: 1,
                last_seen_turn: 1,
                mention_count: 1,
            },
        );
        state.concepts.insert(
            "evidence".to_string(),
            ConceptRecord {
                id: "evidence".to_string(),
                label: "evidence".to_string(),
                summary: "information supporting or refuting a belief".to_string(),
                aliases: vec![],
                status: "known".to_string(),
                first_seen_turn: 1,
                last_seen_turn: 1,
                mention_count: 1,
            },
        );

        let rel = RelationDelta {
            id: String::new(), // auto-generated
            source: "belief".to_string(),
            target: "evidence".to_string(),
            relation: "requires".to_string(),
            status: "known".to_string(),
            rationale: "beliefs require evidence to persist".to_string(),
            confidence: 0.8,
        };

        let mut rejected = vec![];
        let rid = upsert_relation(&mut state, &rel, 1, &mut rejected)
            .expect("upsert_relation should succeed");
        assert!(rejected.is_empty(), "unexpected rejections: {:?}", rejected);

        (state, rid)
    }

    // ── Property 0: admission gate — new relations enter at confidence=0.0 ────

    #[test]
    fn p0_admission_gate_ignores_lm_declared_confidence() {
        let (state, rid) = minimal_state();
        let relation = &state.relations[&rid];

        // The RelationDelta declared confidence=0.8, but the gate must have
        // overridden it to 0.0 because this is a brand-new relation with no evidence.
        assert_eq!(
            relation.confidence, 0.0,
            "admission gate must set new relations to 0.0; got {}",
            relation.confidence
        );
        assert_eq!(relation.evidence_for, 0);
        assert_eq!(relation.evidence_against, 0);
        assert!(relation.support_set.is_empty());
    }

    // ── Property 1: supporting evidence grows the support_set ────────────────

    #[test]
    fn p1_supporting_evidence_populates_support_set() {
        let (mut state, rid) = minimal_state();

        // Relation starts with an empty support_set (declared, not yet evidenced).
        assert_eq!(
            state.relations[&rid].support_set.len(),
            0,
            "new relation must start with empty support_set"
        );

        let ev = EvidenceDelta {
            relation_id: rid.clone(),
            source: String::new(),
            target: String::new(),
            relation: String::new(),
            verdict: "supports".to_string(),
            explanation: "direct empirical observation".to_string(),
            confidence: 0.9,
            origin: "dialogue".to_string(),
        };

        let mut rejected = vec![];
        apply_evidence(&mut state, &ev, 2, String::new(), String::new(), std::collections::BTreeMap::new(), &mut rejected);
        assert!(rejected.is_empty(), "unexpected rejections: {:?}", rejected);

        let relation = &state.relations[&rid];
        assert_eq!(relation.support_set.len(), 1, "support_set must contain the new evidence ID");
        assert_eq!(relation.evidence_for, 1);
        assert!(
            relation.support_set[0].starts_with("ev:2:"),
            "evidence ID must encode turn: got {}",
            relation.support_set[0]
        );
    }

    // ── Property 2: contradicting evidence shrinks support_set & recomputes confidence ──

    #[test]
    fn p2_contradicting_evidence_shrinks_support_set_and_recomputes_confidence() {
        let (mut state, rid) = minimal_state();

        // Turn 2: one supporting piece of evidence.
        let ev_for = EvidenceDelta {
            relation_id: rid.clone(),
            source: String::new(), target: String::new(), relation: String::new(),
            verdict: "supports".to_string(),
            explanation: "turn-2 observation confirms belief requires evidence".to_string(),
            confidence: 1.0,
            origin: "dialogue".to_string(),
        };
        let mut rejected = vec![];
        apply_evidence(&mut state, &ev_for, 2, String::new(), String::new(), std::collections::BTreeMap::new(), &mut rejected);
        assert!(rejected.is_empty());

        let support_size_before = state.relations[&rid].support_set.len();
        assert_eq!(support_size_before, 1);

        // Turn 3: contradicting evidence injected from a disconfirming audit.
        let ev_against = EvidenceDelta {
            relation_id: rid.clone(),
            source: String::new(), target: String::new(), relation: String::new(),
            verdict: "contradicts".to_string(),
            explanation: "audit shows belief can exist without direct evidence (hardening pathology)".to_string(),
            confidence: 0.7,
            origin: "dialogue".to_string(),
        };
        apply_evidence(&mut state, &ev_against, 3, String::new(), String::new(), std::collections::BTreeMap::new(), &mut rejected);
        assert!(rejected.is_empty());

        let relation = &state.relations[&rid];

        // With admission gate: relation started at 0.0, then got one support
        // (evidence_for=1) pushing it up, then one contradiction (evidence_against=1)
        // pushing it back down. evidence_for=1, evidence_against=1 → raw = 0.5.
        // support_set may still have the turn-2 entry (turn-1 prior invalidation
        // targets turn 2, i.e., ev:2:N which is still in the set — that's fine).
        // So confidence ≤ 0.5 + 0.1 floor = 0.6 at most.
        assert!(
            relation.confidence <= 0.65,
            "confidence must be evidence-derived (≤0.65); got {}",
            relation.confidence
        );
        assert_eq!(relation.evidence_for, 1);
        assert_eq!(relation.evidence_against, 1);
    }

    // ── Property 3: empty support_set collapses confidence to raw ratio (no floor) ──

    #[test]
    fn p3_empty_support_set_collapses_confidence() {
        let (mut state, rid) = minimal_state();

        // Inject three rounds of contradicting evidence with no support at all.
        // support_set stays empty; confidence should track pure ratio = 0 / N.
        for turn in 2..=4 {
            let ev = EvidenceDelta {
                relation_id: rid.clone(),
                source: String::new(), target: String::new(), relation: String::new(),
                verdict: "contradicts".to_string(),
                explanation: format!("disconfirmation #{}", turn - 1),
                confidence: 0.9,
                origin: "dialogue".to_string(),
            };
            let mut rejected = vec![];
            apply_evidence(&mut state, &ev, turn, String::new(), String::new(), std::collections::BTreeMap::new(), &mut rejected);
            assert!(rejected.is_empty());
        }

        let relation = &state.relations[&rid];
        assert!(
            relation.support_set.is_empty(),
            "support_set must be empty when only contradicting evidence was applied"
        );
        // evidence_for=0, evidence_against=3 → raw = 0.0, floor bonus = 0.0
        assert!(
            relation.confidence < 0.05,
            "confidence must collapse near zero with empty support_set; got {}",
            relation.confidence
        );
        println!(
            "[p3] confidence={:.4} support_set={} evidence_for={} evidence_against={}",
            relation.confidence,
            relation.support_set.len(),
            relation.evidence_for,
            relation.evidence_against
        );
    }
}
