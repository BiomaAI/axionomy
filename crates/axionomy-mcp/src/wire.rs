//! Tool inputs and outputs. IDs are strings and quantities are decimal strings
//! up to 2²⁵⁶ − 1. Every map is keyed by ID and every collection may be
//! omitted when empty. Maps are ordered by ID, so the same entries produce the
//! same snapshot handle however a client orders its JSON keys.

use axionomy::{
    ApplyError, AssessmentStatus, Economy, Exchange, ExchangeAssessment, Quantity,
    QuantityExpression, RateCondition, Receipt,
};
use axionomy_search::GraphSearchProgress;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type WireEconomy = Economy<String, String, String, String>;
pub type WireExchange = Exchange<String, String, String>;
pub type WireReceipt = Receipt<String, String, String, String>;
pub type WireAssessment = ExchangeAssessment<String, String, String, String>;
pub type WireApplyError = ApplyError<String, String, String, String>;
pub type WireCondition = RateCondition<String, String>;
pub type WireExpression = QuantityExpression<String, String>;

/// Quantities by asset ID.
pub type Amounts = BTreeMap<String, Quantity>;

/// Entries to add to an economy, at most 10,000 per call. Adding never
/// replaces: an account must not already hold an asset it is given, and a
/// rate role or invariant must not already list an asset it is given.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomyEntries {
    /// Balances by account ID. A new account ID opens that account; `{}` opens
    /// it empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub accounts: BTreeMap<String, Amounts>,
    /// Rates by rate ID. A new rate ID adds that rate; an existing one gains
    /// the given amounts, role pairs and conditions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rates: BTreeMap<String, RateEntries>,
    /// Linear conservation laws by name: an integer weight per asset. Every
    /// exchange must leave each law's weighted sum over all accounts unchanged.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub invariants: BTreeMap<String, BTreeMap<String, i64>>,
}

/// What one unit of an exchange moves, by role. An exchange binds each role to
/// an account and scales the amounts by its `units`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RateEntries {
    /// Assets taken from each role's account.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub consume: BTreeMap<String, Amounts>,
    /// Assets given to each role's account.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub produce: BTreeMap<String, Amounts>,
    /// Assets each role's account must hold and keeps.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub preserve: BTreeMap<String, Amounts>,
    /// Role pairs that must bind different accounts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub distinct: Vec<(String, String)>,
    /// Amounts evaluated from balances, units and parameters when an exchange
    /// is assessed, rounded in their stated direction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub computed: Vec<ComputedEntry>,
    /// Comparisons every exchange must satisfy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<WireCondition>,
}

/// One amount computed when an exchange is assessed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComputedEntry {
    pub direction: Direction,
    pub role: String,
    pub asset: String,
    pub quantity: WireExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Consume,
    Produce,
    Preserve,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomyCreateRequest {
    #[serde(default)]
    pub entries: EconomyEntries,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomyExtendRequest {
    pub economy_id: String,
    pub entries: EconomyEntries,
}

/// A stored snapshot and what it contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomyStored {
    pub economy_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// The same operation on the same parent was stored before; this is that
    /// snapshot.
    pub deduplicated: bool,
    pub counts: EconomyCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomyCounts {
    pub accounts: usize,
    /// Distinct assets held, moved by a rate, or weighted by an invariant.
    pub assets: usize,
    pub rates: usize,
    pub invariants: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExchangeRequest {
    pub economy_id: String,
    pub exchange: WireExchange,
}

/// One quantity an exchange moves or lacks, flattened from its accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuantityLine {
    /// Position of the exchange in its trace, for snapshots made by several.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<usize>,
    pub account: String,
    pub asset: String,
    pub kind: LineKind,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Consumed,
    Produced,
    Preserved,
    /// The account lacks this much for the exchange.
    Missing,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AssessResponse {
    pub economy_id: String,
    pub assessment_id: String,
    pub status: AssessmentStatus,
    /// Why an invalid exchange cannot apply.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<WireApplyError>,
    /// Shortfalls of an infeasible exchange, or the projected changes of an
    /// applicable one.
    pub lines: usize,
    /// The first lines; the assessment's `lines` resource pages through all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub preview: Vec<QuantityLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangedResponse {
    pub source_economy_id: String,
    pub economy_id: String,
    pub deduplicated: bool,
    pub exchanges: usize,
    /// Quantity lines the exchanges moved; the snapshot's `changes` resource
    /// pages through them.
    pub changes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplayRequest {
    pub economy_id: String,
    /// At most 1,000 exchanges, applied in order; the first failure stops the
    /// replay and stores nothing.
    pub exchanges: Vec<WireExchange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    pub economy_id: String,
    /// Balances each named account must reach.
    pub goal: BTreeMap<String, Amounts>,
    /// At most 1,000 concrete exchanges, reconsidered at every state.
    pub candidates: Vec<WireExchange>,
    /// Maximum states the search may expand before completing without a solution.
    pub max_expansions: usize,
    /// Deterministic number of state expansions between observable progress updates.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
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
    pub progress: GraphSearchProgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution: Option<SearchSolved>,
}

/// The snapshot a solution reaches; its `exchanges` resource is the trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchSolved {
    pub economy_id: String,
    pub exchanges: usize,
    pub cost: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProblemCatalogRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProblemCatalogResponse {
    pub problems: Vec<axionomy_service::ProblemDescriptor>,
}

/// A finished problem run. Its documents and their frames are resources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunSummary {
    pub artifact_id: String,
    pub problem: String,
    pub instance: String,
    pub selected_document_id: String,
    pub documents: Vec<DocumentSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub frames: usize,
}

/// A failed tool call: a stable code, a message, and what the caller needs to
/// correct it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolFailure {
    pub error: FailureCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    /// The arguments are malformed or out of range.
    InvalidRequest,
    /// The named snapshot does not exist, or the server restarted since.
    NotFound,
    /// An entry would replace one the economy already has.
    Conflict,
    /// The request exceeds a per-call limit; split it.
    TooLarge,
    /// The exchange is well formed but accounts lack assets.
    Infeasible,
    /// The exchange is malformed for this economy or violates an invariant.
    Invalid,
    /// The tool runs as an MCP task and the client did not declare the tasks
    /// extension.
    TasksRequired,
    Internal,
}
