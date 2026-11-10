use crate::{
    Capability, DuplicateProblem, InstanceDescriptor, InstanceProfile, Problem, ProblemDescriptor,
    ProblemFamily, ProblemPath, Registry, ServiceError, StrategyDescriptor, StrategyPath, StudyKey,
    StudyProgress, Work, graph_metrics, retain_observation,
};
use axionomy::{
    Economy, Goal, Ontology, Quantity, Trace, path,
    path::{Path, Segments},
};
use axionomy_search::rollout::RolloutResult;
use axionomy_view::{
    AlgorithmPath, DocumentPath, ExchangeFrame, ItemPath, LeaderboardView, MeasurePath,
    ObjectiveView, PathOntology, PlaybackError, ProposalPath, ProposalView, QuantityFormat, Scene,
    SceneAnchorView, SceneEntityRoleView, SceneEntityView, SceneEvidenceRefView, SceneGlyphView,
    SceneMetricView, SceneToneView, SearchObservationKindView, SearchObservationView,
    SearchTelemetryView, TelemetryKindView, TelemetryPointView, TelemetrySubjectView, ViewDocument,
    ViewDocumentMetadata, ViewId, ViewOntology, ViewSource, WrittenOntology, derive_document,
    derive_document_with_frames, derive_model, derive_proposal,
};
use std::{collections::VecDeque, ops::Rem};

/// Keys written from identifiers' paths (D-044), so a scene, a metric or an
/// objective names a value the way storage, MCP and every view do.
pub(super) trait Keyed {
    /// This path followed by `identifier`'s path: `route/direct-out` for
    /// `route` and `Route::DirectOut`.
    fn then(&self, identifier: &impl Segments) -> Self;
}

impl<K> Keyed for Path<K> {
    fn then(&self, identifier: &impl Segments) -> Self {
        self.join(&path::write(identifier))
    }
}

pub(super) fn visual_entity(
    key: ItemPath,
    label: impl Into<String>,
    glyph: SceneGlyphView,
    anchor: SceneAnchorView,
    role: SceneEntityRoleView,
    tone: SceneToneView,
    status: Option<String>,
) -> SceneEntityView {
    SceneEntityView {
        id: ViewId::new(key, label),
        glyph,
        anchor,
        role,
        tone,
        status,
        account: None,
        evidence: Vec::new(),
        metrics: Vec::new(),
    }
}

pub(super) fn visual_metric(
    key: MeasurePath,
    label: impl Into<String>,
    value: impl ToString,
    unit: Option<&str>,
) -> SceneMetricView {
    SceneMetricView {
        key,
        label: label.into(),
        value: value.to_string(),
        unit: unit.map(str::to_owned),
        previous: None,
    }
}

/// A scene metric showing a quantity of `asset`, written in the asset's
/// format, so a scene and the inspectors read it in the same unit.
pub(super) fn asset_metric<A: StudioFormat>(
    key: MeasurePath,
    label: impl Into<String>,
    asset: &A,
    quantity: impl Into<Quantity>,
) -> SceneMetricView {
    let format = asset.studio_format();
    SceneMetricView {
        key,
        label: label.into(),
        value: format.number(quantity.into()),
        unit: format.unit,
        previous: None,
    }
}

/// The key every view of `O` gives `account`, so a scene names the account
/// a frame shows.
pub(super) fn account_key<O: WrittenOntology>(account: &O::Account) -> ItemPath {
    PathOntology::<O>::default().account(account).key
}

/// The key every view of `O` gives `asset`.
pub(super) fn asset_key<O: WrittenOntology>(asset: &O::Asset) -> ItemPath {
    PathOntology::<O>::default().asset(asset).key
}

/// Shows `entity` as `account`, citing the account as its evidence.
pub(super) fn link_account<O: WrittenOntology>(
    mut entity: SceneEntityView,
    account: &O::Account,
) -> SceneEntityView {
    let account = account_key::<O>(account);
    entity.account = Some(account.clone());
    entity
        .evidence
        .push(SceneEvidenceRefView::Account { account });
    entity
}

/// Shows `entity` as `account`, citing its balance of `asset` as evidence.
pub(super) fn link_balance<O: WrittenOntology>(
    mut entity: SceneEntityView,
    account: &O::Account,
    asset: &O::Asset,
) -> SceneEntityView {
    let account = account_key::<O>(account);
    entity.account = Some(account.clone());
    entity.evidence.push(SceneEvidenceRefView::Balance {
        account,
        asset: asset_key::<O>(asset),
    });
    entity
}

mod amm;
mod bridge;
mod connect_four;
mod exact_cover;
mod fronts;
mod labels;
mod lending;
mod logistics;
mod marketplace;
mod maze;
mod mission;
mod perishables;
mod rescue;
mod scheduling;
mod sokoban;
mod work_league;
mod workshop;

use fronts::{
    evaluated_front, exact_front, exact_values, frontier_trace, objective_axes, objective_value,
    sampled_policy_front,
};
use labels::{StudioFormat, StudioLabel, counted, debug_labels, words};

type StudioScene<O> = fn(u64, &Economy<O>) -> Option<Scene>;
type StudioLeaderboards<O> = fn(u64, &Economy<O>) -> Vec<LeaderboardView>;
type StudioProjections<O> = (StudioScene<O>, StudioLeaderboards<O>);

/// Studio's view of an ontology: keys from [`PathOntology`], under the
/// ontology's name; labels from [`StudioLabel`]; and the problem's scene and
/// leaderboards.
struct StudioOntology<O: Ontology> {
    fallback: PathOntology<O>,
    scene: Option<StudioScene<O>>,
    leaderboards: Option<StudioLeaderboards<O>>,
}

impl<O: Ontology> StudioOntology<O> {
    fn new(scene: Option<StudioScene<O>>) -> Self {
        Self {
            fallback: PathOntology::default(),
            scene,
            leaderboards: None,
        }
    }

    fn with_leaderboards(mut self, leaderboards: StudioLeaderboards<O>) -> Self {
        self.leaderboards = Some(leaderboards);
        self
    }

    fn relabel<T: StudioLabel>(mut id: ViewId, value: &T) -> ViewId {
        id.label = value.studio_label();
        id
    }
}

impl<O: WrittenOntology> ViewOntology<O> for StudioOntology<O>
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    fn account(&self, id: &O::Account) -> ViewId {
        Self::relabel(self.fallback.account(id), id)
    }

    fn asset(&self, id: &O::Asset) -> ViewId {
        Self::relabel(self.fallback.asset(id), id)
    }

    fn rate(&self, id: &O::Rate) -> ViewId {
        Self::relabel(self.fallback.rate(id), id)
    }

    fn role(&self, id: &O::Role) -> ViewId {
        Self::relabel(self.fallback.role(id), id)
    }

    fn format(&self, asset: &O::Asset) -> QuantityFormat {
        asset.studio_format()
    }

    fn scene(&self, index: u64, economy: &Economy<O>) -> Option<Scene> {
        self.scene.and_then(|scene| scene(index, economy))
    }

    fn leaderboards(&self, index: u64, economy: &Economy<O>) -> Vec<LeaderboardView> {
        self.leaderboards
            .map_or_else(Vec::new, |leaderboards| leaderboards(index, economy))
    }
}

/// Every canonical problem, in catalog order.
pub fn registry() -> Registry {
    canonical().expect("the canonical problems have distinct keys")
}

fn canonical() -> Result<Registry, DuplicateProblem> {
    let mut registry = Registry::new();
    registry
        .register(maze::Maze::default())?
        .register(sokoban::Sokoban::default())?
        .register(exact_cover::ExactCover::default())?
        .register(workshop::Workshop::default())?
        .register(scheduling::Scheduling::default())?
        .register(rescue::Rescue::default())?
        .register(bridge::Bridge::default())?
        .register(amm::Amm::default())?
        .register(lending::Lending::default())?
        .register(marketplace::Marketplace::default())?
        .register(logistics::Logistics::default())?
        .register(connect_four::ConnectFour::default())?
        .register(work_league::WorkLeague::default())?
        .register(mission::Mission::default())?
        .register(perishables::Perishables::default())?;
    Ok(registry)
}

struct ProblemCopy<'a> {
    key: ProblemPath,
    title: &'a str,
    summary: &'a str,
    instances: [&'a str; 3],
}

/// A strategy as the catalog lists it.
struct StrategyCopy<'a> {
    key: StrategyPath,
    label: &'a str,
    description: &'a str,
    /// The algorithm in words.
    algorithm: &'a str,
}

fn problem(
    copy: ProblemCopy<'_>,
    family: ProblemFamily,
    default_strategy: StrategyPath,
    strategies: &[StrategyCopy<'_>],
    capabilities: &[Capability],
) -> ProblemDescriptor {
    ProblemDescriptor {
        key: copy.key,
        title: copy.title.into(),
        summary: copy.summary.into(),
        family,
        default_instance: InstanceProfile::Showcase,
        instances: [
            InstanceProfile::Micro,
            InstanceProfile::Showcase,
            InstanceProfile::Stress,
        ]
        .into_iter()
        .zip(["Micro", "Showcase", "Stress"])
        .zip(copy.instances)
        .map(|((profile, label), description)| InstanceDescriptor {
            profile,
            label: label.into(),
            description: description.into(),
        })
        .collect(),
        default_strategy,
        strategies: strategies
            .iter()
            .map(|strategy| StrategyDescriptor {
                key: strategy.key.clone(),
                label: strategy.label.into(),
                description: strategy.description.into(),
                algorithm: strategy.algorithm.into(),
            })
            .collect(),
        capabilities: capabilities.to_vec(),
    }
}

/// The greatest common divisor of two counts, at least 1, so a fraction
/// reduced by it keeps a nonzero denominator.
pub(super) fn gcd<T>(mut left: T, mut right: T) -> T
where
    T: Copy + Ord + Default + Rem<Output = T> + From<u8>,
{
    while right != T::default() {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(T::from(1))
}

/// The strategy a study asks for, read as an adapter's own strategy type.
pub(super) fn strategy<S: Segments>(study: &StudyKey) -> Result<S, ServiceError> {
    path::read(&study.strategy).map_err(|_| ServiceError::UnknownStrategy {
        problem: study.problem.clone(),
        strategy: study.strategy.clone(),
    })
}

/// The document showing a strategy's outcome, which a study names by the
/// strategy's own path.
pub(crate) fn strategy_document(strategy: &StrategyPath) -> DocumentPath {
    renamed(strategy)
}

/// The same path naming another kind of thing.
fn renamed<K, J>(path: &Path<K>) -> Path<J> {
    path.as_str()
        .parse()
        .unwrap_or_else(|_| unreachable!("`{path}` is a path of every kind"))
}

pub(super) struct DocumentSpec<'a> {
    /// The problem studied, the study's own.
    pub problem: &'a ProblemPath,
    /// The strategy whose outcome the document shows, or another outcome the
    /// study keeps beside them, such as `deadlock`.
    pub document: DocumentPath,
    pub title: &'a str,
    pub description: &'a str,
    pub source_label: &'a str,
}

/// The document of a trace replayed from `initial`. Its trajectory is
/// reported to the study's observer.
pub(super) fn document<O: WrittenOntology>(
    progress: &mut StudyProgress<'_>,
    spec: DocumentSpec<'_>,
    initial: &Economy<O>,
    goal: &Goal<O>,
    trace: &Trace<O>,
    objectives: Vec<ObjectiveView>,
    scene: StudioScene<O>,
) -> Result<ViewDocument, PlaybackError>
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    let ontology = StudioOntology::<O>::new(Some(scene));
    let mut document = derive_document(spec.metadata(), initial, trace, &ontology, objectives)?;
    document.model = Some(derive_model(initial, goal, &ontology));
    progress.trajectory(&document.id, initial, trace);
    Ok(document)
}

/// The document of a trace replayed from `initial`, with leaderboards; its
/// frames stream to the study's observer as they are derived when `streamed`.
/// Its trajectory is reported to the observer.
#[allow(clippy::too_many_arguments)]
pub(super) fn document_with_leaderboards_observed<O: WrittenOntology>(
    progress: &mut StudyProgress<'_>,
    spec: DocumentSpec<'_>,
    initial: &Economy<O>,
    goal: &Goal<O>,
    trace: &Trace<O>,
    objectives: Vec<ObjectiveView>,
    projections: StudioProjections<O>,
    streamed: bool,
) -> Result<ViewDocument, PlaybackError>
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    let (scene, leaderboards) = projections;
    let ontology = StudioOntology::<O>::new(Some(scene)).with_leaderboards(leaderboards);
    let metadata = spec.metadata();
    let id = metadata.id.clone();
    let mut document = derive_document_with_frames(
        metadata,
        initial,
        trace,
        &ontology,
        objectives,
        |frame: &ExchangeFrame| {
            if streamed {
                progress.frame(&id, frame);
            }
        },
    )?;
    document.model = Some(derive_model(initial, goal, &ontology));
    progress.trajectory(&id, initial, trace);
    Ok(document)
}

impl DocumentSpec<'_> {
    fn metadata(&self) -> ViewDocumentMetadata {
        ViewDocumentMetadata {
            id: self.document.clone(),
            title: self.title.into(),
            description: self.description.into(),
            source: ViewSource {
                key: renamed(self.problem),
                label: self.source_label.into(),
            },
        }
    }
}

pub(super) struct ProposalSpec<'a> {
    pub id: ProposalPath,
    pub label: &'a str,
    pub description: &'a str,
}

/// A proposal assessed against the document's initial state.
pub(super) fn proposal<O: WrittenOntology>(
    spec: ProposalSpec<'_>,
    economy: &Economy<O>,
    exchange: &axionomy::Exchange<O>,
) -> ProposalView
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    proposal_at(spec, 0, economy, exchange)
}

/// A proposal assessed against `economy`, the document's snapshot
/// `snapshot_index`: zero is the initial state, and frame `i` leaves
/// snapshot `i + 1`.
pub(super) fn proposal_at<O: WrittenOntology>(
    spec: ProposalSpec<'_>,
    snapshot_index: u64,
    economy: &Economy<O>,
    exchange: &axionomy::Exchange<O>,
) -> ProposalView
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    let ontology = StudioOntology::<O>::new(None);
    derive_proposal(
        spec.id,
        spec.label,
        spec.description,
        snapshot_index,
        economy,
        exchange,
        &ontology,
    )
}

/// The work of the algorithm `title` names, counted.
pub(super) fn telemetry(
    title: impl Into<String>,
    exact: bool,
    values: impl IntoIterator<Item = (TelemetryKindView, u64, String)>,
) -> SearchTelemetryView {
    series(TelemetrySubjectView::Algorithm, title, exact, values)
}

pub(crate) fn series(
    subject: TelemetrySubjectView,
    title: impl Into<String>,
    exact: bool,
    values: impl IntoIterator<Item = (TelemetryKindView, u64, String)>,
) -> SearchTelemetryView {
    SearchTelemetryView {
        subject,
        title: title.into(),
        exact,
        points: values
            .into_iter()
            .enumerate()
            .map(|(sequence, (kind, value, label))| TelemetryPointView {
                sequence: sequence as u64,
                kind,
                value: value.to_string(),
                label,
            })
            .collect(),
    }
}

/// Runs a simulation from a fork of `initial` and gives the trace it left and
/// the state it ended in, or why it did not reach its goal.
pub(super) fn simulate<O: Ontology>(
    problem: &ProblemPath,
    initial: &Economy<O>,
    run: impl FnOnce(&mut Economy<O>) -> RolloutResult<O>,
) -> Result<(Trace<O>, Economy<O>), ServiceError> {
    let mut world = initial.clone();
    let result = run(&mut world);
    if !result.termination().is_terminal() {
        return Err(problem_error(
            problem,
            format!("did not reach its goal: {:?}", result.termination()),
        ));
    }
    let trace = result
        .trace()
        .cloned()
        .ok_or_else(|| problem_error(problem, "the rollout kept no trace"))?;
    Ok((trace, world))
}

pub(super) fn problem_error(problem: &ProblemPath, error: impl ToString) -> ServiceError {
    ServiceError::Problem {
        problem: problem.clone(),
        message: error.to_string(),
    }
}
