//! Tool inputs and outputs. Accounts, assets, rates and roles are identified by
//! paths such as `loans/42`; quantities are decimal strings up to 2²⁵⁶ − 1.
//! Every map is keyed by ID and every collection may be omitted when empty.
//! Maps are ordered by ID, so the same entries produce the same economy handle
//! however a client orders its JSON keys.

use crate::handles::{AssessmentId, EconomyId, RefId, StudyId};
use axionomy::{
    AccountPath, AssetPath, ConditionPath, InvariantPath, Measure, ParameterPath, Quantity,
    RatePath, RolePath, paths,
};
use axionomy_engine::{Ended, ExperimentPath, ObjectivePath, PackagePath, StrategyPath, Version};
use axionomy_search::{GraphSearchProgress, pareto::ObjectiveDirection};
use axionomy_store::RefPath;
use axionomy_study::StudyKey;
use axionomy_view::DocumentPath;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, num::NonZeroUsize};

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

/// A stored economy and what it contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EconomyStored {
    pub economy_id: EconomyId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<EconomyId>,
    /// The same operation on the same parent was stored before; this is that
    /// economy.
    pub deduplicated: bool,
    /// The economy's content hash: economies with equal roots have equal
    /// contents, however they were made.
    pub root: StateRoot,
    pub counts: EconomyCounts,
}

/// An economy's content hash, as 64 hexadecimal digits.
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
            "description": "An economy's content hash: 64 lowercase hexadecimal digits. Economies with equal roots have equal accounts, balances, rates and invariants, however they were made.",
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
    /// The position, from zero, of the exchange that moved this quantity
    /// among those that made the economy: the exchange at index i takes step
    /// i to step i + 1. Absent from an assessment's lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_index: Option<usize>,
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
    pub issues: Vec<Issue>,
    /// Shortfalls of an infeasible exchange, or the projected changes of an
    /// applicable one.
    pub lines: usize,
    /// The first lines; the assessment's `lines` resource pages through all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub preview: Vec<QuantityLine>,
}

/// Whether an exchange would apply to an economy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStatus {
    /// It would apply; the lines are what it would move.
    Applicable,
    /// It is well formed, but accounts lack assets; the lines are what each
    /// lacks.
    Infeasible,
    /// It cannot apply to this economy; the issues say why.
    Invalid,
}

impl From<axionomy::AssessmentStatus> for AssessmentStatus {
    fn from(status: axionomy::AssessmentStatus) -> Self {
        match status {
            axionomy::AssessmentStatus::Applicable => Self::Applicable,
            axionomy::AssessmentStatus::Infeasible => Self::Infeasible,
            axionomy::AssessmentStatus::Invalid => Self::Invalid,
        }
    }
}

/// Why an exchange cannot apply: what is wrong, in words that say where to
/// look, and the paths involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Issue {
    pub message: String,
    #[serde(flatten)]
    pub kind: IssueKind,
}

/// What kind of issue an exchange has, with the paths it involves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IssueKind {
    /// The economy has no rate of this path.
    MissingRate { rate: RatePath },
    /// The exchange binds no account to one of the rate's roles.
    MissingBinding { role: RolePath },
    /// The exchange binds a role the rate does not have.
    UnknownBinding { role: RolePath },
    /// Two roles the rate keeps distinct bind the same account.
    RolesMustDiffer { left: RolePath, right: RolePath },
    /// A role binds an account the economy does not have.
    MissingAccount { account: AccountPath },
    /// The exchange's units are zero.
    ZeroUnits,
    /// The exchange lacks a parameter the rate needs.
    MissingParameter { name: ParameterPath },
    /// The exchange gives a parameter the rate does not take.
    UnknownParameter { name: ParameterPath },
    /// The units times one of the rate's amounts exceed 2²⁵⁶ − 1.
    RateOverflow { rate: RatePath, asset: AssetPath },
    /// A computed amount of the rate cannot be evaluated.
    RateLawEvaluation { rate: RatePath, reason: String },
    /// A condition of the rate does not hold.
    RateConditionFailed {
        rate: RatePath,
        condition: ConditionPath,
    },
    /// Accounts lack assets; an assessment's lines say how much.
    Infeasible,
    /// A balance would exceed 2²⁵⁶ − 1.
    BalanceOverflow {
        account: AccountPath,
        asset: AssetPath,
    },
    /// A conservation law would change.
    InvariantViolation {
        invariant: InvariantPath,
        before: Measure,
        after: Measure,
    },
    /// An account a role opens already exists.
    AccountExists { account: AccountPath },
    /// Another role binds the account a role opens.
    OpeningShared { role: RolePath },
    /// An account the exchange closes would not be empty.
    ClosedNotEmpty {
        account: AccountPath,
        remaining: Amounts,
    },
}

/// What exchanges made from the economy they were applied to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangedResponse {
    /// The economy the exchanges were applied to: the one passed to
    /// `axionomy_exchange_apply` or `axionomy_trace_replay`, or, for a step of
    /// a trace, the trace's own source, which is step zero.
    pub source_economy_id: EconomyId,
    /// The economy they made.
    pub economy_id: EconomyId,
    pub deduplicated: bool,
    pub root: StateRoot,
    /// How many exchanges made `economy_id` from `source_economy_id`.
    pub exchanges: usize,
    /// Quantity lines the exchanges moved; the economy's `changes` resource
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

/// A balance condition to find among the steps of a trace.
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

/// The first step of a trace where a balance condition holds.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceFindResponse {
    /// The first step where the condition holds: zero if it holds in the
    /// economy the exchanges were applied to, null if it holds at no step.
    pub step: Option<usize>,
    /// The economy of that step, as `axionomy_trace_step` gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economy: Option<TraceStepResponse>,
    /// Why no step was found, when none was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// The economy at one step of the trace that made another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TraceStepResponse {
    /// The economy whose trace was stepped through: the `economy_id` passed
    /// in, its last step.
    pub traced_economy_id: EconomyId,
    /// The step: how many of the trace's exchanges `economy_id` has applied
    /// to `source_economy_id`, the trace's source, which is step zero.
    pub step: usize,
    #[serde(flatten)]
    pub economy: ExchangedResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceStepRequest {
    /// An economy made by applying exchanges.
    pub economy_id: EconomyId,
    /// The step: how many of the exchanges that made the economy to apply to
    /// the economy they were applied to. Zero names that source, all of them
    /// the economy itself.
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
    /// The most states the search may expand before it stops without a
    /// solution.
    pub max_expansions: NonZeroUsize,
    /// State expansions between progress updates and cancellation checks,
    /// from 1 to 10,000.
    #[serde(default = "default_chunk_size")]
    #[schemars(range(min = 1, max = 10_000))]
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

/// The economy a solution reaches; its `exchanges` resource is the trace.
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
    pub problems: Vec<axionomy_study::ProblemDescriptor>,
}

/// A finished study of a problem. Its documents and their frames are
/// resources, and each document's trajectory is two economies: the economy it
/// starts from and the economy its exchanges reach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StudySummary {
    pub study_id: StudyId,
    /// What the study was computed from: the problem, instance, strategy, seed
    /// and budget.
    pub key: StudyKey,
    /// The document of the requested strategy's run.
    pub selected_document: DocumentPath,
    pub documents: Vec<DocumentSummary>,
}

/// One document of a study: one outcome, such as a strategy's run, as a
/// trajectory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSummary {
    pub id: DocumentPath,
    pub title: String,
    /// Frames, one per exchange of the trajectory.
    pub frames: usize,
    /// The economy the trajectory starts from, stored as an authored economy.
    pub initial_economy_id: EconomyId,
    /// The economy the trajectory reaches, made by applying its exchanges to
    /// the initial one: `axionomy_trace_step` with step N names the economy
    /// after N of them, the one frame N − 1 shows, and its `exchanges`
    /// resource is the trace.
    /// The initial economy itself when the trajectory has no exchanges.
    pub final_economy_id: EconomyId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefCreateRequest {
    /// The economy the ref names first: where the application starts.
    pub economy_id: EconomyId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefReadRequest {
    pub ref_id: RefId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefAdvanceRequest {
    pub ref_id: RefId,
    /// Where the mover expects the ref to be: the economy it made
    /// `economy_id` from. The move is refused, with where the ref is, when it
    /// is anywhere else.
    pub expected_economy_id: EconomyId,
    /// Where to move the ref: an economy made by exchanges from
    /// `expected_economy_id`, whose steps the move delivers to followers.
    pub economy_id: EconomyId,
}

/// Where a ref is: the economy an application following it is at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RefPosition {
    pub ref_id: RefId,
    /// The economy the ref names.
    pub economy_id: EconomyId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefFramesRequest {
    pub ref_id: RefId,
    /// The last economy the follower handled: the ref's economy when it last
    /// looked, or the last frame's. Frames are the steps after it.
    pub after_economy_id: EconomyId,
    /// The most frames to return, from 1 to 100; the next call follows from
    /// the last one returned.
    #[serde(default = "default_frame_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
    /// How long to wait, up to 30 seconds, for the ref to move when it is
    /// still at `after_economy_id`. Zero, the default, returns at once.
    #[serde(default)]
    #[schemars(range(max = 30))]
    pub wait_seconds: u64,
    /// Balances each named account must reach, as `axionomy_search` takes
    /// them; each frame then says whether its economy holds them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<BTreeMap<AccountPath, Amounts>>,
}

const fn default_frame_limit() -> usize {
    100
}

/// The steps a ref was moved over after the economy a follower last handled.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefFramesResponse {
    pub ref_id: RefId,
    /// Where the ref is now. While it is past the last frame, more frames
    /// follow it.
    pub economy_id: EconomyId,
    /// The steps, earliest first; empty when the ref has not moved.
    pub frames: Vec<RefFrame>,
}

/// One step a ref was moved over: the exchange, its receipt, and the economy
/// it made.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefFrame {
    /// The economy the step made, made by `exchange` from the frame before
    /// it, or from `after_economy_id` for the first frame.
    pub economy_id: EconomyId,
    /// The step's exchange, as `axionomy_exchange_apply` takes it.
    #[schemars(schema_with = "exchange_as_written")]
    pub exchange: paths::Exchange,
    /// What the exchange moved.
    pub receipt: FrameReceipt,
    /// Whether the economy holds the goal, when one was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holds: Option<bool>,
}

/// An exchange's schema as JSON writes it, which leaves out `parameters`
/// when it has none.
fn exchange_as_written(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let mut schema = <paths::Exchange as JsonSchema>::json_schema(generator);
    if let Some(required) = schema
        .get_mut("required")
        .and_then(serde_json::Value::as_array_mut)
    {
        required.retain(|field| field != "parameters");
    }
    schema
}

/// What a step's exchange did: the quantities it moved, flattened from its
/// accounts as an economy's `changes` resource lists them, and the accounts it
/// opened and closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FrameReceipt {
    pub changes: Vec<QuantityLine>,
    /// Accounts the exchange created, in role order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opened: Vec<AccountPath>,
    /// Accounts the exchange removed, in role order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closed: Vec<AccountPath>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageListRequest {}

/// The packages loaded into the server's store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageListResponse {
    pub packages: Vec<PackageSummary>,
}

/// One loaded package: what its manifest says of it, the points its refs
/// name, and the experiments and strategies it carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageSummary {
    pub name: PackagePath,
    pub version: PackageVersion,
    /// The store format the package is written in.
    pub format: u64,
    /// The script host API its scripts are written for.
    pub host_api: u64,
    /// Its refs that name a point, each with that point's economy.
    pub refs: Vec<PackageRef>,
    /// The experiments it carries, which axionomy_experiment_run runs.
    pub experiments: Vec<ExperimentPath>,
    /// The strategies its experiments run as arms.
    pub strategies: Vec<StrategyPath>,
}

/// A package's name for a point, such as an instance it starts from, and
/// the economy at that point. The name is the package's, not a `ref_id`:
/// to follow an application from the economy, make a ref with
/// `axionomy_ref_create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageRef {
    #[serde(rename = "ref")]
    pub reference: RefPath,
    pub economy_id: EconomyId,
}

/// A package's version, written `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageVersion(pub Version);

impl Serialize for PackageVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PackageVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        std::borrow::Cow::<str>::deserialize(deserializer)?
            .parse()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for PackageVersion {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PackageVersion".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "A package's version: three whole numbers, `major.minor.patch`, such as `1.0.2`.",
            "type": "string",
            "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+$"
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExperimentRequest {
    /// An experiment of a loaded package, such as `maze/micro`;
    /// `axionomy_package_list` lists them.
    pub experiment: ExperimentPath,
}

/// What an experiment's trials did: every arm's, measured on the same
/// objectives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExperimentResults {
    pub experiment: ExperimentPath,
    /// The objectives every trial is measured on, in the experiment's order.
    pub objectives: Vec<ExperimentObjective>,
    pub arms: Vec<ArmResults>,
}

/// An objective and the direction that improves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExperimentObjective {
    pub objective: ObjectivePath,
    pub direction: ObjectiveDirection,
}

/// One arm: a strategy, from its start, over the experiment's trials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArmResults {
    pub strategy: StrategyPath,
    /// The package's ref the arm starts from: the experiment's start, or
    /// the arm's own, such as a counterfactual start.
    pub start: RefPath,
    /// The economy every trial of the arm starts from.
    pub start_economy_id: EconomyId,
    pub trials: Vec<TrialResult>,
}

/// One trial: one seeded run of an arm's strategy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrialResult {
    /// The trial's seed, drawn from the experiment's seed and the trial's
    /// position, so every arm meets the same chances.
    pub seed: u64,
    pub ended: TrialEnded,
    /// The experiment's objectives where the trial ended, exactly.
    pub values: BTreeMap<ObjectivePath, Quantity>,
    /// The economy the trial reached, made by `exchanges` exchanges from
    /// the arm's start: `axionomy_trace_step` names each step, and its
    /// `exchanges` resource is the trace. The start itself when the trial
    /// applied none.
    pub economy_id: EconomyId,
    pub exchanges: u64,
}

/// How a trial's run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrialEnded {
    /// The strategy's goal holds where the run ends.
    Reached,
    /// The policy proposed nothing, or a plan ended short of the goal.
    Stopped,
    /// The policy used its every turn.
    Horizon,
    /// The policy proposed an exchange that does not apply on turn `turn`,
    /// counting from zero; the run ends before it.
    Refused { turn: u64 },
    /// The search explored everything its candidates reach without the
    /// goal.
    Exhausted,
    /// The search spent the experiment's budget first.
    Spent,
    /// The run was stopped before it ended.
    Interrupted,
}

impl From<Ended> for TrialEnded {
    fn from(ended: Ended) -> Self {
        match ended {
            Ended::Reached => Self::Reached,
            Ended::Stopped => Self::Stopped,
            Ended::Horizon => Self::Horizon,
            Ended::Refused { turn } => Self::Refused { turn },
            Ended::Exhausted => Self::Exhausted,
            Ended::Spent => Self::Spent,
            Ended::Interrupted => Self::Interrupted,
        }
    }
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
    /// The arguments are malformed or out of range; `details.pointer` is the
    /// JSON pointer of the argument at fault.
    InvalidRequest,
    /// What a handle or path names does not exist: an economy, assessment,
    /// study or ref this server's store does not hold, an account no
    /// economy of the trace has, an experiment no loaded package carries, or
    /// results an experiment has not recorded.
    NotFound,
    /// An entry would replace one the economy already has.
    Conflict,
    /// A ref is not where a move expected it: another move came first.
    /// `details.current_economy_id` names where it is, to look again from.
    Moved,
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
