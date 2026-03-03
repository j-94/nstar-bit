use serde::{Deserialize, Serialize};

use crate::receipt::Effect;
use crate::utir::Operation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub condition: String,
    pub prime_id: u64,
    #[serde(default)]
    pub control_signals: Vec<String>,
    pub threshold: f32,
    pub activation: f32,
    pub discovered_turn: u64,
    pub reinforcements: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeKind {
    Supports,
    Inhibits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub weight: f32,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePattern {
    pub require_all: Vec<String>,
    pub block_any: Vec<String>,
    #[serde(default)]
    pub control_signals: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphState {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub patterns: Vec<GatePattern>,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeObservation {
    pub id: String,
    pub label: String,
    pub condition: String,
    pub activation: f32,
    #[serde(default)]
    pub control_signals: Vec<String>,
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDiscovery {
    pub id: String,
    pub label: String,
    pub condition: String,
    #[serde(default)]
    pub control_signals: Vec<String>,
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
    pub quality: f32,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        self.emitted_signals.iter().any(|s| s.to_lowercase() == sig.to_lowercase())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationReport {
    pub max_risk: f64,
    pub predicted_effects: Vec<String>,
    pub blocked_reasons: Vec<String>,
    pub can_materialize: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantReport {
    pub passed: bool,
    pub violations: Vec<String>,
    pub evidence_coverage: f32,
    pub contradiction_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnDecision {
    Commit,
    Rollback,
    Escalate,
    Halt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTrace {
    pub input: CanonicalInput,
    pub proposal: CanonicalProposal,
    pub gate: GateDecision,
    pub simulation: SimulationReport,
    pub execution_effects: Vec<Effect>,
    pub invariants: InvariantReport,
    pub coordinates: Vec<ScaleCoordinate>,
    pub audit_triggered: bool,
    pub decision: TurnDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalReceipt {
    pub version: String,
    pub deterministic: bool,
    pub turn: u64,
    pub timestamp: String,
    pub prev_hash: String,
    pub hash: String,
    #[serde(default)]
    pub proposal_quality: f32,
    pub decision: TurnDecision,
    pub gate_summary: String,
    #[serde(default)]
    pub audit_triggered: bool,
    #[serde(default)]
    pub simulation_max_risk: f64,
    pub invariant_passed: bool,
    #[serde(default)]
    pub evidence_coverage: f32,
    #[serde(default)]
    pub contradiction_score: f32,
    pub coordinates: Vec<ScaleCoordinate>,
    pub discovered_nodes: Vec<String>,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalConfig {
    pub max_risk: f64,
    pub audit_rate: f32,
    pub require_read_before_write: bool,
    pub min_evidence_coverage: f32,
    pub contradiction_threshold: f32,
    pub activation_cutoff: f32,
    pub propagation_steps: usize,
}

impl Default for CanonicalConfig {
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
pub struct CanonicalState {
    pub session_id: String,
    pub turn_count: u64,
    pub graph: GraphState,
    pub receipts: Vec<CanonicalReceipt>,
    pub project_activation: Vec<(String, f32)>,
    pub last_turn_activation: Vec<(String, f32)>,
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalTurnResult {
    pub trace: TurnTrace,
    pub receipt: CanonicalReceipt,
    pub discovered_nodes: Vec<String>,
}
