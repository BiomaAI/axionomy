//! Experiments and comparison (D-050): an experiment's trials, every arm's
//! over the same seeds, and any points compared on objectives.

use crate::{ArmView, Natural, ObjectiveView, PointId, Refusal, packages::objectives};
use axionomy::Quantity;
use axionomy_engine::{Ended, Engine, EngineError, ExperimentPath, ObjectivePath, Results};
use axionomy_search::GraphSearchProgress;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;

/// What an experiment's trials did, every arm's, measured on the same
/// objectives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResultsView {
    pub experiment: ExperimentPath,
    /// The objectives every trial is measured on, in the experiment's order.
    pub objectives: Vec<ObjectiveView>,
    pub arms: Vec<ArmResultView>,
}

/// What one arm did: its strategy from its start, over the trials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArmResultView {
    pub arm: ArmView,
    /// The point every trial of the arm starts from.
    pub start: PointId,
    pub trials: Vec<TrialView>,
}

/// One trial: one seeded run of an arm's strategy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrialView {
    /// Drawn from the experiment's seed and the trial's position, so every
    /// arm meets the same chances.
    pub seed: Natural,
    /// Where the trial ended: the arm's start when it applied nothing.
    pub end: PointId,
    pub ended: EndedView,
    /// The experiment's objectives at `end`, exactly, in its order.
    pub values: Vec<Quantity>,
}

/// How a trial's run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EndedView {
    /// The experiment's goal holds where the run ends.
    Reached,
    /// The policy proposed nothing, or a plan ended short of the goal.
    Stopped,
    /// The policy or the game played every turn the experiment gives.
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

impl From<Ended> for EndedView {
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

/// How far a running experiment is: the trial it plays, by its arm in the
/// experiment's order and that arm's trial, both from 0, and how far that
/// trial's run is: its search's progress, or its turns counted as a search
/// counts expansions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ExperimentProgress {
    pub arm: usize,
    pub trial: u64,
    pub progress: GraphSearchProgress,
}

impl ExperimentProgress {
    /// Whether this is the progress of another trial than `last` was.
    pub fn another_trial(&self, last: &Self) -> bool {
        (self.arm, self.trial) != (last.arm, last.trial)
    }
}

/// What an experiment reports while it runs, then its results or why it
/// has none.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExperimentEvent {
    Progress {
        progress: ExperimentProgress,
    },
    /// Every trial ran; the store keeps these results.
    Results {
        results: ResultsView,
    },
    /// The experiment could not run, as the status says.
    Failed {
        status: u16,
        error: String,
    },
}

impl From<Result<ResultsView, Refusal>> for ExperimentEvent {
    fn from(results: Result<ResultsView, Refusal>) -> Self {
        match results {
            Ok(results) => Self::Results { results },
            Err(refusal) => Self::Failed {
                status: refusal.status(),
                error: refusal.message().to_owned(),
            },
        }
    }
}

/// Points to compare on objectives, each in its direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareRequest {
    pub points: Vec<PointId>,
    /// The objectives, in the order a pick prefers them.
    pub objectives: Vec<ObjectivePath>,
}

/// Points compared: each with its values, the points no other dominates,
/// and the best by the objectives in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComparisonView {
    pub objectives: Vec<ObjectiveView>,
    /// Every point with its objective values, in the order given.
    pub values: Vec<PointValues>,
    /// The points no other point dominates, in the order given.
    pub front: Vec<PointId>,
    /// The point best by the objectives in order, the first on a tie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<PointId>,
}

/// A point and its objective values, exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PointValues {
    pub point: PointId,
    pub values: Vec<Quantity>,
}

/// Checks that the experiment at `path` exists, so a transport can refuse at
/// once before it runs anything.
pub fn check_experiment(engine: &Engine, path: &ExperimentPath) -> Result<(), Refusal> {
    let experiment = engine.experiment(path)?;
    engine.referred(&experiment.start)?;
    Ok(())
}

/// Runs the experiment at `path`, keeping its results; `watch` sees every
/// trial's progress and may stop it, which stops the experiment.
pub fn run_experiment(
    engine: &Engine,
    path: ExperimentPath,
    watch: &mut impl FnMut(&ExperimentProgress) -> ControlFlow<()>,
) -> Result<ResultsView, Refusal> {
    let results = engine.run_experiment(&path, &mut |playing, progress| {
        watch(&ExperimentProgress {
            arm: playing.arm,
            trial: playing.trial,
            progress: *progress,
        })
    })?;
    results_view(engine, path, results)
}

/// The results the experiment at `path` last recorded.
pub fn results(engine: &Engine, path: ExperimentPath) -> Result<ResultsView, Refusal> {
    engine.experiment(&path)?;
    match engine.results(&path) {
        Ok(results) => results_view(engine, path, results),
        Err(EngineError::Undefined(_)) => Err(Refusal::NotFound(format!(
            "experiment `{path}` has recorded no results; run it first"
        ))),
        Err(error) => Err(error.into()),
    }
}

/// `request.points` compared on `request.objectives`.
pub fn compare(engine: &Engine, request: CompareRequest) -> Result<ComparisonView, Refusal> {
    let points: Vec<_> = request.points.iter().map(|point| point.point()).collect();
    let compared = engine.compare(&points, &request.objectives)?;
    let pick = engine.pick(&points, &request.objectives)?;
    Ok(ComparisonView {
        objectives: objectives(engine, &request.objectives)?,
        values: compared
            .values
            .into_iter()
            .map(|(point, values)| PointValues {
                point: point.into(),
                values,
            })
            .collect(),
        front: compared.front.into_iter().map(PointId::from).collect(),
        pick: pick.map(PointId::from),
    })
}

fn results_view(
    engine: &Engine,
    path: ExperimentPath,
    results: Results,
) -> Result<ResultsView, Refusal> {
    let experiment = engine.experiment(&path)?;
    Ok(ResultsView {
        objectives: objectives(engine, &experiment.objectives)?,
        arms: results
            .arms
            .into_iter()
            .map(|arm| ArmResultView {
                arm: ArmView {
                    strategy: arm.arm.strategy,
                    start: arm.arm.start,
                },
                start: arm.start.into(),
                trials: arm
                    .trials
                    .into_iter()
                    .map(|trial| TrialView {
                        seed: trial.seed.into(),
                        end: trial.end.into(),
                        ended: trial.ended.into(),
                        values: trial.values,
                    })
                    .collect(),
            })
            .collect(),
        experiment: path,
    })
}
