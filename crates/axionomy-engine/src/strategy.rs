//! Strategies: a built-in algorithm of `axionomy-search`, the definitions it
//! calls and its parameters (D-050). The engine runs one from a point and
//! records the exchanges it reaches as a run forked from that point.

use crate::{
    CandidatesPath, Engine, EngineError, GoalPath, ObjectivePath, Random, ScriptCall, ScriptState,
    definitions::{CandidateSet, Measure, Objective},
};
use axionomy::{
    ObservationKey, Quantity,
    paths::{self, Paths},
};
use axionomy_search::{
    AStarSession, BfsSession, GraphSearchProgress,
    action_source::eager_actions,
    best_first,
    bounded::{Tie, beam, branch_and_bound, minimax},
    dijkstra,
    ismcts::{InformationState, search as ismcts},
    mcts::{MctsConfig, search as mcts},
    pareto::{Objective as Measured, ObjectiveDirection, ObjectiveVector, ParetoSearchSession},
    sampling::{SeededSampler, TicketSource, WeightedExchange, sample, sample_seed},
    session::{SearchStatus, WorkBudget},
};
use axionomy_store::Point;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, btree_map::Entry},
    hash::{Hash, Hasher},
    ops::ControlFlow,
};

/// The most expansions a search makes between two looks at its progress.
const CHUNK: usize = 1_000;

/// An algorithm with what it calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strategy {
    /// A policy driving the economy turn by turn: each turn it proposes one
    /// exchange, `fn(state, random)`, or nothing, until the goal holds, it
    /// proposes nothing, or `horizon` turns. Turn order, Nature's responses
    /// and who acts are the policy's to decide.
    Policy {
        policy: ScriptCall,
        goal: GoalPath,
        horizon: u64,
    },
    /// A plan a script makes whole, `fn(state)` returning the exchanges in
    /// order or nothing, such as a specialised solver's or an outside
    /// solver's answer, or `fn(state, random)` for a plan that draws chance
    /// (Nature's outcomes) from the run's seed: the kernel applies them all
    /// or none, and the run ends at the goal or where the plan ends.
    Plan { plan: ScriptCall, goal: GoalPath },
    /// The fewest exchanges from the candidate set that reach the goal.
    BreadthFirst {
        candidates: CandidatesPath,
        goal: GoalPath,
    },
    /// The cheapest exchanges from the candidate set that reach the goal,
    /// each costing what `cost` charges, `fn(before, exchange, after)`: by
    /// Dijkstra's search as `axionomy_search::dijkstra` runs it, which of
    /// equally cheap plans reaches the one its heap yields first and stops
    /// without a plan once it has expanded its budget; or, when `heuristic`,
    /// `fn(state)`, bounds the cost still to pay from below, by A*, watched
    /// as it goes.
    Cheapest {
        candidates: CandidatesPath,
        goal: GoalPath,
        cost: ScriptCall,
        heuristic: Option<ScriptCall>,
    },
    /// The cheapest goal state the candidate set reaches, by depth-first
    /// branch-and-bound: `cost` is a goal state's cost, `fn(state)`, and
    /// `bound` a lower bound on the cost of any goal state reachable from a
    /// state, `fn(state)`.
    BranchAndBound {
        candidates: CandidatesPath,
        goal: GoalPath,
        cost: ScriptCall,
        bound: ScriptCall,
    },
    /// Beam search: each depth keeps the `width` states that score best,
    /// `fn(state)` returning an integer, higher better, for at most `depth`
    /// depths; the best-scoring goal state found, the first or the last
    /// found of those scoring alike, as `tie` says.
    Beam {
        candidates: CandidatesPath,
        goal: GoalPath,
        score: ScriptCall,
        width: u64,
        depth: u64,
        tie: Tie,
    },
    /// The goal state the candidate set reaches that ranks best by `cost`,
    /// `fn(state)`, a state's whole cost so far, plus `heuristic`'s estimate,
    /// `fn(state)`, of what is left, lower first; states of equal rank in the
    /// order they were found. It runs to its end.
    BestFirst {
        candidates: CandidatesPath,
        goal: GoalPath,
        cost: ScriptCall,
        heuristic: Option<ScriptCall>,
    },
    /// The exact Pareto front of the goal states the candidate set reaches,
    /// each measured by `objectives`, which measure states: the run reaches
    /// the front's best state by the objectives `by`, each one of
    /// `objectives`, in order, the first found on a tie. A budget spent
    /// before the front is exact leaves it at its start.
    Pareto {
        candidates: CandidatesPath,
        goal: GoalPath,
        objectives: Vec<ObjectivePath>,
        by: Vec<ObjectivePath>,
    },
    /// A game played turn by turn until the goal or `horizon` turns: each
    /// turn Nature responds if `nature`, `fn(state, random)`, proposes an
    /// exchange, and otherwise the player `actor`, `fn(state)` returning an
    /// index into `players`, chooses as its chooser does. Nature draws from
    /// a random stream of its own, so how hard the players think never
    /// changes what Nature does: weighted scenarios are Nature sampling
    /// them, and receding-horizon planning is a player that searches again
    /// every turn from what happened.
    Game {
        goal: GoalPath,
        horizon: u64,
        nature: Option<ScriptCall>,
        actor: ScriptCall,
        players: Vec<Chooser>,
    },
}

/// How a player in a game chooses its exchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Chooser {
    /// What a policy proposes, `fn(state, random)`.
    Policy(ScriptCall),
    /// Minimax with alpha-beta pruning `depth` exchanges ahead, over the
    /// candidate set, between players 0 and 1: `evaluate`, `fn(state)`,
    /// scores a state for player 0 as an integer, higher better for 0.
    Minimax {
        candidates: CandidatesPath,
        evaluate: ScriptCall,
        depth: u64,
    },
    /// Monte Carlo tree search, `iterations` iterations each turn, over the
    /// candidate set: `payoffs`, `fn(state)`, gives every player's payoff at
    /// a state the game ends in, an array of numbers, or nothing at any
    /// other; `estimate`, `fn(state)`, every player's expected payoff where
    /// a simulation stops at `depth`. Where `chance`, `fn(state)`, gives
    /// Nature's weighted outcomes, `[exchange, weight]` pairs, the tree
    /// draws one by its weight instead of a player choosing. Simulations
    /// choose as `rollout`, `fn(state, candidates)`, does, one of the
    /// candidates or nothing to stop there, and uniformly without one. The
    /// search of the game's `k`th choice, counting from zero the turns Nature
    /// did not respond on, is seeded with the game's seed XOR `salt`, plus
    /// `k`. A player with no candidates chooses nothing.
    Mcts {
        candidates: CandidatesPath,
        payoffs: ScriptCall,
        estimate: ScriptCall,
        iterations: u64,
        depth: u64,
        chance: Option<ScriptCall>,
        rollout: Option<ScriptCall>,
        salt: u64,
    },
    /// Information-set Monte Carlo tree search, for a player who sees only
    /// part of the economy: `iterations` iterations each turn, as
    /// [`Mcts`](Self::Mcts) searches, but over what the player observes.
    /// `observe`, `fn(state, player)`, is what a player sees of a state: a
    /// view of the accounts it may see, `state.view([...])`, or the state
    /// itself where it sees everything. States a player sees alike are one
    /// node of the tree, and the candidate set, `fn(view)`, and `rollout`,
    /// `fn(view, candidates)`, read only that view; the candidates that apply
    /// are reconsidered in each world searched, and the one chosen is
    /// checked against the real economy. `chance`, `fn(state)`, gives
    /// Nature's weighted outcomes, `[exchange, weight]` pairs, where Nature
    /// draws, which the tree draws by weight.
    ///
    /// Each iteration searches one world the player may be in, drawn by
    /// weight from what it believes. Its beliefs replay the history of the
    /// point the game started from, from the economy that history began
    /// with, and then the game's: at each of Nature's draws, a step where
    /// `chance` gives outcomes, every world draws again every way `chance`
    /// gives there, and at every other step it takes the step as it was. A
    /// world is kept while the player sees it as it saw the real economy and
    /// Nature draws in it just where Nature draws in the real one, weighted
    /// by the draws that made it. Payoffs, estimates, seeds, salt, and a
    /// player with no candidates, are as with `Mcts`.
    Ismcts {
        candidates: CandidatesPath,
        observe: ScriptCall,
        chance: ScriptCall,
        payoffs: ScriptCall,
        estimate: ScriptCall,
        iterations: u64,
        depth: u64,
        rollout: Option<ScriptCall>,
        salt: u64,
    },
}

/// What a player observes of an economy: the view its `observe` script
/// gives, one information set with every view showing the same.
#[derive(Clone)]
struct Observed {
    observation: ObservationKey<Paths>,
    view: ScriptState,
}

impl Observed {
    fn new(view: ScriptState) -> Self {
        Self {
            observation: view.observation(),
            view,
        }
    }
}

impl PartialEq for Observed {
    fn eq(&self, other: &Self) -> bool {
        self.observation == other.observation
    }
}

impl Eq for Observed {}

impl Hash for Observed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.observation.hash(state);
    }
}

/// What an information-set player believes: the worlds it may be in, each
/// weighted by Nature's draws that made it, kept up with the real economy
/// `seen` steps into the history they replay.
struct Beliefs {
    worlds: Vec<WeightedExchange<paths::Economy>>,
    actual: paths::Economy,
    /// Whether Nature draws in the real economy there.
    drawing: bool,
    seen: usize,
}

/// What a player's choice in a game is made from.
struct Turn<'a> {
    /// Where the game started, and the exchanges it has made since.
    at: Point,
    played: &'a [paths::Exchange],
    economy: &'a paths::Economy,
    /// The player choosing, of `players`.
    player: usize,
    players: usize,
    actor: &'a ScriptCall,
    random: &'a Random,
    seed: u64,
    /// How many choices the game's players made before this one.
    choice: u64,
}

/// What the players of a game who cannot see everything believe, kept from
/// turn to turn: the history their beliefs replay, the economy it began
/// with and every exchange to the game's start, read when one first
/// chooses, and each such player's beliefs.
#[derive(Default)]
struct Believing {
    history: Option<(paths::Economy, Vec<paths::Exchange>)>,
    players: BTreeMap<usize, Beliefs>,
}

/// What a strategy is given to spend: the seed of its randomness and its
/// budget, in expansions for a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effort {
    pub seed: u64,
    pub budget: u64,
}

/// How a strategy's run ended.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// The goal holds where the run ends.
    Reached,
    /// The policy proposed nothing, or a plan ended short of the goal.
    Stopped,
    /// The policy used its every turn.
    Horizon,
    /// The policy proposed an exchange that does not apply, on turn `turn`
    /// counting from zero; the run ends before it.
    Refused {
        turn: u64,
        assessment: Box<paths::Assessment>,
    },
    /// The search explored everything the candidates reach without the goal.
    Exhausted,
    /// The search spent its budget first.
    Spent,
    /// The caller stopped the search.
    Interrupted,
}

/// A strategy's run: where it started and ended, how, and what a search
/// explored on the way.
#[derive(Debug, Clone)]
pub struct Ran {
    pub start: Point,
    /// Where the run ended: the point its exchanges reach, recorded as a run
    /// forked from `start`, or `start` itself when it applied none.
    pub end: Point,
    pub outcome: Outcome,
    /// What the search expanded and generated; none for a policy.
    pub thinking: Option<GraphSearchProgress>,
    /// The total cost of a cheapest search's exchanges.
    pub cost: Option<u64>,
}

impl Engine {
    /// Runs `strategy` from `at` with `effort`, telling `watch` of a search's
    /// progress, which may stop it. Every exchange is the kernel's to apply;
    /// what the strategy reaches is recorded as a run forked from `at`.
    pub fn run_strategy(
        &self,
        at: Point,
        strategy: &Strategy,
        effort: Effort,
        watch: &mut impl FnMut(&GraphSearchProgress) -> ControlFlow<()>,
    ) -> Result<Ran, EngineError> {
        let initial = self.reach(at)?.state.economy.clone();
        match strategy {
            Strategy::Policy {
                policy,
                goal,
                horizon,
            } => self.drive(
                at,
                initial,
                policy,
                &self.goal(goal)?,
                *horizon,
                effort.seed,
            ),
            Strategy::Plan { plan, goal } => {
                let goal = self.goal(goal)?;
                let (script, function) = (&plan.script, plan.function.as_str());
                let state = ScriptState::new(initial.clone());
                // A plan that draws chance takes the run's random as well.
                let planned: rhai::Dynamic = if self.defines(script, function, 2)? {
                    self.call(script, function, (state, Random::new(effort.seed)))?
                } else {
                    self.call(script, function, (state,))?
                };
                if planned.is_unit() {
                    return Ok(Ran {
                        start: at,
                        end: at,
                        outcome: Outcome::Stopped,
                        thinking: None,
                        cost: None,
                    });
                }
                let exchanges = crate::exchanges_of(planned.try_cast::<rhai::Array>().ok_or(
                    EngineError::Returned {
                        expected: "an array of exchanges or nothing",
                        found: "something else".to_owned(),
                    },
                )?)?;
                let (end, outcome) = match self.apply(at, &exchanges) {
                    Ok(end) => {
                        let reached = self.reach(end)?.state.economy.matches(&goal);
                        (
                            end,
                            if reached {
                                Outcome::Reached
                            } else {
                                Outcome::Stopped
                            },
                        )
                    }
                    Err(EngineError::Refused { index, assessment }) => (
                        at,
                        Outcome::Refused {
                            turn: index as u64,
                            assessment,
                        },
                    ),
                    Err(error) => return Err(error),
                };
                Ok(Ran {
                    start: at,
                    end,
                    outcome,
                    thinking: None,
                    cost: None,
                })
            }
            Strategy::BreadthFirst { candidates, goal } => {
                let candidates = self.candidate_set(candidates)?;
                let failure = RefCell::new(None);
                let propose =
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure);
                let mut session = BfsSession::new(&initial, self.goal(goal)?, propose);
                let status = search(&mut session, effort.budget, watch);
                let progress = session.progress();
                let solution = session.into_solution();
                failed(failure)?;
                self.searched(at, status, progress, solution)
            }
            Strategy::BranchAndBound {
                candidates,
                goal,
                cost,
                bound,
            } => {
                let candidates = self.candidate_set(candidates)?;
                let failure = RefCell::new(None);
                let found = branch_and_bound(
                    &initial,
                    &self.goal(goal)?,
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure),
                    |economy: &paths::Economy| self.counted(cost, economy, &failure),
                    |economy: &paths::Economy| self.counted(bound, economy, &failure),
                    usize::try_from(effort.budget).unwrap_or(usize::MAX),
                );
                failed(failure)?;
                let outcome = match (&found.solution, found.exhausted) {
                    (Some(_), _) => Outcome::Reached,
                    (None, true) => Outcome::Exhausted,
                    (None, false) => Outcome::Spent,
                };
                let (end, cost) = match found.solution {
                    Some(solution) => (
                        self.apply(at, solution.trace().exchanges())?,
                        Some(solution.cost()),
                    ),
                    None => (at, None),
                };
                Ok(Ran {
                    start: at,
                    end,
                    outcome,
                    thinking: None,
                    cost,
                })
            }
            Strategy::Beam {
                candidates,
                goal,
                score,
                width,
                depth,
                tie,
            } => {
                let candidates = self.candidate_set(candidates)?;
                let failure = RefCell::new(None);
                let found = beam(
                    &initial,
                    &self.goal(goal)?,
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure),
                    |economy: &paths::Economy| self.signed(score, economy, &failure),
                    usize::try_from(*width).unwrap_or(usize::MAX),
                    usize::try_from(*depth).unwrap_or(usize::MAX),
                    *tie,
                );
                failed(failure)?;
                let (end, outcome) = match found.best {
                    Some(best) => (self.apply(at, best.trace.exchanges())?, Outcome::Reached),
                    None => (at, Outcome::Exhausted),
                };
                Ok(Ran {
                    start: at,
                    end,
                    outcome,
                    thinking: None,
                    cost: None,
                })
            }
            Strategy::BestFirst {
                candidates,
                goal,
                cost,
                heuristic,
            } => {
                let candidates = self.candidate_set(candidates)?;
                let failure = RefCell::new(None);
                let found = best_first(
                    &initial,
                    &self.goal(goal)?,
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure),
                    |economy: &paths::Economy| self.counted(cost, economy, &failure),
                    |economy: &paths::Economy| {
                        heuristic
                            .as_ref()
                            .map_or(0, |heuristic| self.counted(heuristic, economy, &failure))
                    },
                );
                failed(failure)?;
                let (end, outcome, cost) = match found {
                    Some(solution) => (
                        self.apply(at, solution.trace().exchanges())?,
                        Outcome::Reached,
                        Some(solution.cost()),
                    ),
                    None => (at, Outcome::Exhausted, None),
                };
                Ok(Ran {
                    start: at,
                    end,
                    outcome,
                    thinking: None,
                    cost,
                })
            }
            Strategy::Pareto {
                candidates,
                goal,
                objectives,
                by,
            } => self.pareto(
                at,
                &initial,
                candidates,
                goal,
                (objectives, by),
                effort,
                watch,
            ),
            Strategy::Game {
                goal,
                horizon,
                nature,
                actor,
                players,
            } => self.play(
                at,
                initial,
                &self.goal(goal)?,
                *horizon,
                nature.as_ref(),
                actor,
                players,
                effort.seed,
            ),
            Strategy::Cheapest {
                candidates,
                goal,
                cost,
                heuristic: None,
            } => {
                let candidates = self.candidate_set(candidates)?;
                let failure = RefCell::new(None);
                let budget = effort.budget;
                let (mut expanded, mut spent) = (0, false);
                let found = dijkstra(
                    &initial,
                    &self.goal(goal)?,
                    |economy: &paths::Economy| {
                        // Past its budget the search expands nothing more.
                        if expanded >= budget {
                            spent = true;
                            return Vec::new();
                        }
                        expanded += 1;
                        self.offered(&candidates, economy, &failure)
                    },
                    |before: &paths::Economy,
                     exchange: &paths::Exchange,
                     after: &paths::Economy| {
                        self.charged(cost, (before, exchange, after), &failure)
                    },
                );
                failed(failure)?;
                // A goal found once a state went unexpanded may not be the
                // cheapest.
                let (end, outcome, cost) = match found {
                    _ if spent => (at, Outcome::Spent, None),
                    Some(solution) => (
                        self.apply(at, solution.trace().exchanges())?,
                        Outcome::Reached,
                        Some(solution.cost()),
                    ),
                    None => (at, Outcome::Exhausted, None),
                };
                Ok(Ran {
                    start: at,
                    end,
                    outcome,
                    thinking: None,
                    cost,
                })
            }
            Strategy::Cheapest {
                candidates,
                goal,
                cost,
                heuristic: Some(heuristic),
            } => {
                let candidates = self.candidate_set(candidates)?;
                let failure = RefCell::new(None);
                let propose =
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure);
                let charge = |before: &paths::Economy,
                              exchange: &paths::Exchange,
                              after: &paths::Economy| {
                    self.charged(cost, (before, exchange, after), &failure)
                };
                let estimate = |economy: &paths::Economy| {
                    self.measure_state(heuristic, economy)
                        .map(|estimate| u64::try_from(estimate).unwrap_or(u64::MAX))
                        .unwrap_or_else(|error| {
                            failure.borrow_mut().get_or_insert(error);
                            0
                        })
                };
                let mut session =
                    AStarSession::new(&initial, self.goal(goal)?, propose, charge, estimate);
                let status = search(&mut session, effort.budget, watch);
                let progress = session.progress();
                let solution = session.into_solution();
                failed(failure)?;
                self.searched(at, status, progress, solution)
            }
        }
    }

    fn candidate_set(&self, path: &CandidatesPath) -> Result<CandidateSet, EngineError> {
        self.defined(path)
    }

    /// What `cost` charges for a step, `(before, exchange, after)`, as a
    /// count, keeping the first failure in `failure` and charging nothing
    /// for a step it fails on.
    fn charged(
        &self,
        cost: &ScriptCall,
        (before, exchange, after): (&paths::Economy, &paths::Exchange, &paths::Economy),
        failure: &RefCell<Option<EngineError>>,
    ) -> u64 {
        self.step_cost(cost, before, exchange, after)
            .map(|cost| u64::try_from(cost).unwrap_or(u64::MAX))
            .unwrap_or_else(|error| {
                failure.borrow_mut().get_or_insert(error);
                0
            })
    }

    /// The exchanges `set` offers at `economy` that apply, keeping the first
    /// script failure in `failure` and offering nothing after one.
    fn offered(
        &self,
        set: &CandidateSet,
        economy: &paths::Economy,
        failure: &RefCell<Option<EngineError>>,
    ) -> Vec<paths::Exchange> {
        if failure.borrow().is_some() {
            return Vec::new();
        }
        let proposed = match set {
            CandidateSet::Listed(exchanges) => Ok(exchanges.clone()),
            CandidateSet::Script(call) => self.propose(call, economy),
        };
        match proposed {
            Ok(exchanges) => economy.applicable(exchanges),
            Err(error) => {
                failure.borrow_mut().get_or_insert(error);
                Vec::new()
            }
        }
    }

    /// What `call` measures of `economy`, as a count, keeping the first
    /// failure in `failure` and counting zero after one.
    fn counted(
        &self,
        call: &ScriptCall,
        economy: &paths::Economy,
        failure: &RefCell<Option<EngineError>>,
    ) -> u64 {
        if failure.borrow().is_some() {
            return 0;
        }
        match self.measure_state(call, economy) {
            Ok(value) => u64::try_from(value).unwrap_or(u64::MAX),
            Err(error) => {
                failure.borrow_mut().get_or_insert(error);
                0
            }
        }
    }

    /// The integer `call` returns for `economy`, which may be negative, with
    /// failures kept as [`counted`](Self::counted) keeps them.
    fn signed(
        &self,
        call: &ScriptCall,
        economy: &paths::Economy,
        failure: &RefCell<Option<EngineError>>,
    ) -> i64 {
        if failure.borrow().is_some() {
            return 0;
        }
        match self.call::<i64>(
            &call.script,
            call.function.as_str(),
            (ScriptState::new(economy.clone()),),
        ) {
            Ok(value) => value,
            Err(error) => {
                failure.borrow_mut().get_or_insert(error);
                0
            }
        }
    }

    /// Every player's payoffs `call` gives for `economy`: none where it
    /// returns nothing.
    fn payoffs(
        &self,
        call: &ScriptCall,
        economy: &paths::Economy,
        failure: &RefCell<Option<EngineError>>,
    ) -> Option<Vec<f64>> {
        if failure.borrow().is_some() {
            return None;
        }
        let returned: Result<rhai::Dynamic, _> = self.call(
            &call.script,
            call.function.as_str(),
            (ScriptState::new(economy.clone()),),
        );
        let numbers = returned.and_then(|returned| {
            if returned.is_unit() {
                return Ok(None);
            }
            let found = returned.type_name();
            let values: rhai::Array = returned.try_cast().ok_or_else(|| EngineError::Returned {
                expected: "an array of payoffs or nothing",
                found: found.to_owned(),
            })?;
            values
                .into_iter()
                .map(|value| {
                    let found = value.type_name();
                    value
                        .as_float()
                        .or_else(|_| value.as_int().map(|value| value as f64))
                        .map_err(|_| EngineError::Returned {
                            expected: "a payoff",
                            found: found.to_owned(),
                        })
                })
                .collect::<Result<Vec<f64>, _>>()
                .map(Some)
        });
        numbers.unwrap_or_else(|error| {
            failure.borrow_mut().get_or_insert(error);
            None
        })
    }

    /// Nature's weighted outcomes `call` gives for `economy`: none where it
    /// returns nothing.
    fn outcomes(
        &self,
        call: &ScriptCall,
        economy: &paths::Economy,
    ) -> Result<Vec<WeightedExchange<paths::Exchange>>, EngineError> {
        let returned: rhai::Dynamic = self.call(
            &call.script,
            call.function.as_str(),
            (ScriptState::new(economy.clone()),),
        )?;
        if returned.is_unit() {
            return Ok(Vec::new());
        }
        let found = returned.type_name();
        crate::script::weighted_of(returned.try_cast().ok_or_else(|| EngineError::Returned {
            expected: "weighted outcomes or nothing",
            found: found.to_owned(),
        })?)
    }

    /// The [`outcomes`](Self::outcomes) `call` gives for `economy`, keeping
    /// the first failure in `failure` and giving none after one.
    fn weighted(
        &self,
        call: &ScriptCall,
        economy: &paths::Economy,
        failure: &RefCell<Option<EngineError>>,
    ) -> Vec<WeightedExchange<paths::Exchange>> {
        if failure.borrow().is_some() {
            return Vec::new();
        }
        self.outcomes(call, economy).unwrap_or_else(|error| {
            failure.borrow_mut().get_or_insert(error);
            Vec::new()
        })
    }

    /// What `player` observes of `economy`, by `observe`, `fn(state,
    /// player)`.
    fn seen_by(
        &self,
        observe: &ScriptCall,
        economy: &paths::Economy,
        player: usize,
    ) -> Result<Observed, EngineError> {
        let view: ScriptState = self.call(
            &observe.script,
            observe.function.as_str(),
            (
                ScriptState::new(economy.clone()),
                i64::try_from(player).unwrap_or(i64::MAX),
            ),
        )?;
        Ok(Observed::new(view))
    }

    /// What [`seen_by`](Self::seen_by) gives, keeping the first failure in
    /// `failure` and giving the whole economy after one.
    fn observed(
        &self,
        observe: &ScriptCall,
        economy: &paths::Economy,
        player: usize,
        failure: &RefCell<Option<EngineError>>,
    ) -> Observed {
        if failure.borrow().is_none() {
            match self.seen_by(observe, economy, player) {
                Ok(seen) => return seen,
                Err(error) => {
                    failure.borrow_mut().get_or_insert(error);
                }
            }
        }
        Observed::new(ScriptState::new(economy.clone()))
    }

    /// The exchanges `set` proposes to a player seeing `seen`, whether or not
    /// they apply, keeping the first failure in `failure` and proposing
    /// nothing after one.
    fn proposed(
        &self,
        set: &CandidateSet,
        seen: &Observed,
        failure: &RefCell<Option<EngineError>>,
    ) -> Vec<paths::Exchange> {
        if failure.borrow().is_some() {
            return Vec::new();
        }
        let proposed = match set {
            CandidateSet::Listed(exchanges) => Ok(exchanges.clone()),
            CandidateSet::Script(call) => self
                .call(&call.script, call.function.as_str(), (seen.view.clone(),))
                .and_then(crate::exchanges_of),
        };
        proposed.unwrap_or_else(|error| {
            failure.borrow_mut().get_or_insert(error);
            Vec::new()
        })
    }

    /// Brings what `player` believes up to the end of the game's history,
    /// `played` after the history of the point it started from: a step at a
    /// time, each world draws again every way `chance` gives where Nature
    /// drew in the real economy, and otherwise takes the step as it was;
    /// a world is kept while `player` sees it, by `observe`, as it saw the
    /// real economy, and Nature draws in it where it draws in the real one.
    fn believed<'b>(
        &self,
        believing: &'b mut Believing,
        (at, played): (Point, &[paths::Exchange]),
        player: usize,
        (observe, chance): (&ScriptCall, &ScriptCall),
    ) -> Result<&'b [WeightedExchange<paths::Economy>], EngineError> {
        let history = match believing.history.take() {
            Some(history) => history,
            None => self.history(at)?,
        };
        let (root, before) = believing.history.insert(history);
        let beliefs = match believing.players.entry(player) {
            Entry::Occupied(beliefs) => beliefs.into_mut(),
            Entry::Vacant(vacant) => vacant.insert(Beliefs {
                worlds: vec![WeightedExchange::new(root.clone(), 1)],
                actual: root.clone(),
                drawing: !self.outcomes(chance, root)?.is_empty(),
                seen: 0,
            }),
        };
        for exchange in before.iter().chain(played).skip(beliefs.seen) {
            let mut next = Vec::new();
            for believed in std::mem::take(&mut beliefs.worlds) {
                let weight = believed.weight();
                if !beliefs.drawing {
                    let mut world = believed.into_exchange();
                    if world.apply(exchange.clone()).is_ok() {
                        next.push(WeightedExchange::new(world, weight));
                    }
                    continue;
                }
                for outcome in self.outcomes(chance, believed.exchange())? {
                    let weight = weight.checked_mul(outcome.weight()).ok_or_else(|| {
                        EngineError::Undecided("a belief's weight overflowed".to_owned())
                    })?;
                    let mut world = believed.exchange().clone();
                    if weight > 0 && world.apply(outcome.into_exchange()).is_ok() {
                        next.push(WeightedExchange::new(world, weight));
                    }
                }
            }
            beliefs
                .actual
                .apply(exchange.clone())
                .map_err(|_| EngineError::Refused {
                    index: beliefs.seen,
                    assessment: Box::new(beliefs.actual.assess(exchange)),
                })?;
            beliefs.drawing = !self.outcomes(chance, &beliefs.actual)?.is_empty();
            let seen = self.seen_by(observe, &beliefs.actual, player)?;
            for believed in next {
                if self.seen_by(observe, believed.exchange(), player)? == seen
                    && self.outcomes(chance, believed.exchange())?.is_empty() != beliefs.drawing
                {
                    beliefs.worlds.push(believed);
                }
            }
            beliefs.seen += 1;
        }
        Ok(&beliefs.worlds)
    }

    /// The one of `actions` that `call`, `fn(state, candidates)`, picks at
    /// `state`, or none.
    fn rolled(
        &self,
        call: &ScriptCall,
        state: ScriptState,
        actions: &[paths::Exchange],
        failure: &RefCell<Option<EngineError>>,
    ) -> Option<paths::Exchange> {
        if failure.borrow().is_some() {
            return None;
        }
        let candidates: rhai::Array = actions.iter().cloned().map(rhai::Dynamic::from).collect();
        let picked = self
            .call(&call.script, call.function.as_str(), (state, candidates))
            .and_then(proposal)
            .and_then(|picked| match picked {
                Some(exchange) if !actions.contains(&exchange) => Err(EngineError::Returned {
                    expected: "one of the candidates or nothing",
                    found: exchange.rate().to_string(),
                }),
                picked => Ok(picked),
            });
        picked.unwrap_or_else(|error| {
            failure.borrow_mut().get_or_insert(error);
            None
        })
    }

    #[expect(clippy::too_many_arguments, reason = "a game's parts, each read once")]
    fn play(
        &self,
        at: Point,
        mut economy: paths::Economy,
        goal: &paths::Goal,
        horizon: u64,
        nature: Option<&ScriptCall>,
        actor: &ScriptCall,
        players: &[Chooser],
        seed: u64,
    ) -> Result<Ran, EngineError> {
        let chance = Random::new(seed);
        let random = Random::new(sample_seed(seed, 1));
        let mut exchanges = Vec::new();
        // The turns a player chose on so far.
        let mut choices = 0;
        let mut believing = Believing::default();
        let outcome = loop {
            if economy.matches(goal) {
                break Outcome::Reached;
            }
            let turn = exchanges.len() as u64;
            if turn >= horizon {
                break Outcome::Horizon;
            }
            let responded = match nature {
                Some(nature) => {
                    let proposed: rhai::Dynamic = self.call(
                        &nature.script,
                        nature.function.as_str(),
                        (ScriptState::new(economy.clone()), chance.clone()),
                    )?;
                    proposal(proposed)?
                }
                None => None,
            };
            let chosen = match responded {
                Some(exchange) => Some(exchange),
                None => {
                    let index: i64 = self.call(
                        &actor.script,
                        actor.function.as_str(),
                        (ScriptState::new(economy.clone()),),
                    )?;
                    let (player, chooser) = usize::try_from(index)
                        .ok()
                        .and_then(|index| Some((index, players.get(index)?)))
                        .ok_or_else(|| EngineError::Returned {
                            expected: "the index of a player",
                            found: index.to_string(),
                        })?;
                    let turn = Turn {
                        at,
                        played: &exchanges,
                        economy: &economy,
                        player,
                        players: players.len(),
                        actor,
                        random: &random,
                        seed,
                        choice: choices,
                    };
                    let chosen = self.choose(chooser, turn, &mut believing)?;
                    choices += 1;
                    chosen
                }
            };
            let Some(exchange) = chosen else {
                break Outcome::Stopped;
            };
            if economy.apply(exchange.clone()).is_err() {
                break Outcome::Refused {
                    turn,
                    assessment: Box::new(economy.assess(&exchange)),
                };
            }
            exchanges.push(exchange);
        };
        Ok(Ran {
            start: at,
            end: self.apply(at, &exchanges)?,
            outcome,
            thinking: None,
            cost: None,
        })
    }

    /// What `chooser` chooses on `turn`, with what the game's
    /// information-set players believe in `believing`.
    fn choose(
        &self,
        chooser: &Chooser,
        turn: Turn<'_>,
        believing: &mut Believing,
    ) -> Result<Option<paths::Exchange>, EngineError> {
        let Turn {
            economy,
            random,
            players,
            actor,
            seed,
            choice,
            ..
        } = turn;
        let failure = RefCell::new(None);
        let player = |economy: &paths::Economy| {
            if failure.borrow().is_some() {
                return 0;
            }
            self.call::<i64>(
                &actor.script,
                actor.function.as_str(),
                (ScriptState::new(economy.clone()),),
            )
            .map(|index| usize::try_from(index).unwrap_or(usize::MAX))
            .unwrap_or_else(|error| {
                failure.borrow_mut().get_or_insert(error);
                0
            })
        };
        let chosen = match chooser {
            Chooser::Policy(policy) => {
                let proposed: rhai::Dynamic = self.call(
                    &policy.script,
                    policy.function.as_str(),
                    (ScriptState::new(economy.clone()), random.clone()),
                )?;
                proposal(proposed)?
            }
            Chooser::Minimax {
                candidates,
                evaluate,
                depth,
            } => {
                let candidates = self.candidate_set(candidates)?;
                minimax(
                    economy,
                    usize::try_from(*depth).unwrap_or(usize::MAX),
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure),
                    player,
                    |economy: &paths::Economy| self.signed(evaluate, economy, &failure),
                )
                .map(|chosen| chosen.exchange)
            }
            Chooser::Mcts {
                candidates,
                payoffs,
                estimate,
                iterations,
                depth,
                chance,
                rollout,
                salt,
            } => {
                let candidates = self.candidate_set(candidates)?;
                if self.offered(&candidates, economy, &failure).is_empty() {
                    return match failure.borrow_mut().take() {
                        Some(error) => Err(error),
                        None => Ok(None),
                    };
                }
                let config = MctsConfig::new(
                    usize::try_from(*iterations).unwrap_or(usize::MAX),
                    usize::try_from(*depth).unwrap_or(usize::MAX),
                )
                .with_seed((seed ^ salt).wrapping_add(choice));
                let decided = mcts(
                    economy,
                    config,
                    players,
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure),
                    |economy: &paths::Economy| match chance {
                        Some(chance) => self.weighted(chance, economy, &failure),
                        None => Vec::new(),
                    },
                    player,
                    |economy: &paths::Economy| self.payoffs(payoffs, economy, &failure),
                    |economy: &paths::Economy| {
                        self.payoffs(estimate, economy, &failure)
                            .unwrap_or_else(|| vec![0.0; players])
                    },
                    |economy: &paths::Economy,
                     actions: &[paths::Exchange],
                     sampler: &mut SeededSampler| match rollout {
                        Some(rollout) => self.rolled(
                            rollout,
                            ScriptState::new(economy.clone()),
                            actions,
                            &failure,
                        ),
                        None => uniformly(actions, sampler),
                    },
                );
                // A script that failed explains the search better than the
                // search's own complaint.
                if let Some(error) = failure.borrow_mut().take() {
                    return Err(error);
                }
                match decided {
                    Ok(decision) => Some(decision.action().clone()),
                    Err(error) => return Err(EngineError::Undecided(format!("{error:?}"))),
                }
            }
            Chooser::Ismcts {
                candidates,
                observe,
                chance,
                payoffs,
                estimate,
                iterations,
                depth,
                rollout,
                salt,
            } => {
                let candidates = self.candidate_set(candidates)?;
                let information = |economy: &paths::Economy| {
                    let player = player(economy);
                    InformationState::new(player, self.observed(observe, economy, player, &failure))
                };
                let root = information(economy);
                if self.proposed(&candidates, root.key(), &failure).is_empty() {
                    return match failure.borrow_mut().take() {
                        Some(error) => Err(error),
                        None => Ok(None),
                    };
                }
                // The worlds the player may be in: those where it is the one
                // choosing, and sees what it sees in the real economy.
                let worlds = self
                    .believed(
                        believing,
                        (turn.at, turn.played),
                        turn.player,
                        (observe, chance),
                    )?
                    .iter()
                    .filter(|believed| information(believed.exchange()) == root)
                    .map(|believed| WeightedExchange::new(believed.exchange(), believed.weight()))
                    .collect::<Vec<_>>();
                let config = MctsConfig::new(
                    usize::try_from(*iterations).unwrap_or(usize::MAX),
                    usize::try_from(*depth).unwrap_or(usize::MAX),
                )
                .with_seed((seed ^ salt).wrapping_add(choice));
                let decided = ismcts(
                    economy,
                    config,
                    players,
                    &information,
                    |_: &InformationState<Observed>, sampler: &mut SeededSampler| {
                        sample(&worlds, sampler).ok().map(|world| (*world).clone())
                    },
                    eager_actions(|information: &InformationState<Observed>| {
                        self.proposed(&candidates, information.key(), &failure)
                    }),
                    |economy: &paths::Economy| self.weighted(chance, economy, &failure),
                    |economy: &paths::Economy| self.payoffs(payoffs, economy, &failure),
                    |economy: &paths::Economy| {
                        self.payoffs(estimate, economy, &failure)
                            .unwrap_or_else(|| vec![0.0; players])
                    },
                    |information: &InformationState<Observed>,
                     actions: &[paths::Exchange],
                     sampler: &mut SeededSampler| match rollout {
                        Some(rollout) => {
                            self.rolled(rollout, information.key().view.clone(), actions, &failure)
                        }
                        None => uniformly(actions, sampler),
                    },
                );
                if let Some(error) = failure.borrow_mut().take() {
                    return Err(error);
                }
                match decided {
                    Ok(decision) => Some(decision.action().clone()),
                    Err(error) => return Err(EngineError::Undecided(format!("{error:?}"))),
                }
            }
        };
        failed(failure)?;
        Ok(chosen)
    }

    fn drive(
        &self,
        at: Point,
        mut economy: paths::Economy,
        policy: &ScriptCall,
        goal: &paths::Goal,
        horizon: u64,
        seed: u64,
    ) -> Result<Ran, EngineError> {
        let random = Random::new(seed);
        let mut exchanges = Vec::new();
        let outcome = loop {
            if economy.matches(goal) {
                break Outcome::Reached;
            }
            let turn = exchanges.len() as u64;
            if turn >= horizon {
                break Outcome::Horizon;
            }
            let proposed: rhai::Dynamic = self.call(
                &policy.script,
                policy.function.as_str(),
                (ScriptState::new(economy.clone()), random.clone()),
            )?;
            if proposed.is_unit() {
                break Outcome::Stopped;
            }
            let found = proposed.type_name();
            let exchange =
                proposed
                    .try_cast::<paths::Exchange>()
                    .ok_or_else(|| EngineError::Returned {
                        expected: "an exchange or nothing",
                        found: found.to_owned(),
                    })?;
            if economy.apply(exchange.clone()).is_err() {
                break Outcome::Refused {
                    turn,
                    assessment: Box::new(economy.assess(&exchange)),
                };
            }
            exchanges.push(exchange);
        };
        Ok(Ran {
            start: at,
            end: self.apply(at, &exchanges)?,
            outcome,
            thinking: None,
            cost: None,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a search's parts, each read once"
    )]
    fn pareto(
        &self,
        at: Point,
        initial: &paths::Economy,
        candidates: &CandidatesPath,
        goal: &GoalPath,
        (objectives, by): (&[ObjectivePath], &[ObjectivePath]),
        effort: Effort,
        watch: &mut impl FnMut(&GraphSearchProgress) -> ControlFlow<()>,
    ) -> Result<Ran, EngineError> {
        // Where each objective ranking the front is among those measured.
        let ranking = by
            .iter()
            .map(|objective| {
                objectives
                    .iter()
                    .position(|measured| measured == objective)
                    .ok_or_else(|| {
                        EngineError::Undefined(crate::definitions::definition(objective))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let measures = objectives
            .iter()
            .map(|path| match self.objective(path)? {
                Objective {
                    measure: Measure::State(call),
                    direction,
                } => Ok((call, direction)),
                Objective {
                    measure: Measure::Steps(_),
                    ..
                } => Err(EngineError::NeedsHistory(path.clone())),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let candidates = self.candidate_set(candidates)?;
        let failure = RefCell::new(None);
        let propose = |economy: &paths::Economy| self.offered(&candidates, economy, &failure);
        let project = |economy: &paths::Economy| {
            let values = measures
                .iter()
                .enumerate()
                .map(|(index, (call, direction))| {
                    let value = self.measure_state(call, economy).unwrap_or_else(|error| {
                        failure.borrow_mut().get_or_insert(error);
                        Quantity::ZERO
                    });
                    match direction {
                        ObjectiveDirection::Minimize => Measured::minimize(index, value),
                        ObjectiveDirection::Maximize => Measured::maximize(index, value),
                    }
                });
            ObjectiveVector::try_new(values).expect("objectives are numbered apart")
        };
        let mut session = ParetoSearchSession::new(initial, self.goal(goal)?, propose, project);
        let mut left = usize::try_from(effort.budget).unwrap_or(usize::MAX);
        let status = loop {
            if session.status().is_terminal() || left == 0 {
                break session.status();
            }
            let report = session
                .advance(WorkBudget::new(left.min(CHUNK)), &mut |progress: &_| {
                    watch(&GraphSearchProgress::from(*progress))
                })
                .map_err(|error| EngineError::Undecided(format!("{error:?}")))?;
            if report.status() == SearchStatus::Interrupted {
                break SearchStatus::Interrupted;
            }
            left = left.saturating_sub(report.work_completed().max(1));
        };
        let progress = GraphSearchProgress::from(session.progress());
        if let Some(error) = failure.borrow_mut().take() {
            return Err(error);
        }
        // The best point by the objectives in order, each in its direction.
        let best = (status == SearchStatus::Exhausted)
            .then(|| {
                session
                    .front()
                    .entries()
                    .iter()
                    .min_by(|left, right| {
                        let (left, right) = (
                            left.objectives().objectives(),
                            right.objectives().objectives(),
                        );
                        ranking
                            .iter()
                            .map(|&index| {
                                let (left, right) = (&left[index], &right[index]);
                                match left.direction() {
                                    ObjectiveDirection::Minimize => left.value().cmp(right.value()),
                                    ObjectiveDirection::Maximize => right.value().cmp(left.value()),
                                }
                            })
                            .find(|order| order.is_ne())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|entry| entry.payload().clone())
            })
            .flatten();
        let (end, outcome) = match (status, best) {
            (_, Some(trace)) => (self.apply(at, trace.exchanges())?, Outcome::Reached),
            (SearchStatus::Exhausted, None) => (at, Outcome::Exhausted),
            (SearchStatus::Interrupted, None) => (at, Outcome::Interrupted),
            _ => (at, Outcome::Spent),
        };
        Ok(Ran {
            start: at,
            end,
            outcome,
            thinking: Some(progress),
            cost: None,
        })
    }

    fn searched(
        &self,
        at: Point,
        status: SearchStatus,
        progress: GraphSearchProgress,
        solution: Option<axionomy_search::SearchSolution<axionomy::paths::Paths>>,
    ) -> Result<Ran, EngineError> {
        let (end, outcome, cost) = match (status, solution) {
            (_, Some(solution)) => (
                self.apply(at, solution.trace().exchanges())?,
                Outcome::Reached,
                Some(solution.cost()),
            ),
            (SearchStatus::Exhausted, None) => (at, Outcome::Exhausted, None),
            (SearchStatus::Interrupted, None) => (at, Outcome::Interrupted, None),
            (SearchStatus::Running | SearchStatus::Solved, None) => (at, Outcome::Spent, None),
        };
        Ok(Ran {
            start: at,
            end,
            outcome,
            thinking: Some(progress),
            cost,
        })
    }
}

/// The resumable search sessions a strategy advances.
trait Session {
    fn status(&self) -> SearchStatus;
    fn advance(
        &mut self,
        units: usize,
        watch: &mut dyn FnMut(&GraphSearchProgress) -> ControlFlow<()>,
    ) -> (SearchStatus, usize);
}

impl<C> Session for BfsSession<paths::Paths, C>
where
    C: FnMut(&paths::Economy) -> Vec<paths::Exchange>,
{
    fn status(&self) -> SearchStatus {
        BfsSession::status(self)
    }

    fn advance(
        &mut self,
        units: usize,
        watch: &mut dyn FnMut(&GraphSearchProgress) -> ControlFlow<()>,
    ) -> (SearchStatus, usize) {
        let report = BfsSession::advance(self, WorkBudget::new(units), &mut |progress: &_| {
            watch(progress)
        });
        (report.status(), report.work_completed())
    }
}

impl<C, S, H> Session for AStarSession<paths::Paths, C, S, H>
where
    C: FnMut(&paths::Economy) -> Vec<paths::Exchange>,
    S: FnMut(&paths::Economy, &paths::Exchange, &paths::Economy) -> u64,
    H: FnMut(&paths::Economy) -> u64,
{
    fn status(&self) -> SearchStatus {
        AStarSession::status(self)
    }

    fn advance(
        &mut self,
        units: usize,
        watch: &mut dyn FnMut(&GraphSearchProgress) -> ControlFlow<()>,
    ) -> (SearchStatus, usize) {
        let report = AStarSession::advance(self, WorkBudget::new(units), &mut |progress: &_| {
            watch(progress)
        });
        (report.status(), report.work_completed())
    }
}

/// Advances `session` until it ends or has spent `budget` expansions.
fn search(
    session: &mut impl Session,
    budget: u64,
    watch: &mut impl FnMut(&GraphSearchProgress) -> ControlFlow<()>,
) -> SearchStatus {
    let mut left = usize::try_from(budget).unwrap_or(usize::MAX);
    loop {
        let status = session.status();
        if status.is_terminal() || left == 0 {
            return status;
        }
        let (status, completed) = session.advance(left.min(CHUNK), watch);
        if status.is_terminal() || status == SearchStatus::Interrupted {
            return status;
        }
        left = left.saturating_sub(completed.max(1));
    }
}

/// One of `actions` drawn uniformly with `sampler`, as tree search's
/// simulations choose without a rollout script.
fn uniformly(actions: &[paths::Exchange], sampler: &mut SeededSampler) -> Option<paths::Exchange> {
    let ticket = sampler.ticket(actions.len() as u64);
    actions.get(usize::try_from(ticket).ok()?).cloned()
}

fn failed(failure: RefCell<Option<EngineError>>) -> Result<(), EngineError> {
    match failure.into_inner() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// What a script proposed: an exchange, or nothing.
fn proposal(proposed: rhai::Dynamic) -> Result<Option<paths::Exchange>, EngineError> {
    if proposed.is_unit() {
        return Ok(None);
    }
    let found = proposed.type_name();
    proposed
        .try_cast::<paths::Exchange>()
        .map(Some)
        .ok_or_else(|| EngineError::Returned {
            expected: "an exchange or nothing",
            found: found.to_owned(),
        })
}
