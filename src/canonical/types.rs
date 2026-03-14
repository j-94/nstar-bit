use serde::{Deserialize, Deserializer, Serialize};

use crate::lm::OvmOp;
use crate::receipt::Effect;
use crate::utir::Operation;

use super::schema::{
    BenchmarkReport, LiveControlState, PromotionDecisionRecord, RuntimeExecutionRecord,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub condition: String,
    pub prime_id: u64,
    #[serde(default)]
    pub control_signals: Vec<String>,
    #[serde(default = "default_node_threshold")]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub threshold: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub activation: f32,
    pub discovered_turn: u64,
    #[serde(default)]
    pub reinforcements: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeKind {
    Supports,
    Inhibits,
    Hypothesis,
}

/// Provenance record for a single co-activation event that contributed to an edge.
/// Lets you trace exactly which turns caused c11 to increment — the edge becomes
/// an auditable argument, not just a frequency count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalAnchor {
    /// The turn at which this co-activation was observed.
    pub turn_id: u64,
    /// Short human-readable description of what co-activated (e.g. "node:A ∧ node:B").
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub weight: f32,
    pub kind: EdgeKind,
    #[serde(default)]
    pub c11: u64,
    #[serde(default)]
    pub c10: u64,
    #[serde(default)]
    pub c01: u64,
    #[serde(default)]
    pub c00: u64,
    /// Provenance: the specific turns + snippets that drove c11 upward.
    /// Capped at 8 entries — enough to audit the claim without blowing up state size.
    #[serde(default)]
    pub anchors: Vec<CausalAnchor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePattern {
    #[serde(default)]
    pub require_all: Vec<String>,
    #[serde(default)]
    pub block_any: Vec<String>,
    #[serde(default)]
    pub control_signals: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalCriteria {
    #[serde(default = "default_max_risk")]
    #[serde(deserialize_with = "null_to_default_f64")]
    pub max_risk: f64,
    #[serde(default = "default_audit_rate")]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub audit_rate: f32,
    #[serde(default = "default_require_read_before_write")]
    pub require_read_before_write: bool,
    #[serde(default = "default_min_evidence_coverage")]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub min_evidence_coverage: f32,
    #[serde(default = "default_contradiction_threshold")]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub contradiction_threshold: f32,
    #[serde(default = "default_activation_cutoff")]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub activation_cutoff: f32,
    #[serde(default = "default_propagation_steps")]
    pub propagation_steps: usize,
}

impl Default for CanonicalCriteria {
    fn default() -> Self {
        Self {
            max_risk: 0.8,
            audit_rate: 0.33,
            require_read_before_write: true,
            min_evidence_coverage: 0.7,
            contradiction_threshold: 0.1,
            activation_cutoff: 0.4,
            propagation_steps: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphState {
    pub criteria: CanonicalCriteria,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub patterns: Vec<GatePattern>,
    #[serde(default)]
    pub scoring_rule: String,
    #[serde(default)]
    pub selection_predicate: String,
    #[serde(default)]
    pub rule_scorecard: Option<RuleScorecard>,
    /// Self-generated investigation prompts.
    /// Populated from top_misses and sparse edges after each turn.
    /// Runner drains these before pulling the next external input.
    #[serde(default)]
    pub seed_queue: Vec<String>,
    /// Edge pairs already self-investigated — skip re-generating seeds for these.
    #[serde(default)]
    pub investigated_pairs: Vec<(String, String)>,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            criteria: CanonicalCriteria::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
            patterns: Vec::new(),
            scoring_rule: String::new(),
            selection_predicate: String::new(),
            rule_scorecard: None,
            seed_queue: Vec::new(),
            investigated_pairs: Vec::new(),
        }
    }
}

/// Held-out prediction scorecard for the current scoring rule.
/// Computed every N turns and injected into the LM prompt so the rule
/// can self-improve based on what it mispredicted.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleScorecard {
    pub rule: String,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub precision_at_k: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub recall_at_k: f32,
    pub k: usize,
    pub train_turns: usize,
    pub test_turns: usize,
    /// Top edges ranked high by rule but absent in test (false positives)
    pub top_misses: Vec<ScorecardEdge>,
    /// Top edges ranked high and present in test (true positives)
    pub top_hits: Vec<ScorecardEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorecardEdge {
    pub from: String,
    pub to: String,
    pub c11: u64,
    pub c10: u64,
    pub c01: u64,
    pub c00: u64,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub score: f32,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeObservation {
    pub id: String,
    pub label: String,
    pub condition: String,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub activation: f32,
    #[serde(default)]
    pub control_signals: Vec<String>,
    #[serde(default = "default_node_threshold")]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDiscovery {
    pub id: String,
    pub label: String,
    pub condition: String,
    #[serde(default)]
    pub control_signals: Vec<String>,
    #[serde(default = "default_node_threshold")]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub threshold: f32,
    #[serde(default)]
    pub require_all: Vec<String>,
    #[serde(default)]
    pub block_any: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalInput {
    pub prompt: String,
    pub context: Vec<String>,
    pub turn: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalProposal {
    pub response: String,
    pub actions: Vec<String>,
    pub errors: Vec<String>,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub quality: f32,
    pub operations: Vec<Operation>,
    #[serde(default)]
    pub ovm_ops: Vec<OvmOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Scale {
    Token,
    Turn,
    Session,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleCoordinate {
    pub scale: Scale,
    pub event_id: u64,
    pub primes: Vec<u64>,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub intensity: f32,
    pub active_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    #[serde(default)]
    pub emitted_signals: Vec<String>,
}

impl GateDecision {
    pub fn clear() -> Self {
        Self {
            emitted_signals: Vec::new(),
        }
    }

    pub fn summary(&self) -> String {
        if self.emitted_signals.is_empty() {
            return "CLEAR: proceed".to_string();
        }
        self.emitted_signals.join(" + ")
    }

    pub fn has_signal(&self, sig: &str) -> bool {
        self.emitted_signals
            .iter()
            .any(|s| s.to_lowercase() == sig.to_lowercase())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationReport {
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f64")]
    pub max_risk: f64,
    #[serde(default)]
    pub predicted_effects: Vec<String>,
    #[serde(default)]
    pub blocked_reasons: Vec<String>,
    #[serde(default)]
    pub can_materialize: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantReport {
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub violations: Vec<String>,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub evidence_coverage: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub contradiction_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnDecision {
    Commit,
    Rollback,
    Escalate,
    Halt,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TurnCost {
    pub duration_ms: u64,
    pub activation_count: u64,
    pub traversal_depth: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTrace {
    pub input: CanonicalInput,
    pub proposal: CanonicalProposal,
    pub gate: GateDecision,
    pub simulation: SimulationReport,
    #[serde(default)]
    pub runtime_execution: RuntimeExecutionRecord,
    pub execution_effects: Vec<Effect>,
    pub invariants: InvariantReport,
    pub coordinates: Vec<ScaleCoordinate>,
    pub audit_triggered: bool,
    pub decision: TurnDecision,
    pub criteria_before: CanonicalCriteria,
    pub criteria_after: CanonicalCriteria,
    #[serde(default)]
    pub benchmark_report: Option<BenchmarkReport>,
    #[serde(default)]
    pub promotion_decision: Option<PromotionDecisionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalReceipt {
    pub version: String,
    pub deterministic: bool,
    pub turn: u64,
    pub timestamp: String,
    pub prev_hash: String,
    pub hash: String,
    
    // Captured execution trace elements for offline deterministic replay
    pub recorded_input: CanonicalInput,
    pub recorded_proposal: CanonicalProposal,
    pub recorded_observations: Vec<NodeObservation>,
    pub recorded_discoveries: Vec<NodeDiscovery>,

    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub proposal_quality: f32,
    pub decision: TurnDecision,
    pub gate_summary: String,
    #[serde(default)]
    pub runtime_execution: RuntimeExecutionRecord,
    #[serde(default)]
    pub benchmark_report: Option<BenchmarkReport>,
    #[serde(default)]
    pub promotion_decision: Option<PromotionDecisionRecord>,
    #[serde(default)]
    pub audit_triggered: bool,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f64")]
    pub simulation_max_risk: f64,
    pub invariant_passed: bool,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub evidence_coverage: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub contradiction_score: f32,
    pub coordinates: Vec<ScaleCoordinate>,
    #[serde(default)]
    pub cost: TurnCost,
    pub discovered_nodes: Vec<String>,
    pub violations: Vec<String>,
    pub criteria_before: CanonicalCriteria,
    pub criteria_after: CanonicalCriteria,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalConfig {}

impl Default for CanonicalConfig {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalState {
    pub session_id: String,
    pub turn_count: u64,
    pub graph: GraphState,
    pub receipts: Vec<CanonicalReceipt>,
    pub project_activation: Vec<(String, f32)>,
    pub last_turn_activation: Vec<(String, f32)>,
    #[serde(default)]
    pub live_control: LiveControlState,

    // Lane E: Trend and Motif tracking
    #[serde(default)]
    pub motif_counts: std::collections::HashMap<String, u64>,
    #[serde(default)]
    pub intervention_count: u64,
    #[serde(default)]
    pub contradiction_history: Vec<f32>,
}

impl Default for CanonicalState {
    fn default() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            turn_count: 0,
            graph: GraphState::default(),
            receipts: Vec::new(),
            project_activation: Vec::new(),
            last_turn_activation: Vec::new(),
            live_control: LiveControlState::default(),
            motif_counts: std::collections::HashMap::new(),
            intervention_count: 0,
            contradiction_history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotifReport {
    pub repeated_failures: u64,
    pub trend_slope: f32,
    pub passes_promotion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalTurnResult {
    pub trace: TurnTrace,
    pub receipt: CanonicalReceipt,
    pub discovered_nodes: Vec<String>,
}

fn default_max_risk() -> f64 {
    0.8
}

fn default_audit_rate() -> f32 {
    0.33
}

fn default_require_read_before_write() -> bool {
    true
}

fn default_min_evidence_coverage() -> f32 {
    0.7
}

fn default_contradiction_threshold() -> f32 {
    0.1
}

fn default_activation_cutoff() -> f32 {
    0.4
}

fn default_propagation_steps() -> usize {
    2
}

fn default_node_threshold() -> f32 {
    0.5
}

fn null_to_default_f32<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<f32>::deserialize(deserializer)?.unwrap_or_default())
}

fn null_to_default_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or_default())
}
