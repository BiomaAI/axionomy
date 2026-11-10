//! Problems, and the registry that studies them.

use crate::{
    ProblemDescriptor, ProblemPath, ServiceError, StrategyPath, Study, StudyControl, StudyKey,
    StudyObserver, StudyProgress, StudyRequest, Work, documents::series,
};
use axionomy::{
    path,
    path::{Path, Segments},
};
use axionomy_view::{
    DocumentPath, SearchObservationKindView, TelemetryKindView, TelemetrySubjectView, ViewDocument,
};
use std::{fmt, sync::Arc};
use thiserror::Error;

/// A problem a registry can study: what the catalog lists about it, and how
/// its studies are computed.
///
/// A problem computes a study's documents; the registry resolves the request
/// the study answers, checks the documents and assembles them into the study.
pub trait Problem: Send + Sync {
    /// The problem's catalog entry: its key, instances and strategies.
    fn descriptor(&self) -> &ProblemDescriptor;

    /// The documents of the study `key` names: one for each alternative the
    /// study compares, each derived by replay through the core. The document
    /// of `key.strategy` is named by the strategy's path. The registry
    /// resolved `key` against the descriptor, so its instance and strategy
    /// are ones the problem offers.
    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, ServiceError>;
}

/// A problem registered under a key the registry already holds.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("problem `{0}` is already registered")]
pub struct DuplicateProblem(pub ProblemPath);

/// The problems a front end offers, in the order its catalog lists them,
/// each under its own key.
#[derive(Clone, Default)]
pub struct Registry {
    problems: Vec<Arc<dyn Problem>>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registry")
            .field(
                "problems",
                &self
                    .problems
                    .iter()
                    .map(|problem| &problem.descriptor().key)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Registry {
    /// A registry of no problems.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `problem` after the problems already registered, refusing a key
    /// one of them has.
    pub fn register(
        &mut self,
        problem: impl Problem + 'static,
    ) -> Result<&mut Self, DuplicateProblem> {
        let key = &problem.descriptor().key;
        if self.find(key).is_some() {
            return Err(DuplicateProblem(key.clone()));
        }
        self.problems.push(Arc::new(problem));
        Ok(self)
    }

    fn find(&self, key: &ProblemPath) -> Option<&dyn Problem> {
        self.problems
            .iter()
            .map(Arc::as_ref)
            .find(|problem| &problem.descriptor().key == key)
    }

    /// Every problem's catalog entry, in the order they were registered.
    pub fn catalog(&self) -> Vec<ProblemDescriptor> {
        self.problems
            .iter()
            .map(|problem| problem.descriptor().clone())
            .collect()
    }

    /// The catalog entry of the problem `key`.
    pub fn problem(&self, key: &ProblemPath) -> Option<&ProblemDescriptor> {
        self.find(key).map(Problem::descriptor)
    }

    /// Decides a request's instance and strategy, checking the problem
    /// offers them.
    pub fn resolve(&self, request: &StudyRequest) -> Result<StudyKey, ServiceError> {
        let problem = self
            .problem(&request.problem)
            .ok_or_else(|| ServiceError::UnknownProblem(request.problem.clone()))?;
        let instance = request.instance.unwrap_or(problem.default_instance);
        if !problem
            .instances
            .iter()
            .any(|known| known.profile == instance)
        {
            return Err(ServiceError::UnknownInstance {
                problem: problem.key.clone(),
                instance,
            });
        }
        let strategy = request
            .strategy
            .clone()
            .unwrap_or_else(|| problem.default_strategy.clone());
        if !problem.strategies.iter().any(|known| known.key == strategy) {
            return Err(ServiceError::UnknownStrategy {
                problem: problem.key.clone(),
                strategy,
            });
        }
        Ok(StudyKey {
            problem: problem.key.clone(),
            instance,
            strategy,
            seed: request.seed,
            budget: request.budget,
        })
    }

    pub fn study(&self, request: StudyRequest) -> Result<Study, ServiceError> {
        self.study_with(&request, &StudyControl::default(), &mut |_| {})
    }

    /// Studies `request`, reporting to `observer` as the study runs and
    /// checking `control` at every report.
    pub fn study_with(
        &self,
        request: &StudyRequest,
        control: &StudyControl,
        observer: &mut dyn StudyObserver,
    ) -> Result<Study, ServiceError> {
        control.checkpoint()?;
        let key = self.resolve(request)?;
        let problem = self
            .find(&key.problem)
            .ok_or_else(|| ServiceError::UnknownProblem(key.problem.clone()))?;
        let descriptor = problem.descriptor();
        let mut progress = StudyProgress::new(key.problem.clone(), control, observer);
        let instance = descriptor
            .instances
            .iter()
            .find(|candidate| candidate.profile == key.instance)
            .map_or_else(
                || key.instance.to_string(),
                |instance| instance.label.clone(),
            );
        let _ = progress.emit(
            &Work::new(
                path!("prepare"),
                path!("prepare"),
                SearchObservationKindView::Phase,
            ),
            0,
            1,
            format!("Preparing the {instance} instance of {}", descriptor.title),
        );
        progress.checkpoint()?;
        let documents = problem.study(&key, &mut progress)?;
        let mut study = assemble(&key, descriptor, documents)?;
        progress.checkpoint()?;
        if let Some(document) = study.selected()
            && !progress.has_frames(&document.id)
        {
            for frame in &document.frames {
                progress.frame(&document.id, frame);
            }
        }
        let documents = Work::new(
            path!("documents"),
            path!("documents"),
            SearchObservationKindView::Document,
        );
        for (offset, document) in study.documents.iter().enumerate() {
            let _ = progress.emit(
                &documents,
                offset as u64 + 1,
                study.documents.len() as u64,
                format!("Recorded the replay “{}”", document.title),
            );
            progress.checkpoint()?;
        }
        study.assessed_proposals = study
            .documents
            .iter()
            .flat_map(|document| document.proposals.iter().cloned())
            .collect();
        let thinking = progress.observations().cloned().collect::<Vec<_>>();
        for document in &mut study.documents {
            // Problems may retain richer, strategy-specific evidence. The
            // live stream is the transport fallback, not permission to relabel
            // one algorithm's observations as every alternative in the study.
            if document.thinking.is_empty() {
                document.thinking = thinking.clone();
            }
            for frame in &mut document.frames {
                if frame.observations.is_empty() {
                    frame.observations = document.observations.clone();
                }
            }
        }
        Ok(study)
    }
}

/// The study of `key` holding `documents`, each checked: its scenes and
/// their evidence are valid, every frame explains itself, and the study's
/// strategy has a document.
fn assemble(
    key: &StudyKey,
    descriptor: &ProblemDescriptor,
    mut documents: Vec<ViewDocument>,
) -> Result<Study, ServiceError> {
    let instance = descriptor
        .instances
        .iter()
        .find(|instance| instance.profile == key.instance)
        .ok_or_else(|| ServiceError::UnknownInstance {
            problem: key.problem.clone(),
            instance: key.instance,
        })?
        .clone();
    let selected_document = strategy_document(&key.strategy);
    let alternatives = documents.len() as u64;
    for document in &mut documents {
        for snapshot in std::iter::once(&document.initial)
            .chain(document.frames.iter().map(|frame| &frame.after))
        {
            if let Some(scene) = &snapshot.scene {
                scene.validate().map_err(|error| ServiceError::Problem {
                    problem: descriptor.key.clone(),
                    message: format!("invalid scene at snapshot {}: {error}", snapshot.index),
                })?;
                scene
                    .validate_evidence(&snapshot.accounts)
                    .map_err(|error| ServiceError::Problem {
                        problem: descriptor.key.clone(),
                        message: format!(
                            "invalid scene evidence at snapshot {}: {error}",
                            snapshot.index
                        ),
                    })?;
            }
        }
        if document.frames.iter().any(|frame| frame.cues.is_empty()) {
            return Err(ServiceError::Problem {
                problem: descriptor.key.clone(),
                message: "accepted transition omitted its explanatory frame cue".into(),
            });
        }
        let accounts = document.initial.accounts.len() as u64;
        let rates = document
            .model
            .as_ref()
            .map_or(0, |model| model.rates.len() as u64);
        let transitions = document.frames.len() as u64;
        let constraints = document.proposals.len() as u64;
        document.telemetry.push(series(
            TelemetrySubjectView::Model,
            "Model size",
            true,
            [
                (TelemetryKindView::Accounts, accounts, "accounts".into()),
                (TelemetryKindView::Rates, rates, "rules".into()),
                (
                    TelemetryKindView::Transitions,
                    transitions,
                    "steps in this trace".into(),
                ),
                (
                    TelemetryKindView::Constraints,
                    constraints,
                    "rejection probes".into(),
                ),
                (
                    TelemetryKindView::Alternatives,
                    alternatives,
                    "alternatives compared".into(),
                ),
            ],
        ));
    }
    if !documents
        .iter()
        .any(|document| document.id == selected_document)
    {
        return Err(ServiceError::Problem {
            problem: descriptor.key.clone(),
            message: format!("strategy `{}` produced no document", key.strategy),
        });
    }
    Ok(Study {
        key: key.clone(),
        problem: descriptor.clone(),
        instance,
        selected_document,
        documents,
        assessed_proposals: Vec::new(),
    })
}

/// The strategy a study asks for, read as the problem's own strategy type.
pub fn strategy<S: Segments>(study: &StudyKey) -> Result<S, ServiceError> {
    path::read(&study.strategy).map_err(|_| ServiceError::UnknownStrategy {
        problem: study.problem.clone(),
        strategy: study.strategy.clone(),
    })
}

/// The document showing a strategy's outcome, which a study names by the
/// strategy's own path.
pub fn strategy_document(strategy: &StrategyPath) -> DocumentPath {
    renamed(strategy)
}

/// The same path naming another kind of thing.
pub(crate) fn renamed<K, J>(path: &Path<K>) -> Path<J> {
    path.as_str()
        .parse()
        .unwrap_or_else(|_| unreachable!("`{path}` is a path of every kind"))
}

/// The error a study of `problem` fails with: why it could not be studied.
pub fn problem_error(problem: &ProblemPath, error: impl ToString) -> ServiceError {
    ServiceError::Problem {
        problem: problem.clone(),
        message: error.to_string(),
    }
}
