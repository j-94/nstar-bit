use serde::{Deserialize, Deserializer, Serialize};

fn null_to_default_f32<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<f32>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeExecutionRecord {
    #[serde(default)]
    pub executor_id: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub operation_count: usize,
    #[serde(default)]
    pub effect_count: usize,
    #[serde(default)]
    pub blocked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricComparator {
    AtLeast,
    AtMost,
}

impl Default for MetricComparator {
    fn default() -> Self {
        Self::AtLeast
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkMetric {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub value: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub target: f32,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub comparator: MetricComparator,
    #[serde(default)]
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkSuiteResult {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub metrics: Vec<BenchmarkMetric>,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkReport {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub macro_score: f32,
    #[serde(default)]
    pub suites: Vec<BenchmarkSuiteResult>,
    #[serde(default)]
    pub failing_action_gates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimePolicyCandidate {
    #[serde(default)]
    pub candidate_id: String,
    #[serde(default)]
    pub scoring_rule: String,
    #[serde(default)]
    pub selection_predicate: String,
    #[serde(default)]
    pub source_turn: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChampionPolicy {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub scoring_rule: String,
    #[serde(default)]
    pub selection_predicate: String,
    #[serde(default)]
    pub promoted_at_turn: u64,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub macro_score: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub evidence_coverage: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub contradiction_score: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub heldout_precision: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub heldout_recall: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromotionAction {
    Promote,
    Hold,
    Reject,
}

impl Default for PromotionAction {
    fn default() -> Self {
        Self::Hold
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromotionDecisionRecord {
    #[serde(default)]
    pub action: PromotionAction,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub benchmark_macro_score: f32,
    #[serde(default)]
    pub failing_action_gates: Vec<String>,
    #[serde(default)]
    pub candidate: Option<RuntimePolicyCandidate>,
    #[serde(default)]
    pub champion_before: Option<ChampionPolicy>,
    #[serde(default)]
    pub champion_after: Option<ChampionPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeSnapshot {
    #[serde(default)]
    pub turn: u64,
    #[serde(default)]
    pub decision: String,
    #[serde(default)]
    pub gate_summary: String,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub evidence_coverage: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub contradiction_score: f32,
    #[serde(default)]
    #[serde(deserialize_with = "null_to_default_f32")]
    pub deterministic_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LiveControlState {
    #[serde(default)]
    pub active_champion: Option<ChampionPolicy>,
    #[serde(default)]
    pub promotion_history: Vec<PromotionDecisionRecord>,
    #[serde(default)]
    pub latest_benchmark: Option<BenchmarkReport>,
    #[serde(default)]
    pub runtime_history: Vec<RuntimeSnapshot>,
    #[serde(default)]
    pub failing_action_gates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DerivedWarRoomView {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub active_champion: Option<String>,
    #[serde(default)]
    pub failing_action_gates: Vec<String>,
    #[serde(default)]
    pub stalled_critical_suites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityLedgerView {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub active_capabilities: Vec<String>,
    #[serde(default)]
    pub evidence_lane: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequirementItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub statement: String,
    #[serde(default)]
    pub strictness: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequirementsLockView {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub ready_for_implementation: bool,
    #[serde(default)]
    pub requirements: Vec<RequirementItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphProjectionView {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub node_count: usize,
    #[serde(default)]
    pub edge_count: usize,
    #[serde(default)]
    pub active_nodes: Vec<String>,
    #[serde(default)]
    pub hypothesis_edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoContribution {
    #[serde(default)]
    pub source_repo: String,
    #[serde(default)]
    pub contribution: String,
    #[serde(default)]
    pub target_subsystem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoAbsorptionMap {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub target_repo: String,
    #[serde(default)]
    pub contributions: Vec<RepoContribution>,
}
