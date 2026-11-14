#![doc = include_str!("../README.md")]

mod cache;
pub mod definitions;
mod divergence;
mod experiment;
mod histories;
mod package;
mod refs;
pub mod script;
mod strategy;
mod view;

pub use definitions::{
    CandidateSet, CandidatesPath, ExperimentPath, GoalPath, Measure, Objective, ObjectivePath,
    ScriptCall, ScriptFunction, StrategyPath,
};
pub use divergence::{AccountDifference, Aligned, BalanceDifference, Divergence, Side, Tip};
pub use experiment::{Arm, ArmResult, Comparison, Ended, Experiment, Playing, Results, Trial};
pub use histories::{Change, Segment, Subscriber};
pub use package::{
    Contents, Loaded, Manifest, PackageError, PackagePath, Version, VersionError, package, zipped,
};
pub use refs::{Following, Frame};
pub use rhai;
pub use script::{HOST_API, Random, ScriptPath, ScriptState, exchanges_of};
pub use strategy::{Chooser, Effort, Outcome, Ran, Strategy};
pub use view::{View, ViewPath};

use axionomy::{
    PathKind, Quantity,
    path::Path,
    paths::{self, Paths},
};
use axionomy_store::{
    Checkpoints, DefinitionPath, Durability, Point, Reader, RefPath, Run, Store, StoreError,
};
use cache::Cache;
use definitions::definition;
use script::Scripts;
use serde::{Serialize, de::DeserializeOwned};
use std::{num::NonZeroU64, sync::Arc};
use thiserror::Error;

/// A state the store holds, written as paths: the economy, and its canonical
/// tree, whose hash is the state's root.
pub type State = axionomy_store::State<Paths>;

/// States a run of applied exchanges stores besides its first and last:
/// reaching any of its steps replays at most this many exchanges.
const CHECKPOINT_EVERY: NonZeroU64 = NonZeroU64::new(64).expect("nonzero");
/// Points kept in memory with their states and receipts.
const CACHED: usize = 32;

#[derive(Debug, Clone, Error)]
pub enum EngineError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("step {} of run {} is not a point of this store", .0.step, .0.run)]
    UnknownPoint(Point),
    #[error("step {step} is not on the history of step {} of run {}", .at.step, .at.run)]
    NotOnHistory { at: Point, step: u64 },
    /// Two points whose histories share no point: neither history runs
    /// through the other's, nor do both start from a third, as for two
    /// economies authored apart.
    #[error(
        "step {} of run {} and step {} of run {} share no history",
        .left.step, .left.run, .right.step, .right.run
    )]
    Unrelated { left: Point, right: Point },
    /// An exchange of those applied together did not apply, so none was
    /// recorded. `index` counts from zero, and `assessment` explains the
    /// failure against the economy that exchange met.
    #[error("exchange {index} does not apply: it is {:?}", .assessment.status())]
    Refused {
        index: usize,
        assessment: Box<paths::Assessment>,
    },
    #[error("the store has no script `{0}`")]
    UnknownScript(ScriptPath),
    /// A script imports one the store does not keep.
    #[error("script `{script}` imports `{import}`, which the store does not keep")]
    UnknownImport {
        script: ScriptPath,
        import: ScriptPath,
    },
    /// Scripts that import one another in a cycle: each imports the next,
    /// and the last is the first.
    #[error("scripts import one another in a cycle: {}", cycle(.0))]
    ImportCycle(Vec<ScriptPath>),
    /// Two scripts, one importing the other or both imported by a script,
    /// directly or through others, that define a function of the same name
    /// and number of parameters, which Rhai could call one for the other.
    #[error(
        "scripts `{first}` and `{second}` both define `{function}` with {parameters} parameters: a script and those it imports name their functions apart"
    )]
    SameFunction {
        function: ScriptFunction,
        parameters: usize,
        first: ScriptPath,
        second: ScriptPath,
    },
    #[error("no ref `{0}` names a point")]
    UnknownRef(RefPath),
    /// A script that does not compile, or failed while it ran; `message` is
    /// Rhai's, with the position it names.
    #[error("script `{path}`: {message}")]
    Script { path: ScriptPath, message: String },
    #[error("a script returned {found} where {expected} was expected")]
    Returned {
        expected: &'static str,
        found: String,
    },
    #[error("the store defines no `{0}`")]
    Undefined(DefinitionPath),
    /// An objective summed over a history, asked of an economy without one.
    #[error("objective `{0}` is measured over a history, so it needs a point")]
    NeedsHistory(ObjectivePath),
    /// A tree search could not decide, with the reason it gives.
    #[error("no decision: {0}")]
    Undecided(String),
    #[error(transparent)]
    Package(#[from] PackageError),
    /// A ref was not on the point a move expected; it is on `current`, or
    /// names nothing.
    #[error("ref `{name}` is not where the move expected it")]
    Moved {
        name: RefPath,
        current: Option<Point>,
    },
    /// The caller stopped an experiment before it ended; it kept no results.
    /// A strategy's run its caller stops ends as
    /// [`Outcome::Interrupted`] instead.
    #[error("the experiment was stopped before it ended")]
    Interrupted,
    /// A ref moves forward only, over steps on the history of where it goes.
    #[error("ref `{name}` cannot move to step {} of {}: its point is not on that history", .to.step, .to.run)]
    Backward { name: RefPath, to: Point },
    /// A ref a package ships is fixed: it is never moved or deleted, so the
    /// package means what it shipped.
    #[error(
        "ref `{name}` is one package `{package}` ships, so it is fixed: name a ref of your own at its point and move that"
    )]
    Shipped { name: RefPath, package: PackagePath },
}

/// A point as the engine serves it: the state there, the point its run
/// started from, if it forked, and the receipts of the steps its run took to
/// reach it, in order.
#[derive(Debug)]
pub struct Reached {
    pub point: Point,
    pub state: State,
    /// The point the run forked from, whose state is the run's first; none for
    /// a run that starts from an economy of its own.
    pub from: Option<Point>,
    pub receipts: Vec<paths::Receipt>,
}

/// The work every front end shares, over a store and nothing registered.
#[derive(Clone)]
pub struct Engine {
    store: Store,
    reached: Cache<Point, Reached>,
    host: Arc<rhai::Engine>,
    scripts: Scripts,
    /// Told each time a ref moves, so a follower waiting on one wakes.
    moved: Arc<refs::Moves>,
    /// Told of every run recorded, ref moved and definition kept.
    changes: Arc<histories::Changes>,
}

impl Engine {
    /// An engine keeping what it records in `store`.
    pub fn new(store: Store) -> Result<Self, EngineError> {
        let scripts = Scripts::new(store.clone());
        Ok(Self {
            store,
            reached: Cache::new(CACHED),
            host: Arc::new(script::host(scripts.clone())),
            scripts,
            moved: Arc::default(),
            changes: Arc::default(),
        })
    }

    /// Keeps `source` as the script at `path`, replacing any script there.
    /// Refuses one that does not compile, or whose imports do not check out:
    /// each names by a literal path a script the store keeps, so a script is
    /// kept after those it imports; none imports it back, directly or
    /// through others; and no two of them, nor it and one of them, define a
    /// function of the same name and number of parameters.
    pub fn keep_script(&self, path: &ScriptPath, source: &str) -> Result<(), EngineError> {
        let compiled = self.scripts.checked(&self.host, path, source)?;
        self.keep(definition(path), source.as_bytes().to_vec())?;
        self.scripts.kept(path, compiled);
        Ok(())
    }

    /// The source of the script at `path`.
    pub fn script(&self, path: &ScriptPath) -> Result<Option<String>, EngineError> {
        self.scripts.source(path)
    }

    /// Calls `function` of the script at `path` with `arguments`, Rhai values
    /// such as a [`ScriptState`], a [`Random`] or a [`axionomy::Quantity`],
    /// and returns what it returns, which must be a `T`.
    pub fn call<T: Clone + Send + Sync + 'static>(
        &self,
        path: &ScriptPath,
        function: &str,
        arguments: impl rhai::FuncArgs,
    ) -> Result<T, EngineError> {
        let compiled = self.compiled(path)?;
        let returned: rhai::Dynamic = self
            .host
            .call_fn(&mut rhai::Scope::new(), &compiled.ast, function, arguments)
            .map_err(|error| EngineError::Script {
                path: path.clone(),
                message: error.to_string(),
            })?;
        let found = returned.type_name();
        returned
            .try_cast::<T>()
            .ok_or_else(|| EngineError::Returned {
                expected: std::any::type_name::<T>(),
                found: found.to_owned(),
            })
    }

    /// Keeps `goal` at `path`, replacing any goal there.
    pub fn keep_goal(&self, path: &GoalPath, goal: &paths::Goal) -> Result<(), EngineError> {
        self.define(path, goal)
    }

    pub fn goal(&self, path: &GoalPath) -> Result<paths::Goal, EngineError> {
        self.defined(path)
    }

    /// Whether the economy at `at` holds the goal at `goal`.
    pub fn holds(&self, at: Point, goal: &GoalPath) -> Result<bool, EngineError> {
        Ok(self.reach(at)?.state.economy.matches(&self.goal(goal)?))
    }

    /// Keeps `objective` at `path`, refusing one whose script is not kept.
    pub fn keep_objective(
        &self,
        path: &ObjectivePath,
        objective: &Objective,
    ) -> Result<(), EngineError> {
        let (Measure::State(call) | Measure::Steps(call)) = &objective.measure;
        self.require_script(&call.script)?;
        self.define(path, objective)
    }

    pub fn objective(&self, path: &ObjectivePath) -> Result<Objective, EngineError> {
        self.defined(path)
    }

    /// The value of the objective at `objective` at `at`: its measure of the
    /// state there, or the sum of its step costs over every exchange of the
    /// history that reached it.
    pub fn measure(&self, at: Point, objective: &ObjectivePath) -> Result<Quantity, EngineError> {
        match self.objective(objective)?.measure {
            Measure::State(call) => self.measure_state(&call, &self.reach(at)?.state.economy),
            Measure::Steps(call) => {
                let (mut economy, exchanges) = self.history(at)?;
                let mut total = Quantity::ZERO;
                for exchange in exchanges {
                    let before = economy.clone();
                    economy.apply(exchange.clone()).map_err(|_| {
                        EngineError::Store(StoreError::Malformed("a history that does not replay"))
                    })?;
                    let cost = self.step_cost(&call, &before, &exchange, &economy)?;
                    total = total.checked_add(cost).ok_or(EngineError::Returned {
                        expected: "step costs whose sum is a quantity",
                        found: "a sum out of range".to_owned(),
                    })?;
                }
                Ok(total)
            }
        }
    }

    /// The value of the objective at `objective` for an economy alone, which
    /// only an objective measuring a state has.
    pub fn measure_economy(
        &self,
        objective: &ObjectivePath,
        economy: &paths::Economy,
    ) -> Result<Quantity, EngineError> {
        match self.objective(objective)?.measure {
            Measure::State(call) => self.measure_state(&call, economy),
            Measure::Steps(_) => Err(EngineError::NeedsHistory(objective.clone())),
        }
    }

    /// What `call` measures of `economy`: `fn(state)`.
    pub fn measure_state(
        &self,
        call: &ScriptCall,
        economy: &paths::Economy,
    ) -> Result<Quantity, EngineError> {
        let measured: rhai::Dynamic = self.call(
            &call.script,
            call.function.as_str(),
            (ScriptState::new(economy.clone()),),
        )?;
        quantity(measured)
    }

    /// What `call` charges for `exchange` taking `before` to `after`:
    /// `fn(before, exchange, after)`.
    pub fn step_cost(
        &self,
        call: &ScriptCall,
        before: &paths::Economy,
        exchange: &paths::Exchange,
        after: &paths::Economy,
    ) -> Result<Quantity, EngineError> {
        let cost: rhai::Dynamic = self.call(
            &call.script,
            call.function.as_str(),
            (
                ScriptState::new(before.clone()),
                exchange.clone(),
                ScriptState::new(after.clone()),
            ),
        )?;
        quantity(cost)
    }

    /// Keeps the candidate set at `path`, refusing one whose script is not
    /// kept.
    pub fn keep_candidates(
        &self,
        path: &CandidatesPath,
        set: &CandidateSet,
    ) -> Result<(), EngineError> {
        if let CandidateSet::Script(call) = set {
            self.require_script(&call.script)?;
        }
        self.define(path, set)
    }

    /// The exchanges the candidate set at `set` offers at `at`: what its
    /// script proposes, or those of its listed exchanges that apply.
    pub fn candidates(
        &self,
        at: Point,
        set: &CandidatesPath,
    ) -> Result<Vec<paths::Exchange>, EngineError> {
        let economy = &self.reach(at)?.state.economy;
        match self.defined(set)? {
            CandidateSet::Script(call) => self.propose(&call, economy),
            CandidateSet::Listed(exchanges) => Ok(economy.applicable(exchanges)),
        }
    }

    /// Keeps `strategy` at `path`, refusing one that names a candidate set,
    /// an objective or a script the store does not keep.
    pub fn keep_strategy(
        &self,
        path: &StrategyPath,
        strategy: &Strategy,
    ) -> Result<(), EngineError> {
        let mut candidates: Vec<&CandidatesPath> = Vec::new();
        let mut scripts: Vec<&ScriptCall> = Vec::new();
        match strategy {
            Strategy::Policy { policy, .. } => scripts.push(policy),
            Strategy::Plan { plan } => scripts.push(plan),
            Strategy::BreadthFirst { candidates: set } => candidates.push(set),
            Strategy::Cheapest {
                candidates: set,
                cost,
                heuristic,
            }
            | Strategy::BestFirst {
                candidates: set,
                cost,
                heuristic,
            } => {
                candidates.push(set);
                scripts.push(cost);
                scripts.extend(heuristic);
            }
            Strategy::BranchAndBound {
                candidates: set,
                cost,
                bound,
            } => {
                candidates.push(set);
                scripts.extend([cost, bound]);
            }
            Strategy::Beam {
                candidates: set,
                score,
                ..
            } => {
                candidates.push(set);
                scripts.push(score);
            }
            Strategy::Pareto {
                candidates: set,
                objectives,
                by,
            } => {
                candidates.push(set);
                for objective in objectives {
                    if let Measure::Steps(_) = self.objective(objective)?.measure {
                        return Err(EngineError::NeedsHistory(objective.clone()));
                    }
                }
                if let Some(missing) = by.iter().find(|objective| !objectives.contains(objective)) {
                    return Err(EngineError::Undefined(definition(missing)));
                }
            }
            Strategy::Game {
                nature,
                actor,
                players,
                ..
            } => {
                scripts.push(actor);
                scripts.extend(nature);
                for player in players {
                    match player {
                        Chooser::Policy(policy) => scripts.push(policy),
                        Chooser::Minimax {
                            candidates: set,
                            evaluate,
                            ..
                        } => {
                            candidates.push(set);
                            scripts.push(evaluate);
                        }
                        Chooser::Mcts {
                            candidates: set,
                            payoffs,
                            estimate,
                            chance,
                            rollout,
                            ..
                        } => {
                            candidates.push(set);
                            scripts.extend([payoffs, estimate]);
                            scripts.extend(chance.iter().chain(rollout));
                        }
                        Chooser::Ismcts {
                            candidates: set,
                            observe,
                            chance,
                            payoffs,
                            estimate,
                            rollout,
                            ..
                        } => {
                            candidates.push(set);
                            scripts.extend([observe, chance, payoffs, estimate]);
                            scripts.extend(rollout);
                        }
                    }
                }
            }
        }
        for set in candidates {
            self.defined::<_, CandidateSet>(set)?;
        }
        for call in scripts {
            self.require_script(&call.script)?;
        }
        self.define(path, strategy)
    }

    pub fn strategy(&self, path: &StrategyPath) -> Result<Strategy, EngineError> {
        self.defined(path)
    }

    /// The exchanges `call` proposes for `economy`: `fn(state)`, returning
    /// an array of exchanges.
    pub fn propose(
        &self,
        call: &ScriptCall,
        economy: &paths::Economy,
    ) -> Result<Vec<paths::Exchange>, EngineError> {
        let proposed: rhai::Array = self.call(
            &call.script,
            call.function.as_str(),
            (ScriptState::new(economy.clone()),),
        )?;
        exchanges_of(proposed)
    }

    /// The economy a point's history starts from, and the exchanges that
    /// take it to the point, across the runs it forked from.
    pub fn history(
        &self,
        at: Point,
    ) -> Result<(paths::Economy, Vec<paths::Exchange>), EngineError> {
        let reader = self.store.read()?;
        run_at(&reader, at)?;
        let mut segments = Vec::new();
        let mut on = at;
        let root = loop {
            let run = reader
                .run_of::<Paths>(on.run)?
                .ok_or(EngineError::UnknownPoint(on))?;
            segments.push((on.run, run.first, on.step));
            match run.from {
                Some(from) => on = from,
                None => {
                    break Point {
                        run: on.run,
                        step: run.first,
                    };
                }
            }
        };
        let economy = first_state(&reader, root, root.step)?.economy;
        let mut exchanges = Vec::new();
        for (run, first, last) in segments.into_iter().rev() {
            let count = usize::try_from(last - first).unwrap_or(usize::MAX);
            for (_, step) in reader.steps::<Paths>(run, first + 1, count)? {
                exchanges.push(step.exchange);
            }
        }
        Ok((economy, exchanges))
    }

    fn require_script(&self, path: &ScriptPath) -> Result<(), EngineError> {
        match self.script(path)? {
            Some(_) => Ok(()),
            None => Err(EngineError::UnknownScript(path.clone())),
        }
    }

    /// Keeps `value`, in postcard, as what `path` defines.
    fn define<K: PathKind, T: Serialize>(
        &self,
        path: &Path<K>,
        value: &T,
    ) -> Result<(), EngineError> {
        let bytes = postcard::to_allocvec(value).expect("definitions encode");
        self.keep(definition(path), bytes)
    }

    fn defined<K: PathKind, T: DeserializeOwned>(&self, path: &Path<K>) -> Result<T, EngineError> {
        let name = definition(path);
        let bytes = self
            .store
            .read()?
            .definition(&name)?
            .ok_or(EngineError::Undefined(name))?;
        postcard::from_bytes(&bytes)
            .map_err(|_| EngineError::Store(StoreError::Malformed("a definition")))
    }

    fn keep(&self, name: DefinitionPath, bytes: Vec<u8>) -> Result<(), EngineError> {
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.define(&name, Some(&bytes))
            })
            .wait()?;
        self.changes.tell(&Change::Definitions);
        Ok(())
    }

    /// The script at `path`, compiled.
    fn compiled(&self, path: &ScriptPath) -> Result<Arc<script::Compiled>, EngineError> {
        self.scripts.compiled(&self.host, path)
    }

    /// Whether the script at `path` defines `function` with `parameters`
    /// parameters.
    fn defines(
        &self,
        path: &ScriptPath,
        function: &str,
        parameters: usize,
    ) -> Result<bool, EngineError> {
        Ok(self
            .compiled(path)?
            .ast
            .iter_functions()
            .any(|defined| defined.name == function && defined.params.len() == parameters))
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Records `economy` as the first point of a run of its own.
    pub fn author(&self, economy: paths::Economy) -> Result<Point, EngineError> {
        let state = State::new(economy);
        let point = Point {
            run: self.store.mint_run(),
            step: 0,
        };
        let tree = state.tree.clone();
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.create_run::<Paths>(point.run, None, 0, &tree, Checkpoints::Ends)?;
                txn.pin(point, None)
            })
            .wait()?;
        self.changes.tell(&Change::Run(Segment {
            from: None,
            tip: point,
        }));
        self.reached.insert(
            point,
            Reached {
                point,
                state,
                from: None,
                receipts: Vec::new(),
            },
        );
        Ok(point)
    }

    /// Applies `exchanges` at `at`, all of them or none, and records them as a
    /// run forked from `at`, returning the point they reach. The point `at`
    /// never changes, and keeps its one name: the fork's first state is `at`,
    /// and the fork's points are the steps it takes. Applying no exchanges
    /// reaches `at` itself.
    pub fn apply(&self, at: Point, exchanges: &[paths::Exchange]) -> Result<Point, EngineError> {
        let source = self.reach(at)?;
        if exchanges.is_empty() {
            return Ok(at);
        }
        let mut state = source.state.clone();
        let mut steps = Vec::with_capacity(exchanges.len());
        for (index, exchange) in exchanges.iter().enumerate() {
            let Ok(receipt) = state.apply(exchange.clone()) else {
                return Err(EngineError::Refused {
                    index,
                    assessment: Box::new(state.economy.assess(exchange)),
                });
            };
            let step = at.step + index as u64 + 1;
            steps.push((receipt, checkpoint(step).then(|| state.tree.clone())));
        }
        let run = self.store.mint_run();
        let last = Point {
            run,
            step: at.step + steps.len() as u64,
        };
        let receipts = steps.iter().map(|(receipt, _)| receipt.clone()).collect();
        let first = source.state.tree.clone();
        let last_tree = state.tree.clone();
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.create_run::<Paths>(
                    run,
                    Some(at),
                    at.step,
                    &first,
                    Checkpoints::Every(CHECKPOINT_EVERY),
                )?;
                for (index, (receipt, tree)) in steps.into_iter().enumerate() {
                    let point = Point {
                        run,
                        step: at.step + index as u64 + 1,
                    };
                    txn.put_step::<Paths>(point, &receipt, tree.as_ref())?;
                }
                txn.pin(last, Some(&last_tree))
            })
            .wait()?;
        self.changes.tell(&Change::Run(Segment {
            from: Some(at),
            tip: last,
        }));
        self.reached.insert(
            last,
            Reached {
                point: last,
                state,
                from: Some(at),
                receipts,
            },
        );
        Ok(last)
    }

    /// The state at `at`, with the receipts of the steps its run took to
    /// reach it, each replayed and checked against what was recorded. A
    /// fork's first state is refused under the fork's name: it is a point
    /// of the run the fork started from, and has that name alone.
    pub fn reach(&self, at: Point) -> Result<Arc<Reached>, EngineError> {
        if let Some(reached) = self.reached.get(&at) {
            return Ok(reached);
        }
        let reader = self.store.read()?;
        let run = run_at(&reader, at)?;
        let mut state = first_state(&reader, at, run.first)?;
        let receipts = reader.replay(&mut state, at.run, run.first, at.step)?;
        Ok(self.reached.insert(
            at,
            Reached {
                point: at,
                state,
                from: run.from,
                receipts,
            },
        ))
    }

    /// The point at step `step` of `at`'s history, by its one name, its
    /// state stored so it is reached as directly as `at`. The history of a
    /// run that forked continues into the run it forked from, so a step
    /// `at`'s run did not take is named in the run that took it.
    pub fn ancestor(&self, at: Point, step: u64) -> Result<Point, EngineError> {
        let point = self.on_history(at, step)?;
        let state = self
            .store
            .read()?
            .state::<Paths>(point)?
            .ok_or(EngineError::UnknownPoint(point))?;
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.pin(point, Some(&state.tree))
            })
            .wait()?;
        Ok(point)
    }

    /// The point at step `step` of `at`'s history, named by the run whose
    /// step reached it, or the first point of a run that starts from an
    /// economy of its own: a fork's first state is the point it forked from.
    pub(crate) fn on_history(&self, at: Point, step: u64) -> Result<Point, EngineError> {
        let reader = self.store.read()?;
        let mut run = run_at(&reader, at)?;
        if step > at.step {
            return Err(EngineError::NotOnHistory { at, step });
        }
        let mut on = at.run;
        loop {
            match run.from {
                Some(from) if step <= run.first => {
                    on = from.run;
                    run = reader.run(on)?.ok_or(EngineError::UnknownPoint(from))?;
                }
                None if step < run.first => {
                    return Err(EngineError::NotOnHistory { at, step });
                }
                _ => return Ok(Point { run: on, step }),
            }
        }
    }

    /// Whether `exchange` would apply at the economy `before` reaches from
    /// `at`, and what it would move or what it lacks. Nothing is recorded.
    pub fn assess(
        &self,
        at: Point,
        before: &[paths::Exchange],
        exchange: &paths::Exchange,
    ) -> Result<paths::Assessment, EngineError> {
        let reached = self.reach(at)?;
        let mut economy = reached.state.economy.clone();
        for (index, earlier) in before.iter().enumerate() {
            if economy.apply(earlier.clone()).is_err() {
                return Err(EngineError::Refused {
                    index,
                    assessment: Box::new(economy.assess(earlier)),
                });
            }
        }
        Ok(economy.assess(exchange))
    }
}

/// The run of `at`, which must name one of the store's points: a step its
/// run took, or the first state of a run that starts from an economy of its
/// own. A fork's first state is not a point of the fork; it has the name of
/// the point the fork started from.
fn run_at(reader: &Reader, at: Point) -> Result<Run, EngineError> {
    let unknown = || EngineError::UnknownPoint(at);
    let run = reader.run(at.run)?.ok_or_else(unknown)?;
    let last = reader.last_step(at.run)?.ok_or_else(unknown)?;
    let named = match run.from {
        Some(_) => at.step > run.first,
        None => at.step >= run.first,
    };
    if !named || at.step > last {
        return Err(unknown());
    }
    Ok(run)
}

/// The first state of `at`'s run, which starts at step `first`.
fn first_state(reader: &Reader, at: Point, first: u64) -> Result<State, EngineError> {
    let point = Point {
        run: at.run,
        step: first,
    };
    reader
        .state::<Paths>(point)?
        .ok_or(EngineError::UnknownPoint(point))
}

/// A quantity a script returned: a quantity, or an integer that is not
/// negative.
fn quantity(returned: rhai::Dynamic) -> Result<Quantity, EngineError> {
    let found = returned.type_name();
    if returned.is::<Quantity>() {
        return Ok(returned.cast());
    }
    returned
        .as_int()
        .ok()
        .and_then(|value| u64::try_from(value).ok())
        .map(Quantity::new)
        .ok_or_else(|| EngineError::Returned {
            expected: "a quantity",
            found: found.to_owned(),
        })
}

/// Scripts importing one another in a cycle, as `a` → `b` → `a`.
fn cycle(scripts: &[ScriptPath]) -> String {
    scripts
        .iter()
        .map(|script| format!("`{script}`"))
        .collect::<Vec<_>>()
        .join(" → ")
}

/// Whether a run of applied exchanges stores the state after step `step`
/// before its last.
fn checkpoint(step: u64) -> bool {
    step.is_multiple_of(CHECKPOINT_EVERY.get())
}
