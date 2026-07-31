use axionomy::{Economy, Exchange, ExchangeAssessment, Goal, Receipt, Trace};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type WireEconomy = Economy<String, String, String, String, u64>;
pub type WireExchange = Exchange<String, String, String, u64>;
pub type WireGoal = Goal<String, String, u64>;
pub type WireTrace = Trace<String, String, String, u64>;
pub type WireReceipt = Receipt<String, String, String, String, u64>;
pub type WireAssessment = ExchangeAssessment<String, String, String, String, u64>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomyHandle {
    pub economy_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EconomyPutRequest {
    pub economy: WireEconomy,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EconomyPutResponse {
    pub economy_id: String,
    pub deduplicated: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AssessRequest {
    pub economy_id: String,
    pub exchange: WireExchange,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AssessResponse {
    pub economy_id: String,
    pub status: axionomy::AssessmentStatus,
    pub assessment: WireAssessment,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ApplyRequest {
    pub economy_id: String,
    pub exchange: WireExchange,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplyResponse {
    pub source_economy_id: String,
    pub economy_id: String,
    pub receipt: WireReceipt,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ReplayRequest {
    pub economy_id: String,
    pub trace: WireTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReplayResponse {
    pub source_economy_id: String,
    pub economy_id: String,
    pub receipts: Vec<WireReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchRequest {
    pub economy_id: String,
    pub goal: WireGoal,
    /// Complete, caller-owned action universe reconsidered at every state.
    pub candidates: Vec<WireExchange>,
    /// Maximum states the search may expand before completing without a solution.
    pub max_expansions: usize,
    /// Deterministic number of state expansions between persisted progress updates.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    /// Caller-stable key for retrying the same logical search request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

const fn default_chunk_size() -> usize {
    256
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchOutcome {
    Solved,
    Exhausted,
    ExpansionLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResponse {
    pub economy_id: String,
    pub outcome: SearchOutcome,
    pub progress: axionomy_search::GraphSearchProgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution: Option<axionomy_search::SearchSolution<String, String, String, u64>>,
}
