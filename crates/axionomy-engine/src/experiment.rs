//! Experiments and comparison (D-050). An experiment runs strategies as arms
//! from one start toward one goal with one effort, each over the same seeds,
//! and measures every run on the same objectives; comparison ranks any runs
//! by objective vectors, dominance and the Pareto front.

use crate::{
    Engine, EngineError, ExperimentPath, GoalPath, ObjectivePath, Outcome, StrategyPath,
    definitions::{Objective, kind},
    strategy::Effort,
};
use axionomy::{Quantity, path::Path};
use axionomy_search::{
    pareto::{Dominance, ObjectiveDirection},
    sampling::sample_seed,
};
use axionomy_store::{Durability, Group, GroupId, Member, Point, RefPath, StoreError};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ops::ControlFlow};

/// One arm: a strategy, from the experiment's start or from a start of its
/// own, such as the start with a trace already played or an actor's
/// holdings withdrawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arm {
    pub strategy: StrategyPath,
    pub start: Option<RefPath>,
}

/// Strategies run under the same conditions and measured alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Experiment {
    /// The ref naming the point the arms start from.
    pub start: RefPath,
    /// The goal every arm works toward, so the arms answer the same
    /// question (D-052).
    pub goal: GoalPath,
    pub arms: Vec<Arm>,
    pub objectives: Vec<ObjectivePath>,
    /// What every arm is given to spend: one effort for every arm, so arms
    /// are compared on the same terms; its seed seeds the trials.
    pub effort: Effort,
    /// How many trials each arm runs, trial `i` seeded from the effort's
    /// seed and `i`; more than one makes a Monte Carlo estimate of each
    /// arm.
    pub trials: u64,
}

/// How a run ended, as an experiment records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ended {
    Reached,
    Stopped,
    Horizon,
    Refused { turn: u64 },
    Exhausted,
    Spent,
    Interrupted,
}

impl From<&Outcome> for Ended {
    fn from(outcome: &Outcome) -> Self {
        match outcome {
            Outcome::Reached => Self::Reached,
            Outcome::Stopped => Self::Stopped,
            Outcome::Horizon => Self::Horizon,
            Outcome::Refused { turn, .. } => Self::Refused { turn: *turn },
            Outcome::Exhausted => Self::Exhausted,
            Outcome::Spent => Self::Spent,
            Outcome::Interrupted => Self::Interrupted,
        }
    }
}

/// One trial of one arm: one seeded run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    pub seed: u64,
    pub end: Point,
    pub ended: Ended,
    /// The experiment's objectives at `end`, in its order.
    pub values: Vec<Quantity>,
}

/// What one arm did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmResult {
    pub arm: Arm,
    pub start: Point,
    pub trials: Vec<Trial>,
}

impl ArmResult {
    /// Each objective summed over the arm's trials: arms run the same
    /// number of trials, so sums compare as means do, exactly.
    pub fn totals(&self) -> Vec<Quantity> {
        let mut totals = Vec::new();
        for trial in &self.trials {
            totals.resize(trial.values.len(), Quantity::ZERO);
            for (total, value) in totals.iter_mut().zip(&trial.values) {
                *total = total.checked_add(*value).unwrap_or(*total);
            }
        }
        totals
    }
}

/// An experiment's results: every arm's trials, and the store group its
/// runs were recorded in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Results {
    pub group: GroupId,
    pub arms: Vec<ArmResult>,
}

/// Points compared on objectives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// Every point with its objective values, in the order given.
    pub values: Vec<(Point, Vec<Quantity>)>,
    /// The points no other point dominates, in the order given.
    pub front: Vec<Point>,
}

impl Engine {
    /// Keeps `experiment` at `path`, refusing one that names a goal, a
    /// strategy or an objective the store does not keep.
    pub fn keep_experiment(
        &self,
        path: &ExperimentPath,
        experiment: &Experiment,
    ) -> Result<(), EngineError> {
        self.goal(&experiment.goal)?;
        for arm in &experiment.arms {
            self.strategy(&arm.strategy)?;
        }
        for objective in &experiment.objectives {
            self.objective(objective)?;
        }
        self.define(path, experiment)
    }

    pub fn experiment(&self, path: &ExperimentPath) -> Result<Experiment, EngineError> {
        self.defined(path)
    }

    /// The experiments the store keeps, in path order.
    pub fn experiments(&self) -> Result<Vec<ExperimentPath>, EngineError> {
        let prefix = crate::definitions::prefix::<kind::Experiment>();
        self.store
            .read()?
            .definitions(&prefix, usize::MAX)?
            .into_iter()
            .map(|(name, _)| {
                name.as_str()
                    .strip_prefix(&prefix)
                    .and_then(|path| path.parse().ok())
                    .ok_or(EngineError::Store(StoreError::Malformed(
                        "an experiment's path",
                    )))
            })
            .collect()
    }

    /// Runs every arm of the experiment at `path` once per trial, toward the
    /// experiment's goal with its effort, records the runs as a store group
    /// of kind `experiment`, and keeps and returns the results. `watch` sees every
    /// search's progress and may stop it, which stops the experiment: it
    /// keeps no results, and the results it kept before stay, so an
    /// experiment stopped is never mistaken for one whose searches ran out.
    pub fn run_experiment(
        &self,
        path: &ExperimentPath,
        watch: &mut impl FnMut(&axionomy_search::GraphSearchProgress) -> ControlFlow<()>,
    ) -> Result<Results, EngineError> {
        let experiment = self.experiment(path)?;
        let start = self.referred(&experiment.start)?;
        let group = self.store.mint_group();
        let record = Group {
            kind: "experiment".parse().expect("a path"),
            source: Some(start),
            tags: BTreeMap::new(),
        };
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.create_group(group, record)
            })
            .wait()?;
        let mut arms = Vec::with_capacity(experiment.arms.len());
        let mut member = 0;
        for arm in &experiment.arms {
            let from = match &arm.start {
                Some(start) => self.referred(start)?,
                None => start,
            };
            let strategy = self.strategy(&arm.strategy)?;
            let mut trials = Vec::new();
            for trial in 0..experiment.trials {
                let seed = sample_seed(
                    experiment.effort.seed,
                    usize::try_from(trial).unwrap_or(usize::MAX),
                );
                let effort = Effort {
                    seed,
                    ..experiment.effort
                };
                let ran = self.run_strategy(from, &strategy, &experiment.goal, effort, watch)?;
                if let Outcome::Interrupted = ran.outcome {
                    return Err(EngineError::Interrupted);
                }
                let values = experiment
                    .objectives
                    .iter()
                    .map(|objective| self.measure(ran.end, objective))
                    .collect::<Result<Vec<_>, _>>()?;
                let record = Member {
                    run: Some(ran.end.run),
                    seed: Some(seed),
                    metrics: experiment
                        .objectives
                        .iter()
                        .zip(&values)
                        .map(|(objective, value)| {
                            let name = format!("{}/{objective}", arm.strategy)
                                .parse()
                                .expect("two paths make a path");
                            (name, approximately(*value))
                        })
                        .collect(),
                };
                self.store
                    .transact(Durability::Eventual, move |txn| {
                        txn.add_member(group, member, &record)
                    })
                    .wait()?;
                member += 1;
                trials.push(Trial {
                    seed,
                    end: ran.end,
                    ended: Ended::from(&ran.outcome),
                    values,
                });
            }
            arms.push(ArmResult {
                arm: arm.clone(),
                start: from,
                trials,
            });
        }
        let results = Results { group, arms };
        self.define(&results_path(path), &results)?;
        Ok(results)
    }

    /// The results the experiment at `path` last recorded.
    pub fn results(&self, path: &ExperimentPath) -> Result<Results, EngineError> {
        self.defined(&results_path(path))
    }

    /// `points` compared on `objectives`: each point's values, and the
    /// points no other dominates.
    pub fn compare(
        &self,
        points: &[Point],
        objectives: &[ObjectivePath],
    ) -> Result<Comparison, EngineError> {
        let directions = objectives
            .iter()
            .map(|objective| {
                self.objective(objective)
                    .map(|Objective { direction, .. }| direction)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let values = points
            .iter()
            .map(|point| {
                objectives
                    .iter()
                    .map(|objective| self.measure(*point, objective))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|values| (*point, values))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let front = values
            .iter()
            .filter(|(_, mine)| {
                !values
                    .iter()
                    .any(|(_, other)| dominance(other, mine, &directions) == Dominance::Dominates)
            })
            .map(|(point, _)| *point)
            .collect();
        Ok(Comparison { values, front })
    }

    /// The point whose values are best in lexicographic order of
    /// `objectives`, each in its direction; the first on a tie.
    pub fn pick(
        &self,
        points: &[Point],
        objectives: &[ObjectivePath],
    ) -> Result<Option<Point>, EngineError> {
        let directions = objectives
            .iter()
            .map(|objective| {
                self.objective(objective)
                    .map(|Objective { direction, .. }| direction)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let compared = self.compare(points, objectives)?;
        let mut best: Option<&(Point, Vec<Quantity>)> = None;
        for entry in &compared.values {
            let better = best.is_none_or(|(_, kept)| {
                for ((value, kept), direction) in entry.1.iter().zip(kept).zip(&directions) {
                    let order = match direction {
                        ObjectiveDirection::Minimize => kept.cmp(value),
                        ObjectiveDirection::Maximize => value.cmp(kept),
                    };
                    if order.is_ne() {
                        return order.is_gt();
                    }
                }
                false
            });
            if better {
                best = Some(entry);
            }
        }
        Ok(best.map(|(point, _)| *point))
    }
}

/// Whether `left` dominates `right`, in each objective's direction.
fn dominance(
    left: &[Quantity],
    right: &[Quantity],
    directions: &[ObjectiveDirection],
) -> Dominance {
    let (mut better, mut worse) = (false, false);
    for ((left, right), direction) in left.iter().zip(right).zip(directions) {
        let order = match direction {
            ObjectiveDirection::Minimize => right.cmp(left),
            ObjectiveDirection::Maximize => left.cmp(right),
        };
        better |= order.is_gt();
        worse |= order.is_lt();
    }
    match (better, worse) {
        (true, false) => Dominance::Dominates,
        (false, true) => Dominance::Dominated,
        (false, false) => Dominance::Equal,
        (true, true) => Dominance::Incomparable,
    }
}

/// A quantity as a metric reads it: exactly below 2^53, close enough above.
fn approximately(value: Quantity) -> f64 {
    u64::try_from(value).map_or(f64::MAX, |value| value as f64)
}

fn results_path(experiment: &ExperimentPath) -> Path<kind::Results> {
    experiment.as_str().parse().expect("a path is a path")
}
