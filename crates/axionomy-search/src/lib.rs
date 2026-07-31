#![doc = include_str!("../README.md")]

pub mod mcts;
pub mod monte_carlo;
pub mod rl;
pub mod rollout;
pub mod sampling;

use axionomy::{Economy, Exchange, Goal, Trace};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct SearchSolution<RateId, Role, AccountId> {
    trace: Trace<RateId, Role, AccountId>,
    cost: u64,
    expanded: usize,
}

impl<RateId, Role, AccountId> SearchSolution<RateId, Role, AccountId> {
    pub fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }

    pub fn cost(&self) -> u64 {
        self.cost
    }

    pub fn expanded(&self) -> usize {
        self.expanded
    }
}

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
    Candidates: Fn(&Economy<AccountId, A, RateId, Role>) -> Vec<Exchange<RateId, Role, AccountId>>,
{
    if initial.matches(goal) {
        return Some(SearchSolution {
            trace: Trace::new(),
            cost: 0,
            expanded: 0,
        });
    }

    let mut queue = VecDeque::from([(initial.clone(), Trace::new())]);
    let mut visited = HashSet::from([initial.state_key()]);
    let mut expanded = 0;

    while let Some((state, trace)) = queue.pop_front() {
        expanded += 1;
        for exchange in candidates(&state) {
            let mut next = state.clone();
            if next.apply(exchange.clone()).is_err() {
                continue;
            }
            let key = next.state_key();
            if !visited.insert(key) {
                continue;
            }
            let mut next_trace = trace.clone();
            next_trace.push(exchange);
            if next.matches(goal) {
                return Some(SearchSolution {
                    cost: u64::try_from(next_trace.exchanges().len()).unwrap_or(u64::MAX),
                    trace: next_trace,
                    expanded,
                });
            }
            queue.push_back((next, next_trace));
        }
    }
    None
}

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
