#![doc = include_str!("../README.md")]

pub mod action_source;
pub mod ismcts;
pub mod mcts;
pub mod monte_carlo;
pub mod rl;
pub mod rollout;
pub mod sampling;

use axionomy::{Economy, Exchange, Goal, Quantity, QuantityScalar, Trace};
use pathfinding::prelude::{
    astar as pathfinding_astar, bfs as pathfinding_bfs, dijkstra as pathfinding_dijkstra,
};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct SearchSolution<RateId, Role, AccountId, N = u64> {
    trace: Trace<RateId, Role, AccountId, N>,
    cost: u64,
    expanded: usize,
}

impl<RateId, Role, AccountId, N> SearchSolution<RateId, Role, AccountId, N> {
    pub fn trace(&self) -> &Trace<RateId, Role, AccountId, N> {
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
pub fn bfs<AccountId, A, RateId, Role, N, Candidates>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    mut candidates: Candidates,
) -> Option<SearchSolution<RateId, Role, AccountId, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
{
    let start = EconomicNode::root(initial.fork());
    let mut expanded = 0;
    let path = pathfinding_bfs(
        &start,
        |node| {
            expanded += 1;
            successors(node, &mut candidates)
        },
        |node| node.world.matches(goal),
    )?;
    let cost = u64::try_from(path.len().saturating_sub(1)).unwrap_or(u64::MAX);
    Some(solution_from_path(path, cost, expanded))
}

/// Dijkstra search with caller-owned algorithmic edge costs.
///
/// Costs are disposable solver policy. A cost that changes economic validity
/// must instead be encoded as assets and rates.
pub fn dijkstra<AccountId, A, RateId, Role, N, Candidates, StepCost>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    mut candidates: Candidates,
    mut step_cost: StepCost,
) -> Option<SearchSolution<RateId, Role, AccountId, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
    StepCost: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
        &Exchange<RateId, Role, AccountId, N>,
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> u64,
{
    let start = EconomicNode::root(initial.fork());
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
pub fn astar<AccountId, A, RateId, Role, N, Candidates, StepCost, Heuristic>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    mut candidates: Candidates,
    mut step_cost: StepCost,
    mut heuristic: Heuristic,
) -> Option<SearchSolution<RateId, Role, AccountId, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
    StepCost: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
        &Exchange<RateId, Role, AccountId, N>,
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> u64,
    Heuristic: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> u64,
{
    let start = EconomicNode::root(initial.fork());
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
pub fn best_first<AccountId, A, RateId, Role, N, Candidates, Cost, Heuristic>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    candidates: Candidates,
    cost: Cost,
    heuristic: Heuristic,
) -> Option<SearchSolution<RateId, Role, AccountId, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Candidates:
        Fn(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
    Cost: Fn(&Economy<AccountId, A, RateId, Role, N>) -> u64,
    Heuristic: Fn(&Economy<AccountId, A, RateId, Role, N>) -> u64,
{
    let mut states = vec![(initial.fork(), Trace::new())];
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
            let mut next = state.fork();
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
            states.push((next.fork(), next_trace));
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
struct EconomicNode<AccountId, A, RateId, Role, N> {
    world: Economy<AccountId, A, RateId, Role, N>,
    key: Vec<(AccountId, A, Quantity<N>)>,
    incoming: Option<Exchange<RateId, Role, AccountId, N>>,
}

type WeightedNodes<AccountId, A, RateId, Role, N> =
    Vec<(EconomicNode<AccountId, A, RateId, Role, N>, u64)>;

impl<AccountId, A, RateId, Role, N> EconomicNode<AccountId, A, RateId, Role, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
{
    fn root(world: Economy<AccountId, A, RateId, Role, N>) -> Self {
        let key = world.state_key();
        Self {
            world,
            key,
            incoming: None,
        }
    }

    fn successor(
        world: Economy<AccountId, A, RateId, Role, N>,
        incoming: Exchange<RateId, Role, AccountId, N>,
    ) -> Self {
        let key = world.state_key();
        Self {
            world,
            key,
            incoming: Some(incoming),
        }
    }
}

impl<AccountId, A, RateId, Role, N> PartialEq for EconomicNode<AccountId, A, RateId, Role, N>
where
    AccountId: PartialEq,
    A: PartialEq,
    N: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<AccountId, A, RateId, Role, N> Eq for EconomicNode<AccountId, A, RateId, Role, N>
where
    AccountId: Eq,
    A: Eq,
    N: Eq,
{
}

impl<AccountId, A, RateId, Role, N> Hash for EconomicNode<AccountId, A, RateId, Role, N>
where
    AccountId: Hash,
    A: Hash,
    N: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

fn successors<AccountId, A, RateId, Role, N, Candidates>(
    node: &EconomicNode<AccountId, A, RateId, Role, N>,
    candidates: &mut Candidates,
) -> Vec<EconomicNode<AccountId, A, RateId, Role, N>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
{
    candidates(&node.world)
        .into_iter()
        .filter_map(|exchange| {
            let mut next = node.world.fork();
            next.apply(exchange.clone())
                .ok()
                .map(|_| EconomicNode::successor(next, exchange))
        })
        .collect()
}

fn weighted_successors<AccountId, A, RateId, Role, N, Candidates, StepCost>(
    node: &EconomicNode<AccountId, A, RateId, Role, N>,
    candidates: &mut Candidates,
    step_cost: &mut StepCost,
) -> WeightedNodes<AccountId, A, RateId, Role, N>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
    StepCost: FnMut(
        &Economy<AccountId, A, RateId, Role, N>,
        &Exchange<RateId, Role, AccountId, N>,
        &Economy<AccountId, A, RateId, Role, N>,
    ) -> u64,
{
    candidates(&node.world)
        .into_iter()
        .filter_map(|exchange| {
            let mut next = node.world.fork();
            next.apply(exchange.clone()).ok()?;
            let cost = step_cost(&node.world, &exchange, &next);
            Some((EconomicNode::successor(next, exchange), cost))
        })
        .collect()
}

fn solution_from_path<AccountId, A, RateId, Role, N>(
    path: Vec<EconomicNode<AccountId, A, RateId, Role, N>>,
    cost: u64,
    expanded: usize,
) -> SearchSolution<RateId, Role, AccountId, N> {
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
