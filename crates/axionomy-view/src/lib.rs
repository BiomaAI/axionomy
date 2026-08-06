//! Runtime-neutral playback and presentation contracts for Axionomy.
//!
//! This crate deliberately contains no world model of its own. A view document
//! is derived by replaying exchanges through [`axionomy::Economy`], and optional
//! scenes are read-only projections supplied by the caller.

use axionomy::{
    AccountAssessment, AccountDelta, Basket, Economy, Exchange, ExchangeAssessment, Goal, Quantity,
    QuantityScalar, Receipt, Trace,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt::Debug, hash::Hash, marker::PhantomData};
use thiserror::Error;

/// A browser-safe identity for an arbitrary user-defined ontology value.
///
/// `key` is used for joins, `label` is for people, and `value` is optional
/// diagnostic context. None of these fields replaces the underlying Rust value
/// as semantic authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewId {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

impl ViewId {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: None,
        }
    }

    pub fn with_value(mut self, value: impl Serialize) -> Result<Self, serde_json::Error> {
        self.value = Some(serde_json::to_value(value)?);
        Ok(self)
    }
}

/// An exact, non-negative quantity encoded as text for JavaScript safety.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ExactQuantity(pub String);

impl<N> From<&Quantity<N>> for ExactQuantity
where
    N: QuantityScalar,
{
    fn from(value: &Quantity<N>) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AssetQuantityView {
    pub asset: ViewId,
    pub quantity: ExactQuantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountView {
    pub account: ViewId,
    pub balances: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoleBindingView {
    pub role: ViewId,
    pub account: ViewId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeView {
    pub rate: ViewId,
    pub units: ExactQuantity,
    pub bindings: Vec<RoleBindingView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountDeltaView {
    pub account: ViewId,
    pub consumed: Vec<AssetQuantityView>,
    pub produced: Vec<AssetQuantityView>,
    pub preserved: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountAssessmentView {
    pub account: ViewId,
    pub available: Vec<AssetQuantityView>,
    pub required: Vec<AssetQuantityView>,
    pub consumed: Vec<AssetQuantityView>,
    pub produced: Vec<AssetQuantityView>,
    pub preserved: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStatusView {
    Applicable,
    Infeasible,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AssessmentView {
    pub status: AssessmentStatusView,
    pub accounts: Vec<AccountAssessmentView>,
    pub projected_deltas: Vec<AccountDeltaView>,
    pub shortfalls: Vec<AccountShortfallView>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountShortfallView {
    pub account: ViewId,
    pub missing: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReceiptView {
    pub deltas: Vec<AccountDeltaView>,
}

/// A rate role's complete consume/produce/preserve contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateRoleView {
    pub role: ViewId,
    pub consumed: Vec<AssetQuantityView>,
    pub produced: Vec<AssetQuantityView>,
    pub preserved: Vec<AssetQuantityView>,
}

/// One immutable transition rule in the closed model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateView {
    pub rate: ViewId,
    pub roles: Vec<RateRoleView>,
    pub distinct_roles: Vec<[ViewId; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GoalRequirementView {
    pub account: ViewId,
    pub required: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvariantTermView {
    pub asset: ViewId,
    pub weight: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvariantView {
    pub name: String,
    pub terms: Vec<InvariantTermView>,
}

/// Read-only explanation of the authoritative economy definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelView {
    pub rates: Vec<RateView>,
    pub goal: Vec<GoalRequirementView>,
    pub invariants: Vec<InvariantView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GraphNodeView {
    pub id: ViewId,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphEdgeView {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
}

/// Optional domain projections. They explain economic state but never govern it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scene {
    Graph {
        title: String,
        nodes: Vec<GraphNodeView>,
        edges: Vec<GraphEdgeView>,
        #[serde(skip_serializing_if = "Option::is_none")]
        focus: Option<String>,
    },
    Grid {
        title: String,
        width: u32,
        height: u32,
        cells: Vec<GridCellView>,
    },
    Matrix {
        title: String,
        rows: Vec<ViewId>,
        columns: Vec<ViewId>,
        cells: Vec<MatrixCellView>,
    },
    Timeline {
        title: String,
        lanes: Vec<TimelineLaneView>,
        spans: Vec<TimelineSpanView>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GridCellView {
    pub x: u32,
    pub y: u32,
    pub label: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatrixCellView {
    pub row: String,
    pub column: String,
    pub label: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineLaneView {
    pub id: ViewId,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineSpanView {
    pub id: String,
    pub lane: String,
    pub start: u64,
    pub end: u64,
    pub label: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewSnapshot {
    pub index: u64,
    pub accounts: Vec<AccountView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<Scene>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeFrame {
    pub index: u64,
    pub exchange: ExchangeView,
    pub assessment: AssessmentView,
    pub receipt: ReceiptView,
    pub before: ViewSnapshot,
    pub after: ViewSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectiveView {
    pub key: String,
    pub label: String,
    pub direction: ObjectiveDirectionView,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveDirectionView {
    Minimize,
    Maximize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontierCompletenessView {
    Exact,
    Approximate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectiveAxisView {
    pub key: String,
    pub label: String,
    pub direction: ObjectiveDirectionView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParetoPointView {
    pub label: String,
    /// Ordered exactly like `ParetoFrontView.axes`; values remain exact text.
    pub values: Vec<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParetoFrontView {
    pub title: String,
    pub completeness: FrontierCompletenessView,
    pub axes: Vec<ObjectiveAxisView>,
    pub points: Vec<ParetoPointView>,
}

/// A proposal assessed against an exact snapshot, including rejected actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProposalView {
    pub id: String,
    pub label: String,
    pub description: String,
    pub snapshot_index: u64,
    pub exchange: ExchangeView,
    pub assessment: AssessmentView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryKindView {
    Expanded,
    Generated,
    Iteration,
    Sample,
    InformationSet,
    Message,
}

/// A transport-neutral progress observation. Counters are exact text so the
/// same contract remains safe in native and JavaScript consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryPointView {
    pub sequence: u64,
    pub kind: TelemetryKindView,
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchTelemetryView {
    pub algorithm: String,
    pub exact: bool,
    #[serde(default)]
    pub points: Vec<TelemetryPointView>,
}

/// An actor-relative observation. Omitted accounts and assets are intentionally
/// not visible to that actor; this is not a filtered copy of authoritative
/// state pretending to be complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObservationView {
    pub actor: ViewId,
    pub label: String,
    pub visible_accounts: Vec<AccountView>,
    #[serde(default)]
    pub facts: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewDocument {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source: ViewSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelView>,
    pub initial: ViewSnapshot,
    pub frames: Vec<ExchangeFrame>,
    #[serde(default)]
    pub objectives: Vec<ObjectiveView>,
    #[serde(default)]
    pub pareto_fronts: Vec<ParetoFrontView>,
    #[serde(default)]
    pub proposals: Vec<ProposalView>,
    #[serde(default)]
    pub telemetry: Vec<SearchTelemetryView>,
    #[serde(default)]
    pub observations: Vec<ObservationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewSource {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewDocumentMetadata {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source: ViewSource,
}

/// Lifecycle events emitted by a Studio run. `sequence` orders transport
/// events; it is not economic time and has no effect on replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StudioEvent {
    RunStarted {
        run_id: String,
        sequence: u64,
        problem: String,
        strategy: String,
    },
    Progress {
        run_id: String,
        sequence: u64,
        completed: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
        message: String,
    },
    FrameAppended {
        run_id: String,
        sequence: u64,
        frame_index: u64,
    },
    ArtifactPublished {
        run_id: String,
        sequence: u64,
        artifact_id: String,
        documents: u64,
    },
    RunPaused {
        run_id: String,
        sequence: u64,
    },
    RunResumed {
        run_id: String,
        sequence: u64,
    },
    RunCompleted {
        run_id: String,
        sequence: u64,
        artifact_id: String,
        document_id: String,
    },
    RunCancelled {
        run_id: String,
        sequence: u64,
    },
    RunFailed {
        run_id: String,
        sequence: u64,
        message: String,
    },
}

impl StudioEvent {
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::RunStarted { sequence, .. }
            | Self::Progress { sequence, .. }
            | Self::FrameAppended { sequence, .. }
            | Self::ArtifactPublished { sequence, .. }
            | Self::RunPaused { sequence, .. }
            | Self::RunResumed { sequence, .. }
            | Self::RunCompleted { sequence, .. }
            | Self::RunCancelled { sequence, .. }
            | Self::RunFailed { sequence, .. } => *sequence,
        }
    }

    pub fn run_id(&self) -> &str {
        match self {
            Self::RunStarted { run_id, .. }
            | Self::Progress { run_id, .. }
            | Self::FrameAppended { run_id, .. }
            | Self::ArtifactPublished { run_id, .. }
            | Self::RunPaused { run_id, .. }
            | Self::RunResumed { run_id, .. }
            | Self::RunCompleted { run_id, .. }
            | Self::RunCancelled { run_id, .. }
            | Self::RunFailed { run_id, .. } => run_id,
        }
    }
}

/// Converts user ontology values into stable, browser-facing identities and
/// may derive a scene from each replayed economy snapshot.
pub trait ViewOntology<AccountId, A, RateId, Role, N = u64> {
    fn account(&self, id: &AccountId) -> ViewId;
    fn asset(&self, id: &A) -> ViewId;
    fn rate(&self, id: &RateId) -> ViewId;
    fn role(&self, id: &Role) -> ViewId;

    fn scene(
        &self,
        _index: u64,
        _economy: &Economy<AccountId, A, RateId, Role, N>,
    ) -> Option<Scene> {
        None
    }
}

/// A useful default for user ontologies and reference fixtures. It preserves
/// exact typed engine values internally while deriving stable diagnostic IDs
/// from `Debug` output at the presentation boundary.
type OntologyMarker<AccountId, A, RateId, Role, N> =
    PhantomData<fn() -> (AccountId, A, RateId, Role, N)>;

pub struct DebugOntology<AccountId, A, RateId, Role, N = u64, SceneFn = NoScene> {
    namespace: String,
    scene: SceneFn,
    marker: OntologyMarker<AccountId, A, RateId, Role, N>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoScene;

impl<AccountId, A, RateId, Role, N> DebugOntology<AccountId, A, RateId, Role, N, NoScene> {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            scene: NoScene,
            marker: PhantomData,
        }
    }
}

impl<AccountId, A, RateId, Role, N, CurrentScene>
    DebugOntology<AccountId, A, RateId, Role, N, CurrentScene>
{
    pub fn with_scene<SceneFn>(
        self,
        scene: SceneFn,
    ) -> DebugOntology<AccountId, A, RateId, Role, N, SceneFn> {
        DebugOntology {
            namespace: self.namespace,
            scene,
            marker: PhantomData,
        }
    }

    fn id<T: Debug>(&self, category: &str, value: &T) -> ViewId {
        let debug = format!("{value:?}");
        ViewId::new(
            format!("{}:{category}:{}", self.namespace, debug_key(&debug)),
            humanize_debug(&debug),
        )
    }
}

impl<AccountId, A, RateId, Role, N> ViewOntology<AccountId, A, RateId, Role, N>
    for DebugOntology<AccountId, A, RateId, Role, N, NoScene>
where
    AccountId: Debug,
    A: Debug,
    RateId: Debug,
    Role: Debug,
{
    fn account(&self, id: &AccountId) -> ViewId {
        self.id("account", id)
    }

    fn asset(&self, id: &A) -> ViewId {
        self.id("asset", id)
    }

    fn rate(&self, id: &RateId) -> ViewId {
        self.id("rate", id)
    }

    fn role(&self, id: &Role) -> ViewId {
        self.id("role", id)
    }
}

impl<AccountId, A, RateId, Role, N, SceneFn> ViewOntology<AccountId, A, RateId, Role, N>
    for DebugOntology<AccountId, A, RateId, Role, N, SceneFn>
where
    AccountId: Debug,
    A: Debug,
    RateId: Debug,
    Role: Debug,
    SceneFn: Fn(u64, &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene>,
{
    fn account(&self, id: &AccountId) -> ViewId {
        self.id("account", id)
    }

    fn asset(&self, id: &A) -> ViewId {
        self.id("asset", id)
    }

    fn rate(&self, id: &RateId) -> ViewId {
        self.id("rate", id)
    }

    fn role(&self, id: &Role) -> ViewId {
        self.id("role", id)
    }

    fn scene(&self, index: u64, economy: &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene> {
        (self.scene)(index, economy)
    }
}

fn debug_key(debug: &str) -> String {
    debug
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn humanize_debug(debug: &str) -> String {
    let mut output = String::with_capacity(debug.len() + 4);
    let mut previous_lowercase = false;
    for character in debug.chars() {
        if matches!(character, '(' | ')' | '{' | '}' | '[' | ']') {
            output.push(' ');
            previous_lowercase = false;
        } else if character == ',' {
            output.push_str(", ");
            previous_lowercase = false;
        } else if character == '_' {
            output.push(' ');
            previous_lowercase = false;
        } else {
            if character.is_ascii_uppercase() && previous_lowercase {
                output.push(' ');
            }
            output.push(character);
            previous_lowercase = character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlaybackError {
    #[error("trace exchange {index} failed replay: {message}")]
    Replay { index: u64, message: String },
}

/// Replays a trace and materializes a portable, self-contained view document.
pub fn derive_document<AccountId, A, RateId, Role, N, O>(
    metadata: ViewDocumentMetadata,
    initial: &Economy<AccountId, A, RateId, Role, N>,
    trace: &Trace<RateId, Role, AccountId, N>,
    ontology: &O,
    objectives: Vec<ObjectiveView>,
) -> Result<ViewDocument, PlaybackError>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    let mut economy = initial.fork();
    let initial = snapshot(0, &economy, ontology);
    let mut frames = Vec::with_capacity(trace.exchanges().len());

    for (offset, exchange) in trace.exchanges().iter().enumerate() {
        let index = offset as u64;
        let before = snapshot(index, &economy, ontology);
        let assessment = assessment_view(&economy.assess(exchange), ontology);
        let receipt = economy
            .apply(exchange.clone())
            .map_err(|error| PlaybackError::Replay {
                index,
                message: error.to_string(),
            })?;
        let after = snapshot(index + 1, &economy, ontology);
        frames.push(ExchangeFrame {
            index,
            exchange: exchange_view(exchange, ontology),
            assessment,
            receipt: receipt_view(&receipt, ontology),
            before,
            after,
        });
    }

    Ok(ViewDocument {
        id: metadata.id,
        title: metadata.title,
        description: metadata.description,
        source: metadata.source,
        model: None,
        initial,
        frames,
        objectives,
        pareto_fronts: Vec::new(),
        proposals: Vec::new(),
        telemetry: Vec::new(),
        observations: Vec::new(),
    })
}

/// Derives the immutable model definition used by a view document.
pub fn derive_model<AccountId, A, RateId, Role, N, O>(
    economy: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    ontology: &O,
) -> ModelView
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    let mut rates = economy
        .rate_ids()
        .filter_map(|id| {
            let rate = economy.rate(id)?;
            let roles = rate
                .roles()
                .map(|role| RateRoleView {
                    role: ontology.role(role),
                    consumed: rate
                        .consumed(role)
                        .map_or_else(Vec::new, |basket| basket_view(basket, ontology)),
                    produced: rate
                        .produced(role)
                        .map_or_else(Vec::new, |basket| basket_view(basket, ontology)),
                    preserved: rate
                        .preserved(role)
                        .map_or_else(Vec::new, |basket| basket_view(basket, ontology)),
                })
                .collect();
            let distinct_roles = rate
                .distinct_roles()
                .map(|(left, right)| [ontology.role(left), ontology.role(right)])
                .collect();
            Some(RateView {
                rate: ontology.rate(id),
                roles,
                distinct_roles,
            })
        })
        .collect::<Vec<_>>();
    rates.sort_by(|left, right| left.rate.key.cmp(&right.rate.key));

    let goal = goal
        .requirements()
        .iter()
        .map(|(account, required)| GoalRequirementView {
            account: ontology.account(account),
            required: basket_view(required, ontology),
        })
        .collect();

    let invariants = economy
        .invariants()
        .map(|invariant| {
            let mut terms = invariant
                .weights()
                .map(|(asset, weight)| InvariantTermView {
                    asset: ontology.asset(asset),
                    weight,
                })
                .collect::<Vec<_>>();
            terms.sort_by(|left, right| left.asset.key.cmp(&right.asset.key));
            InvariantView {
                name: invariant.name().into(),
                terms,
            }
        })
        .collect();

    ModelView {
        rates,
        goal,
        invariants,
    }
}

/// Projects a snapshot without mutating or replaying it.
pub fn derive_snapshot<AccountId, A, RateId, Role, N, O>(
    index: u64,
    economy: &Economy<AccountId, A, RateId, Role, N>,
    ontology: &O,
) -> ViewSnapshot
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    snapshot(index, economy, ontology)
}

/// Projects an exchange and its non-mutating feasibility assessment.
pub fn derive_proposal<AccountId, A, RateId, Role, N, O>(
    id: impl Into<String>,
    label: impl Into<String>,
    description: impl Into<String>,
    snapshot_index: u64,
    economy: &Economy<AccountId, A, RateId, Role, N>,
    exchange: &Exchange<RateId, Role, AccountId, N>,
    ontology: &O,
) -> ProposalView
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    ProposalView {
        id: id.into(),
        label: label.into(),
        description: description.into(),
        snapshot_index,
        exchange: exchange_view(exchange, ontology),
        assessment: assessment_view(&economy.assess(exchange), ontology),
    }
}

fn snapshot<AccountId, A, RateId, Role, N, O>(
    index: u64,
    economy: &Economy<AccountId, A, RateId, Role, N>,
    ontology: &O,
) -> ViewSnapshot
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    let mut accounts = economy
        .accounts()
        .map(|(id, account)| AccountView {
            account: ontology.account(id),
            balances: basket_view(account.balances(), ontology),
        })
        .collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.account.key.cmp(&right.account.key));

    ViewSnapshot {
        index,
        accounts,
        scene: ontology.scene(index, economy),
    }
}

fn basket_view<AccountId, A, RateId, Role, N, O>(
    basket: &Basket<A, N>,
    ontology: &O,
) -> Vec<AssetQuantityView>
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    let mut entries = basket
        .iter()
        .map(|(asset, quantity)| AssetQuantityView {
            asset: ontology.asset(asset),
            quantity: quantity.into(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.asset.key.cmp(&right.asset.key));
    entries
}

fn exchange_view<AccountId, A, RateId, Role, N, O>(
    exchange: &Exchange<RateId, Role, AccountId, N>,
    ontology: &O,
) -> ExchangeView
where
    Role: Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    ExchangeView {
        rate: ontology.rate(exchange.rate()),
        units: exchange.units().into(),
        bindings: exchange
            .bindings()
            .iter()
            .map(|(role, account)| RoleBindingView {
                role: ontology.role(role),
                account: ontology.account(account),
            })
            .collect(),
    }
}

fn delta_view<AccountId, A, RateId, Role, N, O>(
    delta: &AccountDelta<AccountId, A, N>,
    ontology: &O,
) -> AccountDeltaView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    AccountDeltaView {
        account: ontology.account(delta.account()),
        consumed: basket_view(delta.consumed(), ontology),
        produced: basket_view(delta.produced(), ontology),
        preserved: basket_view(delta.preserved(), ontology),
    }
}

fn receipt_view<AccountId, A, RateId, Role, N, O>(
    receipt: &Receipt<RateId, Role, AccountId, A, N>,
    ontology: &O,
) -> ReceiptView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    ReceiptView {
        deltas: receipt
            .deltas()
            .iter()
            .map(|delta| delta_view(delta, ontology))
            .collect(),
    }
}

fn account_assessment_view<AccountId, A, RateId, Role, N, O>(
    assessment: &AccountAssessment<AccountId, A, N>,
    ontology: &O,
) -> AccountAssessmentView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    AccountAssessmentView {
        account: ontology.account(assessment.account()),
        available: basket_view(assessment.available(), ontology),
        required: basket_view(assessment.required(), ontology),
        consumed: basket_view(assessment.consumed(), ontology),
        produced: basket_view(assessment.produced(), ontology),
        preserved: basket_view(assessment.preserved(), ontology),
    }
}

fn assessment_view<AccountId, A, RateId, Role, N, O>(
    assessment: &ExchangeAssessment<AccountId, A, RateId, Role, N>,
    ontology: &O,
) -> AssessmentView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    match assessment {
        ExchangeAssessment::Applicable {
            accounts,
            projected_deltas,
        } => AssessmentView {
            status: AssessmentStatusView::Applicable,
            accounts: accounts
                .iter()
                .map(|account| account_assessment_view(account, ontology))
                .collect(),
            projected_deltas: projected_deltas
                .iter()
                .map(|delta| delta_view(delta, ontology))
                .collect(),
            shortfalls: Vec::new(),
            issues: Vec::new(),
        },
        ExchangeAssessment::Infeasible {
            accounts,
            shortfalls,
        } => AssessmentView {
            status: AssessmentStatusView::Infeasible,
            accounts: accounts
                .iter()
                .map(|account| account_assessment_view(account, ontology))
                .collect(),
            projected_deltas: Vec::new(),
            shortfalls: shortfalls
                .iter()
                .map(|shortfall| AccountShortfallView {
                    account: ontology.account(shortfall.account()),
                    missing: basket_view(shortfall.missing(), ontology),
                })
                .collect(),
            issues: Vec::new(),
        },
        ExchangeAssessment::Invalid { issues } => AssessmentView {
            status: AssessmentStatusView::Invalid,
            accounts: Vec::new(),
            projected_deltas: Vec::new(),
            shortfalls: Vec::new(),
            issues: issues.iter().map(ToString::to_string).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{Account, EconomyBuilder, Exchange, Quantity, Rate, basket};
    use serde_json::json;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum AccountId {
        Agent,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Asset {
        Ready,
        Done,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum RateId {
        Finish,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Role {
        Actor,
    }

    struct Ontology;

    impl ViewOntology<AccountId, Asset, RateId, Role> for Ontology {
        fn account(&self, _: &AccountId) -> ViewId {
            ViewId::new("account:agent", "Agent")
        }

        fn asset(&self, asset: &Asset) -> ViewId {
            match asset {
                Asset::Ready => ViewId::new("asset:ready", "Ready"),
                Asset::Done => ViewId::new("asset:done", "Done"),
            }
        }

        fn rate(&self, _: &RateId) -> ViewId {
            ViewId::new("rate:finish", "Finish")
        }

        fn role(&self, _: &Role) -> ViewId {
            ViewId::new("role:actor", "Actor")
        }
    }

    #[test]
    fn replayed_frames_expose_exact_balances_and_receipt_deltas() {
        let economy = EconomyBuilder::new()
            .account(AccountId::Agent, Account::from(basket([(Asset::Ready, 1)])))
            .rate(
                RateId::Finish,
                Rate::new()
                    .consume(Role::Actor, basket([(Asset::Ready, 1)]))
                    .produce(Role::Actor, basket([(Asset::Done, u64::MAX)])),
            )
            .build()
            .unwrap();
        let exchange =
            Exchange::new(RateId::Finish, Quantity::new(1)).bind(Role::Actor, AccountId::Agent);
        let mut trace = Trace::new();
        trace.push(exchange);

        let document = derive_document(
            ViewDocumentMetadata {
                id: "test".into(),
                title: "Test".into(),
                description: "A test trace".into(),
                source: ViewSource {
                    key: "test".into(),
                    label: "Test".into(),
                },
            },
            &economy,
            &trace,
            &Ontology,
            Vec::new(),
        )
        .unwrap();

        assert_eq!(document.frames.len(), 1);
        assert_eq!(
            document.frames[0].after.accounts[0].balances[0].quantity.0,
            u64::MAX.to_string()
        );
        assert_eq!(
            serde_json::to_value(document.frames[0].assessment.status).unwrap(),
            json!("applicable")
        );
        assert_eq!(
            document.frames[0].assessment.projected_deltas,
            document.frames[0].receipt.deltas
        );
    }

    #[test]
    fn studio_events_are_discriminated_unions() {
        let event = StudioEvent::Progress {
            run_id: "run-1".into(),
            sequence: 2,
            completed: 3,
            total: Some(5),
            message: "replaying".into(),
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["kind"], "progress");
        assert_eq!(value["completed"], 3);

        let decoded: StudioEvent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.run_id(), "run-1");
        assert_eq!(decoded.sequence(), 2);
    }

    #[test]
    fn view_document_has_a_json_schema() {
        let schema = schemars::schema_for!(ViewDocument);
        let value = serde_json::to_value(schema).unwrap();
        assert_eq!(value["title"], "ViewDocument");
    }
}
