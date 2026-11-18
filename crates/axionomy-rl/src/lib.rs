#![doc = include_str!("../README.md")]

use axionomy::{AccountPath, AssetPath, Quantity, RatePath, paths};
use axionomy_engine::{
    CandidateSet, CandidatesPath, Engine, EngineError, ExperimentPath, GoalPath, Objective,
    ObjectivePath, instances, offered,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Point;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use thiserror::Error;

/// Why a task could not be made, or an episode could not take a step.
#[derive(Debug, Clone, Error)]
pub enum RlError {
    #[error("the episode has ended; reset it before taking another step")]
    EpisodeEnded,
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// A script proposes the candidate set's exchanges, so they have no list
    /// fixed before an episode starts.
    #[error("candidate set `{0}` is proposed by a script, so its exchanges have no fixed list")]
    Scripted(CandidatesPath),
    /// An instance whose rate leaves a role free: the exchange that applies
    /// it depends on who holds what, so it has no one place in a fixed list.
    #[error("rate instance `{0}` leaves a role free, so no one exchange applies it")]
    FreeRole(RatePath),
    /// The candidate set offered an exchange the task does not list.
    #[error("candidate set `{set}` offered an exchange of `{rate}` the task does not list")]
    Unlisted { set: CandidatesPath, rate: RatePath },
    /// An action past the end of the task's exchanges.
    #[error("action {action} is not one of the task's {actions} exchanges")]
    NoSuchAction { action: usize, actions: usize },
    /// An action the candidate set does not offer where the episode is.
    #[error("action {action}, an exchange of `{rate}`, is not offered here")]
    NotOffered { action: usize, rate: RatePath },
}

/// What a learner is rewarded for at each step.
#[derive(Debug, Clone, PartialEq)]
pub struct Reward {
    /// How much a change in each objective weighs: a step that moves an
    /// objective one unit in its direction earns its weight, and one that
    /// moves it one unit the other way is charged as much.
    pub weights: Vec<(ObjectivePath, f64)>,
    /// What the step that reaches the goal earns besides.
    pub reached: f64,
}

/// What a learner sees at a point.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    /// Each observed balance, in [`Task::observed`]'s order, as the nearest
    /// `f32`.
    pub balances: Vec<f32>,
    /// Whether the candidate set offers each of [`Task::actions`] here.
    pub offered: Vec<bool>,
}

/// What one step did.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The point the step's exchange led to.
    pub point: Point,
    /// What the step earned.
    pub reward: f64,
    /// Whether the goal holds at the new point.
    pub reached: bool,
    /// Whether the candidate set offers nothing at the new point, the goal
    /// not holding, so the episode cannot go on.
    pub stuck: bool,
}

impl Step {
    /// Whether the episode ended with this step.
    pub fn ended(&self) -> bool {
        self.reached || self.stuck
    }
}

/// An experiment as a learner meets it: where every episode starts, the goal
/// that ends one, the exchanges a learner chooses among, the balances it
/// observes and what it is rewarded for.
#[derive(Clone)]
pub struct Task {
    engine: Engine,
    start: Point,
    goal: GoalPath,
    candidates: CandidatesPath,
    actions: Vec<paths::Exchange>,
    /// Each listed exchange's place in `actions`, by its rate.
    by_rate: HashMap<RatePath, Vec<usize>>,
    observed: Vec<(AccountPath, AssetPath)>,
    /// Each weighed objective with its weight, negated where the objective
    /// is minimized, so a step earns the weight times the objective's change.
    rewards: Vec<(ObjectivePath, f64)>,
    reached: f64,
    /// The objectives and the observation at the start, the same for every
    /// episode.
    measured: Vec<Quantity>,
    observation: Observation,
}

impl Task {
    /// The task of `experiment`: its start, goal and engine, the exchanges
    /// of `candidates` as its actions, rewarded as `reward` says.
    pub fn new(
        engine: &Engine,
        experiment: &ExperimentPath,
        candidates: &CandidatesPath,
        reward: Reward,
    ) -> Result<Self, RlError> {
        let experiment = engine.experiment(experiment)?;
        let start = engine.referred(&experiment.start)?;
        let reached = engine.reach(start)?;
        let economy = &reached.state.economy;
        let actions = listed(economy, candidates, engine.candidate_set(candidates)?)?;
        let mut by_rate: HashMap<RatePath, Vec<usize>> = HashMap::new();
        for (index, exchange) in actions.iter().enumerate() {
            by_rate
                .entry(exchange.rate().clone())
                .or_default()
                .push(index);
        }
        let rewards = reward
            .weights
            .into_iter()
            .map(|(objective, weight)| {
                let Objective { direction, .. } = engine.objective(&objective)?;
                let weight = match direction {
                    ObjectiveDirection::Minimize => -weight,
                    ObjectiveDirection::Maximize => weight,
                };
                Ok((objective, weight))
            })
            .collect::<Result<_, RlError>>()?;
        let mut task = Self {
            engine: engine.clone(),
            start,
            goal: experiment.goal,
            candidates: candidates.clone(),
            observed: observed(economy, &actions),
            actions,
            by_rate,
            rewards,
            reached: reward.reached,
            measured: Vec::new(),
            observation: Observation {
                balances: Vec::new(),
                offered: Vec::new(),
            },
        };
        task.measured = task.measure(start)?;
        task.observation = task.observe(start)?;
        Ok(task)
    }

    /// The engine the task's episodes apply their exchanges with.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The point every episode starts from: the experiment's start.
    pub fn start(&self) -> Point {
        self.start
    }

    /// The goal that ends an episode: the experiment's.
    pub fn goal(&self) -> &GoalPath {
        &self.goal
    }

    /// The exchanges a learner chooses among: action `i` is the `i`th.
    pub fn actions(&self) -> &[paths::Exchange] {
        &self.actions
    }

    /// The balances an observation reads, by account and asset.
    pub fn observed(&self) -> &[(AccountPath, AssetPath)] {
        &self.observed
    }

    /// The action that is `exchange`, if the task lists it.
    pub fn action(&self, exchange: &paths::Exchange) -> Option<usize> {
        self.by_rate
            .get(exchange.rate())?
            .iter()
            .copied()
            .find(|&index| self.actions[index] == *exchange)
    }

    /// What a learner sees at `at`: the observed balances, and the actions
    /// the candidate set offers there. A goal state offers no further actions.
    pub fn observe(&self, at: Point) -> Result<Observation, RlError> {
        Ok(self.observe_goal(at)?.0)
    }

    fn observe_goal(&self, at: Point) -> Result<(Observation, bool), RlError> {
        let reached = self.engine.reach(at)?;
        let economy = &reached.state.economy;
        let goal = economy.matches(&self.engine.goal(&self.goal)?);
        let balances = self
            .observed
            .iter()
            .map(|(account, asset)| f32::from(economy.balance(account, asset).value()))
            .collect();
        let mut offered = vec![false; self.actions.len()];
        if !goal {
            for exchange in self.engine.candidates(at, &self.candidates)? {
                let action = self.action(&exchange).ok_or_else(|| RlError::Unlisted {
                    set: self.candidates.clone(),
                    rate: exchange.rate().clone(),
                })?;
                offered[action] = true;
            }
        }
        Ok((Observation { balances, offered }, goal))
    }

    /// Each weighed objective's value at `at`.
    fn measure(&self, at: Point) -> Result<Vec<Quantity>, RlError> {
        self.rewards
            .iter()
            .map(|(objective, _)| Ok(self.engine.measure(at, objective)?))
            .collect()
    }
}

/// The exchanges `set`, kept at `path`, offers at any state of `economy`'s
/// rates, each once, in order.
fn listed(
    economy: &paths::Economy,
    path: &CandidatesPath,
    set: CandidateSet,
) -> Result<Vec<paths::Exchange>, RlError> {
    match set {
        CandidateSet::Script(_) => Err(RlError::Scripted(path.clone())),
        CandidateSet::Listed(exchanges) => {
            let mut listed: Vec<paths::Exchange> = Vec::with_capacity(exchanges.len());
            for exchange in exchanges {
                if !listed.contains(&exchange) {
                    listed.push(exchange);
                }
            }
            Ok(listed)
        }
        CandidateSet::Instances { under } => {
            // An instance fixing every role is offered as one exchange
            // whatever the state; one leaving a role free is not.
            for id in instances(economy, under.as_ref()) {
                let fixed = economy
                    .exchange_of(&id)
                    .map_or(0, |exchange| exchange.bindings().len());
                let roles = economy
                    .instance(&id)
                    .map_or(0, |instance| instance.roles().count());
                if fixed != roles {
                    return Err(RlError::FreeRole(id));
                }
            }
            Ok(offered(economy, under.as_ref())
                .map(Cow::into_owned)
                .collect())
        }
    }
}

/// Every account and asset `economy` holds a balance of, or that one of
/// `actions` consumes, keeps or produces, in order.
fn observed(
    economy: &paths::Economy,
    actions: &[paths::Exchange],
) -> Vec<(AccountPath, AssetPath)> {
    let mut observed: BTreeSet<(AccountPath, AssetPath)> = economy
        .accounts()
        .flat_map(|(account, held)| {
            held.balances()
                .iter()
                .map(move |(asset, _)| (account.clone(), asset.clone()))
        })
        .collect();
    for exchange in actions {
        let Ok(instance) = economy.instance(exchange.rate()) else {
            continue;
        };
        let bindings = exchange.bindings();
        for role in instance.roles() {
            let Some(account) = bindings.get(role) else {
                continue;
            };
            let baskets = [
                instance.consumed(role),
                instance.preserved(role),
                instance.produced(role),
            ];
            for (asset, _) in baskets
                .into_iter()
                .flatten()
                .flat_map(|basket| basket.iter())
            {
                if let Some(asset) = asset.as_literal() {
                    observed.insert((account.clone(), asset.clone()));
                }
            }
        }
        let computed = instance
            .computed_consumed()
            .iter()
            .chain(instance.computed_preserved())
            .chain(instance.computed_produced());
        for amount in computed {
            if let (Some(account), Some(asset)) =
                (bindings.get(amount.role()), amount.asset().as_literal())
            {
                observed.insert((account.clone(), asset.clone()));
            }
        }
    }
    observed.into_iter().collect()
}

/// One episode of a task: the point it is at, what is observed there and
/// the weighed objectives' values there.
#[derive(Clone)]
pub struct Episode {
    task: Arc<Task>,
    point: Point,
    measured: Vec<Quantity>,
    observation: Observation,
}

impl Episode {
    /// An episode of `task`, at its start.
    pub fn new(task: Arc<Task>) -> Self {
        Self {
            point: task.start,
            measured: task.measured.clone(),
            observation: task.observation.clone(),
            task,
        }
    }

    /// The task the episode is of.
    pub fn task(&self) -> &Arc<Task> {
        &self.task
    }

    /// Starts the episode again from the task's start.
    pub fn reset(&mut self) {
        *self = Self::new(Arc::clone(&self.task));
    }

    /// The point the episode is at.
    pub fn point(&self) -> Point {
        self.point
    }

    /// What a learner sees where the episode is.
    pub fn observation(&self) -> &Observation {
        &self.observation
    }

    /// Whether the goal holds or no action is available, including at reset.
    pub fn ended(&self) -> bool {
        !self.observation.offered.contains(&true)
    }

    /// Applies the task's exchange numbered `action`, refused where the
    /// candidate set does not offer it here, and moves the episode to the
    /// point it leads to.
    pub fn step(&mut self, action: usize) -> Result<Step, RlError> {
        if self.ended() {
            return Err(RlError::EpisodeEnded);
        }
        let task = &self.task;
        let exchange = task.actions.get(action).ok_or(RlError::NoSuchAction {
            action,
            actions: task.actions.len(),
        })?;
        if !self.observation.offered[action] {
            return Err(RlError::NotOffered {
                action,
                rate: exchange.rate().clone(),
            });
        }
        let point = task
            .engine
            .apply(self.point, std::slice::from_ref(exchange))?;
        let measured = task.measure(point)?;
        let (observation, reached) = task.observe_goal(point)?;
        let mut reward = task
            .rewards
            .iter()
            .zip(measured.iter().zip(&self.measured))
            .map(|((_, weight), (after, before))| weight * change(*before, *after))
            .sum::<f64>();
        if reached {
            reward += task.reached;
        }
        let stuck = !reached && !observation.offered.contains(&true);
        self.point = point;
        self.measured = measured;
        self.observation = observation;
        Ok(Step {
            point,
            reward,
            reached,
            stuck,
        })
    }
}

/// How much `after` is above `before`, below zero where it is less, to the
/// nearest `f64`.
fn change(before: Quantity, after: Quantity) -> f64 {
    let (before, after) = (before.value(), after.value());
    if after >= before {
        f64::from(after - before)
    } else {
        -f64::from(before - after)
    }
}
