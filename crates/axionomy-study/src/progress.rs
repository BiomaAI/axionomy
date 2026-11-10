//! How a problem reports a study while it runs: its progress, the thinking
//! behind it, the frames of its documents and their trajectories, with the
//! study's control checked at each report.

use crate::{ProblemPath, Progress, StudyControl, StudyError, StudyObserver};
use axionomy::{Economy, Trace, path, paths};
use axionomy_search::GraphSearchProgress;
use axionomy_view::{
    AlgorithmPath, DocumentPath, ExchangeFrame, PhasePath, SceneMetricView,
    SearchObservationKindView, SearchObservationView, ViewId, WrittenOntology,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeSet, VecDeque},
    ops::ControlFlow,
};

/// What progress is reported about: a phase of work, the algorithm doing it
/// and the kind of checkpoint its observations are.
#[derive(Debug, Clone)]
pub struct Work {
    pub phase: PhasePath,
    pub algorithm: AlgorithmPath,
    pub kind: SearchObservationKindView,
}

impl Work {
    pub fn new(
        phase: PhasePath,
        algorithm: AlgorithmPath,
        kind: SearchObservationKindView,
    ) -> Self {
        Self {
            phase,
            algorithm,
            kind,
        }
    }

    /// Policies evaluated by sampled rollouts.
    pub fn monte_carlo() -> Self {
        Self::new(
            path!("monte-carlo"),
            path!("monte-carlo"),
            SearchObservationKindView::Rollout,
        )
    }

    /// A policy frontier estimated from sampled rollouts.
    pub fn pareto_sampling() -> Self {
        Self::new(
            path!("pareto-sampling"),
            path!("pareto-sampling"),
            SearchObservationKindView::Frontier,
        )
    }
}

/// How many observations a study keeps: the newest.
const KEPT_OBSERVATIONS: usize = 256;

/// Keeps `observation`, forgetting the oldest once a study's worth are kept.
pub fn retain_observation(
    observations: &mut VecDeque<SearchObservationView>,
    observation: SearchObservationView,
) {
    if observations.len() == KEPT_OBSERVATIONS {
        observations.pop_front();
    }
    observations.push_back(observation);
}

/// The counters of a graph search at a checkpoint.
pub fn graph_metrics(progress: GraphSearchProgress) -> Vec<SceneMetricView> {
    vec![
        metric(path!("expanded"), "States expanded", progress.expanded()),
        metric(path!("generated"), "States generated", progress.generated()),
        metric(path!("frontier"), "Frontier", progress.frontier()),
        metric(path!("visited"), "States visited", progress.visited()),
    ]
}

/// A count observed while working.
fn metric(key: axionomy_view::MeasurePath, label: &str, value: impl ToString) -> SceneMetricView {
    SceneMetricView {
        key,
        label: label.into(),
        value: value.to_string(),
        unit: None,
        previous: None,
    }
}

/// A study's reports, as its problem makes them: progress and the
/// observations it makes, frames and trajectories, each passed to the study's
/// observer. Every report checks the study's control first; once the study
/// must stop, reports are dropped and [`StudyProgress::checkpoint`] says why.
pub struct StudyProgress<'a> {
    problem: ProblemPath,
    control: &'a StudyControl,
    observer: &'a mut dyn StudyObserver,
    sequence: u64,
    error: Option<StudyError>,
    observations: VecDeque<SearchObservationView>,
    framed_documents: BTreeSet<DocumentPath>,
}

impl<'a> StudyProgress<'a> {
    pub(crate) fn new(
        problem: ProblemPath,
        control: &'a StudyControl,
        observer: &'a mut dyn StudyObserver,
    ) -> Self {
        Self {
            problem,
            control,
            observer,
            sequence: 0,
            error: None,
            observations: VecDeque::new(),
            framed_documents: BTreeSet::new(),
        }
    }

    /// Reports `completed` of `total` units of `work`, unless the study is
    /// stopping.
    pub fn emit(
        &mut self,
        work: &Work,
        completed: u64,
        total: u64,
        message: impl Into<String>,
    ) -> ControlFlow<()> {
        let metrics = vec![
            metric(path!("completed"), "Work completed", completed),
            metric(
                path!("remaining"),
                "Work remaining",
                total.saturating_sub(completed),
            ),
        ];
        self.report(work, completed, total, message.into(), Vec::new(), metrics)
    }

    /// Publish graph-search work without pretending that an implicit frontier
    /// has a knowable completion percentage.
    pub fn graph(
        &mut self,
        work: &Work,
        progress: GraphSearchProgress,
        message: impl Into<String>,
    ) -> ControlFlow<()> {
        self.graph_with_subjects(work, progress, message, std::iter::empty())
    }

    /// Publish exact graph-search counters together with the Rust-owned scene
    /// subjects expanded during this checkpoint.
    pub fn graph_with_subjects(
        &mut self,
        work: &Work,
        progress: GraphSearchProgress,
        message: impl Into<String>,
        subjects: impl IntoIterator<Item = ViewId>,
    ) -> ControlFlow<()> {
        self.report(
            work,
            progress.expanded() as u64,
            // Zero means the frontier has no honest fixed denominator. The UI
            // renders an indeterminate bar and the exact counters below.
            0,
            message.into(),
            subjects.into_iter().collect(),
            graph_metrics(progress),
        )
    }

    /// Reports progress and the observation it makes, unless the study is
    /// stopping.
    fn report(
        &mut self,
        work: &Work,
        completed: u64,
        total: u64,
        message: String,
        subjects: Vec<ViewId>,
        metrics: Vec<SceneMetricView>,
    ) -> ControlFlow<()> {
        if self.error.is_some() {
            return ControlFlow::Break(());
        }
        if let Err(error) = self.control.checkpoint() {
            self.error = Some(error);
            return ControlFlow::Break(());
        }
        self.observer.progress(Progress {
            sequence: self.sequence,
            phase: work.phase.clone(),
            completed,
            total,
            message: message.clone(),
        });
        let observation = SearchObservationView {
            sequence: self.sequence,
            phase: work.phase.clone(),
            algorithm: work.algorithm.clone(),
            kind: work.kind,
            label: message,
            completed,
            total,
            subjects,
            metrics,
        };
        self.observer.observation(observation.clone());
        retain_observation(&mut self.observations, observation);
        self.sequence += 1;
        ControlFlow::Continue(())
    }

    /// Waits while the study is paused, then says whether it must stop: it
    /// was cancelled, or a report could not be made.
    pub fn checkpoint(&mut self) -> Result<(), StudyError> {
        if self.error.is_none()
            && let Err(error) = self.control.checkpoint()
        {
            self.error = Some(error);
        }
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    /// The observations kept so far, oldest first.
    pub fn observations(&self) -> impl Iterator<Item = &SearchObservationView> {
        self.observations.iter()
    }

    /// Reports a frame of `document` as soon as replay derives it.
    pub fn frame(&mut self, document: &DocumentPath, frame: &ExchangeFrame) {
        if self.error.is_none() {
            self.framed_documents.insert(document.clone());
            self.observer.frame(document, frame.clone());
        }
    }

    pub(crate) fn has_frames(&self, document: &DocumentPath) -> bool {
        self.framed_documents.contains(document)
    }

    /// Reports the trajectory a document's frames replay, written as paths.
    /// A typed economy and trace serialize as `paths` ones (D-044); one that
    /// does not fails the study, at its next check.
    pub fn trajectory<O: WrittenOntology>(
        &mut self,
        document: &DocumentPath,
        initial: &Economy<O>,
        trace: &Trace<O>,
    ) {
        if self.error.is_some() {
            return;
        }
        match written::<_, paths::Economy>(initial)
            .and_then(|initial| Ok((initial, written::<_, paths::Trace>(trace)?)))
        {
            Ok((initial, trace)) => self.observer.trajectory(document, &initial, &trace),
            Err(error) => {
                self.error = Some(StudyError::Problem {
                    problem: self.problem.clone(),
                    message: format!("document `{document}` is not written as paths: {error}"),
                });
            }
        }
    }
}

/// `value` written down and read back as `T`.
fn written<V: Serialize, T: DeserializeOwned>(value: &V) -> Result<T, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(value)?)
}
