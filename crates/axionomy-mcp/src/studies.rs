//! Studies as economies. A study's documents each replay a trajectory, which
//! the service reports written as paths; the server records each one as an
//! economy for its initial state and a run of the exchanges its trace applies,
//! so a client can step into any state of a study and try other exchanges
//! from there. The server keeps a study's key and its documents' handles, and
//! computes its views again from the key when they are not in memory.

use crate::store::{State, checkpoint};
use axionomy::paths;
use axionomy_service::{
    Registry, ServiceError, ServiceProgress, Study, StudyControl, StudyKey, StudyObserver,
    StudyRequest,
};
use axionomy_tree::Tree;
use axionomy_view::DocumentPath;
use std::collections::BTreeMap;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Error)]
pub(crate) enum StudyError {
    #[error(transparent)]
    Service(#[from] ServiceError),
    #[error("document `{0}` reported no trajectory")]
    Untraced(DocumentPath),
    #[error(
        "document `{document}` shows {frames} frames, but its trajectory has {exchanges} exchanges"
    )]
    Mismatched {
        document: DocumentPath,
        frames: usize,
        exchanges: usize,
    },
    #[error("exchange {index} of document `{document}` does not replay as paths: {error}")]
    Replay {
        document: DocumentPath,
        index: usize,
        error: Box<paths::ApplyError>,
    },
}

/// A study document's trajectory written as paths and replayed: the state it
/// starts from, each exchange's receipt with the state its run stores after
/// it, if any, and the state it ends in.
pub(crate) struct Trajectory {
    pub document: DocumentPath,
    pub initial: State,
    pub steps: Vec<(paths::Receipt, Option<Tree>)>,
    pub last: State,
}

impl Trajectory {
    fn replay(
        document: DocumentPath,
        initial: paths::Economy,
        trace: paths::Trace,
    ) -> Result<Self, StudyError> {
        let initial = State::new(initial);
        let mut last = initial.clone();
        let mut steps = Vec::with_capacity(trace.exchanges().len());
        for (index, exchange) in trace.into_exchanges().into_iter().enumerate() {
            let receipt = last.apply(exchange).map_err(|error| StudyError::Replay {
                document: document.clone(),
                index,
                error: Box::new(error),
            })?;
            // The initial state is step zero of its own run.
            let step = index as u64 + 1;
            steps.push((receipt, checkpoint(step).then(|| last.tree.clone())));
        }
        Ok(Self {
            document,
            initial,
            steps,
            last,
        })
    }

    /// The exchanges the trajectory applies, in order.
    pub fn exchanges(&self) -> Vec<&paths::Exchange> {
        self.steps
            .iter()
            .map(|(receipt, _)| receipt.exchange())
            .collect()
    }
}

/// What a study reports while it runs: its progress, for whoever watches it,
/// and each document's trajectory.
struct Observer {
    progress: Option<UnboundedSender<ServiceProgress>>,
    trajectories: BTreeMap<DocumentPath, (paths::Economy, paths::Trace)>,
}

impl StudyObserver for Observer {
    fn progress(&mut self, progress: ServiceProgress) {
        if let Some(sender) = &self.progress {
            let _ = sender.send(progress);
        }
    }

    fn trajectory(
        &mut self,
        document: &DocumentPath,
        initial: &paths::Economy,
        trace: &paths::Trace,
    ) {
        self.trajectories
            .insert(document.clone(), (initial.clone(), trace.clone()));
    }
}

/// Computes a study of a problem in `registry` and its documents'
/// trajectories, in the order of its documents, sending progress to
/// `progress` while it runs.
pub(crate) fn compute(
    registry: &Registry,
    request: &StudyRequest,
    control: &StudyControl,
    progress: Option<UnboundedSender<ServiceProgress>>,
) -> Result<(Study, Vec<Trajectory>), StudyError> {
    let mut observer = Observer {
        progress,
        trajectories: BTreeMap::new(),
    };
    let study = registry.study_with(request, control, &mut observer)?;
    let trajectories = study
        .documents
        .iter()
        .map(|document| {
            let (initial, trace) = observer
                .trajectories
                .remove(&document.id)
                .ok_or_else(|| StudyError::Untraced(document.id.clone()))?;
            if trace.exchanges().len() != document.frames.len() {
                return Err(StudyError::Mismatched {
                    document: document.id.clone(),
                    frames: document.frames.len(),
                    exchanges: trace.exchanges().len(),
                });
            }
            Trajectory::replay(document.id.clone(), initial, trace)
        })
        .collect::<Result<_, _>>()?;
    Ok((study, trajectories))
}

/// The request that computes the study `key` names.
pub(crate) fn request(key: &StudyKey) -> StudyRequest {
    StudyRequest {
        problem: key.problem.clone(),
        instance: Some(key.instance),
        strategy: Some(key.strategy.clone()),
        seed: key.seed,
        budget: key.budget,
    }
}
