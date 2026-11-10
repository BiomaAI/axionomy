//! The documents a problem's study holds, derived by replaying traces
//! through the core: the document helpers every problem uses, and the
//! traces it can find by simulation or on a Pareto front.

use crate::{ProblemPath, StudyError, StudyProgress, problem::renamed, problem_error};
use axionomy::{Economy, Goal, Ontology, Trace};
use axionomy_search::{
    pareto::{ParetoEntry, ParetoSearchResult},
    rollout::RolloutResult,
};
use axionomy_view::{
    DocumentPath, ExchangeFrame, Format, Label, LabeledOntology, LeaderboardsFn,
    ObjectiveDirectionView, ObjectiveView, PlaybackError, ProposalPath, ProposalView, SceneFn,
    SearchTelemetryView, TelemetryKindView, TelemetryPointView, TelemetrySubjectView, ViewDocument,
    ViewDocumentMetadata, ViewSource, WrittenOntology, derive_document,
    derive_document_with_frames, derive_model, derive_proposal, objective_value,
};
/// What a document is: the problem studied, the outcome it shows, and the
/// words it is shown with.
pub struct DocumentSpec<'a> {
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
pub fn document<O: WrittenOntology>(
    progress: &mut StudyProgress<'_>,
    spec: DocumentSpec<'_>,
    initial: &Economy<O>,
    goal: &Goal<O>,
    trace: &Trace<O>,
    objectives: Vec<ObjectiveView>,
    scene: SceneFn<O>,
) -> Result<ViewDocument, PlaybackError>
where
    O::Account: Label,
    O::Asset: Label + Format,
    O::Rate: Label,
    O::Role: Label,
{
    let ontology = LabeledOntology::<O>::new(Some(scene));
    let mut document = derive_document(spec.metadata(), initial, trace, &ontology, objectives)?;
    document.model = Some(derive_model(initial, goal, &ontology));
    progress.trajectory(&document.id, initial, trace);
    Ok(document)
}

/// The document of a trace replayed from `initial`, with leaderboards; its
/// frames stream to the study's observer as they are derived when `streamed`.
/// Its trajectory is reported to the observer.
#[allow(clippy::too_many_arguments)]
pub fn document_with_leaderboards_observed<O: WrittenOntology>(
    progress: &mut StudyProgress<'_>,
    spec: DocumentSpec<'_>,
    initial: &Economy<O>,
    goal: &Goal<O>,
    trace: &Trace<O>,
    objectives: Vec<ObjectiveView>,
    projections: (SceneFn<O>, LeaderboardsFn<O>),
    streamed: bool,
) -> Result<ViewDocument, PlaybackError>
where
    O::Account: Label,
    O::Asset: Label + Format,
    O::Rate: Label,
    O::Role: Label,
{
    let (scene, leaderboards) = projections;
    let ontology = LabeledOntology::<O>::new(Some(scene)).with_leaderboards(leaderboards);
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

/// A proposal to assess: its path and the words it is shown with.
pub struct ProposalSpec<'a> {
    pub id: ProposalPath,
    pub label: &'a str,
    pub description: &'a str,
}

/// A proposal assessed against the document's initial state.
pub fn proposal<O: WrittenOntology>(
    spec: ProposalSpec<'_>,
    economy: &Economy<O>,
    exchange: &axionomy::Exchange<O>,
) -> ProposalView
where
    O::Account: Label,
    O::Asset: Label + Format,
    O::Rate: Label,
    O::Role: Label,
{
    proposal_at(spec, 0, economy, exchange)
}

/// A proposal assessed against `economy`, the document's snapshot
/// `snapshot_index`: zero is the initial state, and frame `i` leaves
/// snapshot `i + 1`.
pub fn proposal_at<O: WrittenOntology>(
    spec: ProposalSpec<'_>,
    snapshot_index: u64,
    economy: &Economy<O>,
    exchange: &axionomy::Exchange<O>,
) -> ProposalView
where
    O::Account: Label,
    O::Asset: Label + Format,
    O::Rate: Label,
    O::Role: Label,
{
    let ontology = LabeledOntology::<O>::new(None);
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
pub fn telemetry(
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
pub fn simulate<O: Ontology>(
    problem: &ProblemPath,
    initial: &Economy<O>,
    run: impl FnOnce(&mut Economy<O>) -> RolloutResult<O>,
) -> Result<(Trace<O>, Economy<O>), StudyError> {
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

/// The trace an exact front keeps for the best value of `first`, ties broken
/// by the best value of `second`, each best as its direction says.
pub fn frontier_trace<O: Ontology, K: Eq>(
    result: &ParetoSearchResult<O, K, u64>,
    first: (K, ObjectiveDirectionView),
    second: (K, ObjectiveDirectionView),
    problem: &ProblemPath,
) -> Result<Trace<O>, StudyError> {
    // Smaller ranks first: a maximized value counts down from the largest.
    let rank = |entry: &ParetoEntry<Trace<O>, K, u64>,
                (key, direction): &(K, ObjectiveDirectionView)| {
        let value = objective_value(entry, key);
        match direction {
            ObjectiveDirectionView::Minimize => value,
            ObjectiveDirectionView::Maximize => u64::MAX - value,
        }
    };
    result
        .front()
        .entries()
        .iter()
        .min_by_key(|entry| (rank(entry, &first), rank(entry, &second)))
        .map(|entry| entry.payload().clone())
        .ok_or_else(|| problem_error(problem, "empty Pareto frontier"))
}
