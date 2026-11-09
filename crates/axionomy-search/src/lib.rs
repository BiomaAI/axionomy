#![doc = include_str!("../README.md")]

pub mod action_source;
pub mod ismcts;
pub mod mcts;
pub mod monte_carlo;
pub mod pareto;
pub mod rl;
pub mod rollout;
pub mod sampling;
pub mod session;
pub mod tree;

use axionomy::{Economy, Exchange, Goal, StateKey, Trace};
use pathfinding::prelude::{astar as pathfinding_astar, dijkstra as pathfinding_dijkstra};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};

use crate::session::{AdvanceReport, Continue, SearchObserver, SearchStatus, WorkBudget};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "RateId: Serialize, Role: Serialize + Ord, AccountId: Serialize",
    deserialize = "RateId: Deserialize<'de>, Role: Deserialize<'de> + Ord, AccountId: Deserialize<'de>"
))]
pub struct SearchSolution<RateId, Role, AccountId> {
    trace: Trace<RateId, Role, AccountId>,
    cost: u64,
    expanded: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphSearchProgress {
    expanded: usize,
    generated: usize,
    frontier: usize,
    visited: usize,
    solution_depth: Option<u64>,
}

impl GraphSearchProgress {
    pub const fn expanded(self) -> usize {
        self.expanded
    }

    pub const fn generated(self) -> usize {
        self.generated
    }

    pub const fn frontier(self) -> usize {
        self.frontier
    }

    pub const fn visited(self) -> usize {
        self.visited
    }

    pub const fn solution_depth(self) -> Option<u64> {
        self.solution_depth
    }
}

struct BfsState<AccountId, A, RateId, Role> {
    world: Economy<AccountId, A, RateId, Role>,
    trace: Trace<RateId, Role, AccountId>,
}

/// Resumable breadth-first search over implicit, core-validated successors.
pub struct BfsSession<AccountId, A, RateId, Role, Candidates> {
    goal: Goal<AccountId, A>,
    candidates: Candidates,
    states: Vec<BfsState<AccountId, A, RateId, Role>>,
    frontier: VecDeque<usize>,
    visited: HashSet<StateKey<AccountId, A>>,
    expanded: usize,
    generated: usize,
    solution: Option<SearchSolution<RateId, Role, AccountId>>,
    exhausted: bool,
}

struct AStarState<AccountId, A, RateId, Role> {
    world: Economy<AccountId, A, RateId, Role>,
    trace: Trace<RateId, Role, AccountId>,
}

/// Resumable A* search over implicit, core-validated successors.
///
/// The step cost and heuristic remain disposable solver policy. Only exchanges
/// accepted by the economy enter the frontier.
pub struct AStarSession<AccountId, A, RateId, Role, Candidates, StepCost, Heuristic> {
    goal: Goal<AccountId, A>,
    candidates: Candidates,
    step_cost: StepCost,
    heuristic: Heuristic,
    states: Vec<AStarState<AccountId, A, RateId, Role>>,
    frontier: BinaryHeap<QueueEntry>,
    best: HashMap<StateKey<AccountId, A>, u64>,
    expanded: usize,
    generated: usize,
    sequence: u64,
    solution: Option<SearchSolution<RateId, Role, AccountId>>,
    exhausted: bool,
}

impl<AccountId, A, RateId, Role, Candidates, StepCost, Heuristic>
    AStarSession<AccountId, A, RateId, Role, Candidates, StepCost, Heuristic>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
    StepCost: FnMut(
        &Economy<AccountId, A, RateId, Role>,
        &Exchange<RateId, Role, AccountId>,
        &Economy<AccountId, A, RateId, Role>,
    ) -> u64,
    Heuristic: FnMut(&Economy<AccountId, A, RateId, Role>) -> u64,
{
    pub fn new(
        initial: &Economy<AccountId, A, RateId, Role>,
        goal: Goal<AccountId, A>,
        candidates: Candidates,
        step_cost: StepCost,
        mut heuristic: Heuristic,
    ) -> Self {
        let initial = initial.clone();
        let key = initial.state_key();
        let solved = initial.matches(&goal).then(|| SearchSolution {
            trace: Trace::new(),
            cost: 0,
            expanded: 0,
        });
        let priority = heuristic(&initial);
        Self {
            goal,
            candidates,
            step_cost,
            heuristic,
            states: vec![AStarState {
                world: initial,
                trace: Trace::new(),
            }],
            frontier: BinaryHeap::from([QueueEntry {
                priority,
                cost: 0,
                sequence: 0,
                index: 0,
            }]),
            best: HashMap::from([(key, 0)]),
            expanded: 0,
            generated: 0,
            sequence: 1,
            solution: solved,
            exhausted: false,
        }
    }

    pub fn progress(&self) -> GraphSearchProgress {
        GraphSearchProgress {
            expanded: self.expanded,
            generated: self.generated,
            frontier: self.frontier.len(),
            visited: self.best.len(),
            solution_depth: self.solution.as_ref().map(SearchSolution::cost),
        }
    }

    pub fn status(&self) -> SearchStatus {
        if self.solution.is_some() {
            SearchStatus::Solved
        } else if self.exhausted {
            SearchStatus::Exhausted
        } else {
            SearchStatus::Running
        }
    }

    pub fn solution(&self) -> Option<&SearchSolution<RateId, Role, AccountId>> {
        self.solution.as_ref()
    }

    pub fn into_solution(self) -> Option<SearchSolution<RateId, Role, AccountId>> {
        self.solution
    }

    pub fn advance(
        &mut self,
        budget: WorkBudget,
        observer: &mut impl SearchObserver<GraphSearchProgress>,
    ) -> AdvanceReport<GraphSearchProgress> {
        if self.status().is_terminal() {
            return AdvanceReport::new(self.status(), 0, self.progress());
        }

        let mut completed = 0;
        while completed < budget.units() {
            if observer.observe(&self.progress()).is_break() {
                return AdvanceReport::new(SearchStatus::Interrupted, completed, self.progress());
            }

            let Some(entry) = self.frontier.pop() else {
                self.exhausted = true;
                return AdvanceReport::new(SearchStatus::Exhausted, completed, self.progress());
            };
            let key = self.states[entry.index].world.state_key();
            if self
                .best
                .get(&key)
                .is_some_and(|known| *known != entry.cost)
            {
                continue;
            }
            if self.states[entry.index].world.matches(&self.goal) {
                self.solution = Some(SearchSolution {
                    trace: self.states[entry.index].trace.clone(),
                    cost: entry.cost,
                    expanded: self.expanded,
                });
                return AdvanceReport::new(SearchStatus::Solved, completed, self.progress());
            }

            self.expanded += 1;
            completed += 1;
            let exchanges = (self.candidates)(&self.states[entry.index].world);
            for exchange in exchanges {
                let mut next = self.states[entry.index].world.clone();
                if next.apply(exchange.clone()).is_err() {
                    continue;
                }
                let step = (self.step_cost)(&self.states[entry.index].world, &exchange, &next);
                let next_cost = entry.cost.saturating_add(step);
                let key = next.state_key();
                if self.best.get(&key).is_some_and(|known| *known <= next_cost) {
                    continue;
                }
                self.best.insert(key, next_cost);
                self.generated += 1;
                let mut trace = self.states[entry.index].trace.clone();
                trace.push(exchange);
                let priority = next_cost.saturating_add((self.heuristic)(&next));
                let index = self.states.len();
                self.states.push(AStarState { world: next, trace });
                self.frontier.push(QueueEntry {
                    priority,
                    cost: next_cost,
                    sequence: self.sequence,
                    index,
                });
                self.sequence += 1;
            }
        }

        if self.frontier.is_empty() {
            self.exhausted = true;
        }
        AdvanceReport::new(self.status(), completed, self.progress())
    }
}

impl<AccountId, A, RateId, Role, Candidates> BfsSession<AccountId, A, RateId, Role, Candidates>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
{
    pub fn new(
        initial: &Economy<AccountId, A, RateId, Role>,
        goal: Goal<AccountId, A>,
        candidates: Candidates,
    ) -> Self {
        let initial = initial.clone();
        let key = initial.state_key();
        let solved = initial.matches(&goal).then(|| SearchSolution {
            trace: Trace::new(),
            cost: 0,
            expanded: 0,
        });
        Self {
            goal,
            candidates,
            states: vec![BfsState {
                world: initial,
                trace: Trace::new(),
            }],
            frontier: VecDeque::from([0]),
            visited: HashSet::from([key]),
            expanded: 0,
            generated: 0,
            solution: solved,
            exhausted: false,
        }
    }

    pub fn progress(&self) -> GraphSearchProgress {
        GraphSearchProgress {
            expanded: self.expanded,
            generated: self.generated,
            frontier: self.frontier.len(),
            visited: self.visited.len(),
            solution_depth: self.solution.as_ref().map(SearchSolution::cost),
        }
    }

    pub fn status(&self) -> SearchStatus {
        if self.solution.is_some() {
            SearchStatus::Solved
        } else if self.exhausted {
            SearchStatus::Exhausted
        } else {
            SearchStatus::Running
        }
    }

    pub fn solution(&self) -> Option<&SearchSolution<RateId, Role, AccountId>> {
        self.solution.as_ref()
    }

    pub fn into_solution(self) -> Option<SearchSolution<RateId, Role, AccountId>> {
        self.solution
    }

    pub fn advance(
        &mut self,
        budget: WorkBudget,
        observer: &mut impl SearchObserver<GraphSearchProgress>,
    ) -> AdvanceReport<GraphSearchProgress> {
        if self.status().is_terminal() {
            return AdvanceReport::new(self.status(), 0, self.progress());
        }

        let mut completed = 0;
        while completed < budget.units() {
            if observer.observe(&self.progress()).is_break() {
                return AdvanceReport::new(SearchStatus::Interrupted, completed, self.progress());
            }

            let Some(index) = self.frontier.pop_front() else {
                self.exhausted = true;
                return AdvanceReport::new(SearchStatus::Exhausted, completed, self.progress());
            };
            self.expanded += 1;
            completed += 1;

            let exchanges = (self.candidates)(&self.states[index].world);
            for exchange in exchanges {
                let mut next = self.states[index].world.clone();
                if next.apply(exchange.clone()).is_err() {
                    continue;
                }
                let key = next.state_key();
                if !self.visited.insert(key) {
                    continue;
                }
                self.generated += 1;
                let mut trace = self.states[index].trace.clone();
                trace.push(exchange);
                if next.matches(&self.goal) {
                    let cost = u64::try_from(trace.exchanges().len()).unwrap_or(u64::MAX);
                    self.solution = Some(SearchSolution {
                        trace,
                        cost,
                        expanded: self.expanded,
                    });
                    return AdvanceReport::new(SearchStatus::Solved, completed, self.progress());
                }
                let next_index = self.states.len();
                self.states.push(BfsState { world: next, trace });
                self.frontier.push_back(next_index);
            }
        }

        if self.frontier.is_empty() {
            self.exhausted = true;
        }
        AdvanceReport::new(self.status(), completed, self.progress())
    }
}

impl<RateId, Role, AccountId> SearchSolution<RateId, Role, AccountId> {
    pub fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }

    pub const fn cost(&self) -> u64 {
        self.cost
    }

    pub const fn expanded(&self) -> usize {
        self.expanded
    }
}

/// Breadth-first graph search over implicit, core-validated successors.
pub fn bfs<AccountId, A, RateId, Role, Candidates>(
    initial: &Economy<AccountId, A, RateId, Role>,
    goal: &Goal<AccountId, A>,
    candidates: Candidates,
) -> Option<SearchSolution<RateId, Role, AccountId>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
{
    let mut session = BfsSession::new(initial, goal.clone(), candidates);
    let mut observer = Continue;
    while !session.status().is_terminal() {
        session.advance(WorkBudget::new(usize::MAX), &mut observer);
    }
    session.into_solution()
}

/// Dijkstra search with caller-owned algorithmic edge costs.
///
/// Costs are disposable solver policy. A cost that changes economic validity
/// must instead be encoded as assets and rates.
pub fn dijkstra<AccountId, A, RateId, Role, Candidates, StepCost>(
    initial: &Economy<AccountId, A, RateId, Role>,
    goal: &Goal<AccountId, A>,
    mut candidates: Candidates,
    mut step_cost: StepCost,
) -> Option<SearchSolution<RateId, Role, AccountId>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
    StepCost: FnMut(
        &Economy<AccountId, A, RateId, Role>,
        &Exchange<RateId, Role, AccountId>,
        &Economy<AccountId, A, RateId, Role>,
    ) -> u64,
{
    let start = EconomicNode::root(initial.clone());
    let mut expanded = 0;
    let (path, cost) = pathfinding_dijkstra(
        &start,
        |node| {
            expanded += 1;
            weighted_successors(node, &mut candidates, &mut step_cost)
        },
        |node| node.world.matches(goal),
    )?;
    Some(solution_from_path(path, cost, expanded))
}

/// A* search with caller-owned edge costs and a derived state heuristic.
pub fn astar<AccountId, A, RateId, Role, Candidates, StepCost, Heuristic>(
    initial: &Economy<AccountId, A, RateId, Role>,
    goal: &Goal<AccountId, A>,
    mut candidates: Candidates,
    mut step_cost: StepCost,
    mut heuristic: Heuristic,
) -> Option<SearchSolution<RateId, Role, AccountId>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
    StepCost: FnMut(
        &Economy<AccountId, A, RateId, Role>,
        &Exchange<RateId, Role, AccountId>,
        &Economy<AccountId, A, RateId, Role>,
    ) -> u64,
    Heuristic: FnMut(&Economy<AccountId, A, RateId, Role>) -> u64,
{
    let start = EconomicNode::root(initial.clone());
    let mut expanded = 0;
    let (path, cost) = pathfinding_astar(
        &start,
        |node| {
            expanded += 1;
            weighted_successors(node, &mut candidates, &mut step_cost)
        },
        |node| heuristic(&node.world),
        |node| node.world.matches(goal),
    )?;
    Some(solution_from_path(path, cost, expanded))
}

/// State-ranked best-first compatibility search.
///
/// Prefer [`dijkstra`] or [`astar`] when the objective is naturally additive.
pub fn best_first<AccountId, A, RateId, Role, Candidates, Cost, Heuristic>(
    initial: &Economy<AccountId, A, RateId, Role>,
    goal: &Goal<AccountId, A>,
    candidates: Candidates,
    cost: Cost,
    heuristic: Heuristic,
) -> Option<SearchSolution<RateId, Role, AccountId>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Candidates: Fn(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
    Cost: Fn(&Economy<AccountId, A, RateId, Role>) -> u64,
    Heuristic: Fn(&Economy<AccountId, A, RateId, Role>) -> u64,
{
    let mut states = vec![(initial.clone(), Trace::new())];
    let mut heap = BinaryHeap::from([QueueEntry {
        priority: cost(initial).saturating_add(heuristic(initial)),
        cost: cost(initial),
        sequence: 0,
        index: 0,
    }]);
    let mut best = HashMap::from([(initial.state_key(), cost(initial))]);
    let mut expanded = 0;
    let mut sequence = 1_u64;

    while let Some(entry) = heap.pop() {
        let (state, trace) = states[entry.index].clone();
        let state_cost = cost(&state);
        if entry.cost != state_cost {
            continue;
        }
        if state.matches(goal) {
            return Some(SearchSolution {
                trace,
                cost: state_cost,
                expanded,
            });
        }
        expanded += 1;
        for exchange in candidates(&state) {
            let mut next = state.clone();
            if next.apply(exchange.clone()).is_err() {
                continue;
            }
            let next_cost = cost(&next);
            let key = next.state_key();
            if best.get(&key).is_some_and(|known| *known <= next_cost) {
                continue;
            }
            best.insert(key, next_cost);
            let mut next_trace = trace.clone();
            next_trace.push(exchange);
            let index = states.len();
            states.push((next.clone(), next_trace));
            heap.push(QueueEntry {
                priority: next_cost.saturating_add(heuristic(&next)),
                cost: next_cost,
                sequence,
                index,
            });
            sequence += 1;
        }
    }
    None
}

#[derive(Clone)]
struct EconomicNode<AccountId, A, RateId, Role> {
    world: Economy<AccountId, A, RateId, Role>,
    key: StateKey<AccountId, A>,
    incoming: Option<Exchange<RateId, Role, AccountId>>,
}

type WeightedNodes<AccountId, A, RateId, Role> =
    Vec<(EconomicNode<AccountId, A, RateId, Role>, u64)>;

impl<AccountId, A, RateId, Role> EconomicNode<AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
{
    fn root(world: Economy<AccountId, A, RateId, Role>) -> Self {
        let key = world.state_key();
        Self {
            world,
            key,
            incoming: None,
        }
    }

    fn successor(
        world: Economy<AccountId, A, RateId, Role>,
        incoming: Exchange<RateId, Role, AccountId>,
    ) -> Self {
        let key = world.state_key();
        Self {
            world,
            key,
            incoming: Some(incoming),
        }
    }
}

impl<AccountId, A, RateId, Role> PartialEq for EconomicNode<AccountId, A, RateId, Role>
where
    AccountId: PartialEq,
    A: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<AccountId, A, RateId, Role> Eq for EconomicNode<AccountId, A, RateId, Role>
where
    AccountId: Eq,
    A: Eq,
{
}

impl<AccountId, A, RateId, Role> Hash for EconomicNode<AccountId, A, RateId, Role>
where
    AccountId: Hash,
    A: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

fn weighted_successors<AccountId, A, RateId, Role, Candidates, StepCost>(
    node: &EconomicNode<AccountId, A, RateId, Role>,
    candidates: &mut Candidates,
    step_cost: &mut StepCost,
) -> WeightedNodes<AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
    StepCost: FnMut(
        &Economy<AccountId, A, RateId, Role>,
        &Exchange<RateId, Role, AccountId>,
        &Economy<AccountId, A, RateId, Role>,
    ) -> u64,
{
    candidates(&node.world)
        .into_iter()
        .filter_map(|exchange| {
            let mut next = node.world.clone();
            next.apply(exchange.clone()).ok()?;
            let cost = step_cost(&node.world, &exchange, &next);
            Some((EconomicNode::successor(next, exchange), cost))
        })
        .collect()
}

fn solution_from_path<AccountId, A, RateId, Role>(
    path: Vec<EconomicNode<AccountId, A, RateId, Role>>,
    cost: u64,
    expanded: usize,
) -> SearchSolution<RateId, Role, AccountId> {
    let mut trace = Trace::new();
    trace.extend(path.into_iter().skip(1).filter_map(|node| node.incoming));
    SearchSolution {
        trace,
        cost,
        expanded,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueueEntry {
    priority: u64,
    cost: u64,
    sequence: u64,
    index: usize,
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
