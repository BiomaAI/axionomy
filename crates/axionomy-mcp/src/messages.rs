//! Tool inputs and outputs. Accounts, assets, rates and roles are identified by
//! paths such as `loans/42`; quantities are decimal strings up to 2²⁵⁶ − 1.
//! Every map is keyed by ID and every collection may be omitted when empty.
//! Maps are ordered by ID, so the same entries produce the same snapshot handle
//! however a client orders its JSON keys.

use axionomy::InvariantPath;

use crate::handles::{AssessmentId, EconomyId, StudyId};
use axionomy::{AccountPath, AssessmentStatus, AssetPath, Quantity, RatePath, RolePath, paths};
use axionomy_search::GraphSearchProgress;
use axionomy_service::StudyKey;
use axionomy_view::DocumentPath;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Quantities by asset.
pub type Amounts = BTreeMap<AssetPath, Quantity>;

/// Entries to add to an economy, at most 10,000 per call. Adding never
/// replaces: an account must not already hold an asset it is given, and a
/// rate role or invariant must not already list an asset it is given.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomyEntries {
    /// Balances by account ID. A new account ID opens that account; `{}` opens
    /// it empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub accounts: BTreeMap<AccountPath, Amounts>,
    /// Rates by rate ID. A new rate ID adds that rate; an existing one gains
    /// the given amounts, role pairs and conditions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rates: BTreeMap<RatePath, RateEntries>,
    /// Linear conservation laws by path: an integer weight per asset. Every
    /// exchange must leave each law's weighted sum over all accounts unchanged.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub invariants: BTreeMap<InvariantPath, BTreeMap<AssetPath, i64>>,
}

/// What one unit of an exchange moves, by role. An exchange binds each role to
/// an account and scales the amounts by its `units`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RateEntries {
    /// Assets taken from each role's account.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub consume: BTreeMap<RolePath, Amounts>,
    /// Assets given to each role's account.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub produce: BTreeMap<RolePath, Amounts>,
    /// Assets each role's account must hold and keeps.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub preserve: BTreeMap<RolePath, Amounts>,
    /// Role pairs that must bind different accounts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub distinct: Vec<(RolePath, RolePath)>,
    /// Amounts evaluated from balances, units and parameters when an exchange
    /// is assessed, rounded in their stated direction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub computed: Vec<ComputedEntry>,
    /// Comparisons every exchange must satisfy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<paths::RateCondition>,
    /// Roles whose bound account an exchange of this rate creates, empty,
    /// before its effects. The exchange names the new account's path; it must
    /// not exist, and no other role may bind it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opens: Vec<RolePath>,
    /// Roles whose bound account an exchange of this rate removes. Its effects
    /// must leave that account empty, so closing never destroys anything; the
    /// path may be opened again later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closes: Vec<RolePath>,
}

/// One amount computed when an exchange is assessed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComputedEntry {
    pub direction: Direction,
    pub role: RolePath,
    pub asset: AssetPath,
    pub quantity: paths::QuantityExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Consume,
    Produce,
    Preserve,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Consume => "consume",
            Self::Produce => "produce",
            Self::Preserve => "preserve",
        })
    }
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
    pub economy_id: EconomyId,
    pub entries: EconomyEntries,
}

/// A stored snapshot and what it contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomyStored {
    pub economy_id: EconomyId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<EconomyId>,
    /// The same operation on the same parent was stored before; this is that
    /// snapshot.
    pub deduplicated: bool,
    /// The state's content hash: snapshots with equal roots hold equal
    /// states, however they were made.
    pub root: StateRoot,
    pub counts: EconomyCounts,
}

/// A state's content hash, as 64 hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateRoot(pub axionomy_tree::Hash);

impl Serialize for StateRoot {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0.to_hex())
    }
}

impl<'de> Deserialize<'de> for StateRoot {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = std::borrow::Cow::<str>::deserialize(deserializer)?;
        axionomy_tree::Hash::from_hex(text.as_bytes())
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for StateRoot {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "StateRoot".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "A state's content hash: 64 lowercase hexadecimal digits.",
            "type": "string",
            "pattern": "^[0-9a-f]{64}$"
        })
    }
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
    pub economy_id: EconomyId,
    pub exchange: paths::Exchange,
}

/// One quantity an exchange moves or lacks, flattened from its accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuantityLine {
    /// Position of the exchange in its trace, for snapshots made by several.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<usize>,
    pub account: AccountPath,
    pub asset: AssetPath,
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
    pub economy_id: EconomyId,
    pub assessment_id: AssessmentId,
    pub status: AssessmentStatus,
    /// Why an invalid exchange cannot apply.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<paths::ApplyError>,
    /// Shortfalls of an infeasible exchange, or the projected changes of an
    /// applicable one.
    pub lines: usize,
    /// The first lines; the assessment's `lines` resource pages through all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub preview: Vec<QuantityLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangedResponse {
    pub source_economy_id: EconomyId,
    pub economy_id: EconomyId,
    pub deduplicated: bool,
    pub root: StateRoot,
    pub exchanges: usize,
    /// Quantity lines the exchanges moved; the snapshot's `changes` resource
    /// pages through them.
    pub changes: usize,
    /// Accounts the exchanges created, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opened: Vec<AccountPath>,
    /// Accounts the exchanges removed, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closed: Vec<AccountPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplayRequest {
    pub economy_id: EconomyId,
    /// At most 1,000 exchanges, applied in order; the first failure stops the
    /// replay and stores nothing.
    pub exchanges: Vec<paths::Exchange>,
}

/// A balance condition to find in the states of a trace.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceFindRequest {
    /// An economy made by applying exchanges.
    pub economy_id: EconomyId,
    pub account: AccountPath,
    pub asset: AssetPath,
    /// How the account's balance of the asset compares with `quantity`.
    pub comparison: axionomy::QuantityComparison,
    pub quantity: Quantity,
}

/// The first state of a trace where a balance condition holds.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceFindResponse {
    /// How many of the exchanges had been applied to the parent when the
    /// condition first holds: zero if it holds in the parent's state. Absent
    /// when it holds in no state of the trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<usize>,
    /// That state, as `axionomy_trace_step` gives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ExchangedResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceStepRequest {
    /// An economy made by applying exchanges.
    pub economy_id: EconomyId,
    /// How many of the exchanges that made it to apply to its parent: zero
    /// for the parent's state, all of them for the economy itself.
    pub step: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    pub economy_id: EconomyId,
    /// Balances each named account must reach.
    pub goal: BTreeMap<AccountPath, Amounts>,
    /// At most 1,000 concrete exchanges, reconsidered at every state.
    pub candidates: Vec<paths::Exchange>,
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
    pub economy_id: EconomyId,
    pub outcome: SearchOutcome,
    pub progress: GraphSearchProgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution: Option<SearchSolved>,
}

/// The snapshot a solution reaches; its `exchanges` resource is the trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchSolved {
    pub economy_id: EconomyId,
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

/// A finished study of a problem. Its documents and their frames are
/// resources, and each document's trajectory is two economies: the state it
/// starts from and the state its exchanges reach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StudySummary {
    pub study_id: StudyId,
    /// What the study was computed from: the problem, instance, strategy, seed
    /// and budget.
    pub key: StudyKey,
    /// The document of the requested strategy.
    pub selected_document: DocumentPath,
    pub documents: Vec<DocumentSummary>,
}

/// One document of a study: one strategy's trajectory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSummary {
    pub id: DocumentPath,
    pub title: String,
    /// Frames, one per exchange of the trajectory.
    pub frames: usize,
    /// The state the trajectory starts from, stored as an authored economy.
    pub initial_economy_id: EconomyId,
    /// The state the trajectory reaches: an economy made by applying its
    /// exchanges to the initial one, so `axionomy_trace_step` names the state
    /// after any number of them and its `exchanges` resource is the trace.
    /// The initial economy itself when the trajectory has no exchanges.
    pub final_economy_id: EconomyId,
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
