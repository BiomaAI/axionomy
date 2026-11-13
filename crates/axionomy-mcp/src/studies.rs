//! Studies as economies. A study's documents each replay a trajectory, which
//! the service reports written as paths; the server records each one as an
//! economy for its initial state and a run of the exchanges its trace applies,
//! so a client can step into any state of a study and try other exchanges
//! from there. The server keeps a study's key and its documents' handles, and
//! computes its views again from the key when they are not in memory.

use crate::store::State;
use axionomy::paths;
use axionomy_study::{
    Progress, Registry, Study, StudyControl, StudyError, StudyKey, StudyObserver, StudyRequest,
};
use axionomy_view::DocumentPath;
use std::collections::BTreeMap;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Error)]
pub(crate) enum ComputeError {
    #[error(transparent)]
    Study(#[from] StudyError),
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
}

/// A study document's trajectory written as paths: the state it starts from
/// and the exchanges its trace applies, which the engine replays when it
/// records them.
pub(crate) struct Trajectory {
    pub document: DocumentPath,
    pub initial: State,
    pub exchanges: Vec<paths::Exchange>,
}

/// What a study reports while it runs: its progress, for whoever watches it,
/// and each document's trajectory.
struct Observer {
    progress: Option<UnboundedSender<Progress>>,
    trajectories: BTreeMap<DocumentPath, (paths::Economy, paths::Trace)>,
}

impl StudyObserver for Observer {
    fn progress(&mut self, progress: Progress) {
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
    progress: Option<UnboundedSender<Progress>>,
) -> Result<(Study, Vec<Trajectory>), ComputeError> {
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
                .ok_or_else(|| ComputeError::Untraced(document.id.clone()))?;
            if trace.exchanges().len() != document.frames.len() {
                return Err(ComputeError::Mismatched {
                    document: document.id.clone(),
                    frames: document.frames.len(),
                    exchanges: trace.exchanges().len(),
                });
            }
            Ok(Trajectory {
                document: document.id.clone(),
                initial: State::new(initial),
                exchanges: trace.into_exchanges(),
            })
        })
        .collect::<Result<_, _>>()?;
    Ok((study, trajectories))
}

/// The request that computes the study `key` names.
pub(crate) fn request(key: &StudyKey) -> StudyRequest {
    StudyRequest {
        problem: key.problem.clone(),
        instance: Some(key.instance.clone()),
        strategy: Some(key.strategy.clone()),
        seed: key.seed,
        budget: key.budget,
    }
}
