//! Strategies: a built-in algorithm of `axionomy-search`, the definitions it
//! calls and its parameters (D-050). The engine runs one from a point and
//! records the exchanges it reaches as a run forked from that point.

use crate::{
    CandidatesPath, Engine, EngineError, GoalPath, Random, ScriptCall, ScriptState,
    definitions::CandidateSet,
};
use axionomy::paths;
use axionomy_search::{
    AStarSession, BfsSession, GraphSearchProgress,
    session::{SearchStatus, WorkBudget},
};
use axionomy_store::Point;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, ops::ControlFlow};

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
    /// The fewest exchanges from the candidate set that reach the goal.
    BreadthFirst {
        candidates: CandidatesPath,
        goal: GoalPath,
    },
    /// The cheapest exchanges from the candidate set that reach the goal,
    /// each costing what `cost` charges, `fn(before, exchange, after)`: in
    /// Dijkstra's order, or A*'s when `heuristic`, `fn(state)`, bounds the
    /// cost still to pay from below.
    Cheapest {
        candidates: CandidatesPath,
        goal: GoalPath,
        cost: ScriptCall,
        heuristic: Option<ScriptCall>,
    },
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
    /// The policy proposed nothing.
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
            Strategy::Cheapest {
                candidates,
                goal,
                cost,
                heuristic,
            } => {
                let candidates = self.candidate_set(candidates)?;
                let failure = RefCell::new(None);
                let propose =
                    |economy: &paths::Economy| self.offered(&candidates, economy, &failure);
                let charge = |before: &paths::Economy,
                              exchange: &paths::Exchange,
                              after: &paths::Economy| {
                    self.step_cost(cost, before, exchange, after)
                        .map(|cost| u64::try_from(cost).unwrap_or(u64::MAX))
                        .unwrap_or_else(|error| {
                            failure.borrow_mut().get_or_insert(error);
                            0
                        })
                };
                let estimate = |economy: &paths::Economy| match heuristic {
                    None => 0,
                    Some(heuristic) => self
                        .measure_state(heuristic, economy)
                        .map(|estimate| u64::try_from(estimate).unwrap_or(u64::MAX))
                        .unwrap_or_else(|error| {
                            failure.borrow_mut().get_or_insert(error);
                            0
                        }),
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

fn failed(failure: RefCell<Option<EngineError>>) -> Result<(), EngineError> {
    match failure.into_inner() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
