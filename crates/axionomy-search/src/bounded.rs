//! Searches that bound what they explore: depth-first branch-and-bound for
//! the cheapest goal state, beam search keeping the best states of each
//! depth, and minimax with alpha-beta pruning for a two-player choice. Every
//! successor is one the kernel applied; what these keep beside the economy
//! is derived and disposable.

use crate::SearchSolution;
use axionomy::{Economy, Exchange, Goal, Ontology, StateKey, Trace};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// What a bounded search found, how much it expanded, and whether it ran to
/// the end, so that its answer is the best there is.
#[derive(Debug, Clone)]
pub struct Bounded<O: Ontology> {
    pub solution: Option<SearchSolution<O>>,
    pub expanded: usize,
    pub exhausted: bool,
}

/// The cheapest state meeting `goal` that `candidates` reach from `initial`,
/// by depth-first branch-and-bound. `cost` is a goal state's cost, and
/// `bound` a lower bound on the cost of any goal state reachable from a
/// state: a branch is abandoned once its bound is no better than the
/// cheapest goal found. A goal state is not expanded further, and a state is
/// expanded once, since a goal state's cost is a function of the state. At
/// most `max_expansions` states are expanded; candidates are tried in the
/// order given.
pub fn branch_and_bound<O, Candidates, Cost, Bound>(
    initial: &Economy<O>,
    goal: &Goal<O>,
    mut candidates: Candidates,
    mut cost: Cost,
    mut bound: Bound,
    max_expansions: usize,
) -> Bounded<O>
where
    O: Ontology,
    Candidates: FnMut(&Economy<O>) -> Vec<Exchange<O>>,
    Cost: FnMut(&Economy<O>) -> u64,
    Bound: FnMut(&Economy<O>) -> u64,
{
    let mut stack = vec![(initial.clone(), Vec::new())];
    let mut seen: HashSet<StateKey<O>> = HashSet::from([initial.state_key()]);
    let mut best: Option<(u64, Vec<Exchange<O>>)> = None;
    let mut expanded = 0;
    while let Some((state, trace)) = stack.pop() {
        if expanded >= max_expansions {
            return finish(best, expanded, false);
        }
        expanded += 1;
        if state.matches(goal) {
            let value = cost(&state);
            if best.as_ref().is_none_or(|(cheapest, _)| value < *cheapest) {
                best = Some((value, trace));
            }
            continue;
        }
        if let Some((cheapest, _)) = &best
            && bound(&state) >= *cheapest
        {
            continue;
        }
        let mut children = Vec::new();
        for exchange in candidates(&state) {
            let mut child = state.clone();
            if child.apply(exchange.clone()).is_ok() && seen.insert(child.state_key()) {
                let mut longer = trace.clone();
                longer.push(exchange);
                children.push((child, longer));
            }
        }
        // The first candidate is explored first.
        stack.extend(children.into_iter().rev());
    }
    finish(best, expanded, true)
}

fn finish<O: Ontology>(
    best: Option<(u64, Vec<Exchange<O>>)>,
    expanded: usize,
    exhausted: bool,
) -> Bounded<O> {
    Bounded {
        solution: best.map(|(cost, trace)| SearchSolution {
            trace: trace.into_iter().collect::<Trace<O>>(),
            cost,
            expanded,
        }),
        expanded,
        exhausted,
    }
}

/// A trace and the score of the state it reaches.
#[derive(Debug, Clone)]
pub struct Scored<O: Ontology> {
    pub trace: Trace<O>,
    pub score: i64,
}

/// What a beam search found and how much it expanded.
#[derive(Debug, Clone)]
pub struct Beamed<O: Ontology> {
    /// The best-scoring state meeting the goal, the first or the last found
    /// on a tie, as the search was asked.
    pub best: Option<Scored<O>>,
    pub expanded: usize,
}

/// Which of the goal states scoring best alike a beam search answers with:
/// the first it reaches, or the last. It reaches them depth by depth, and
/// within a depth in the order it keeps them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tie {
    First,
    Last,
}

/// Beam search: from `initial`, each depth expands every state kept, and
/// keeps the `width` best-scoring new states, by `score`, higher better, in
/// candidate order on a tie, for at most `depth` depths. Returns the
/// best-scoring state that meets `goal` among those it reached, the first or
/// the last of those scoring alike as `tie` says.
pub fn beam<O, Candidates, Score>(
    initial: &Economy<O>,
    goal: &Goal<O>,
    mut candidates: Candidates,
    mut score: Score,
    width: usize,
    depth: usize,
    tie: Tie,
) -> Beamed<O>
where
    O: Ontology,
    Candidates: FnMut(&Economy<O>) -> Vec<Exchange<O>>,
    Score: FnMut(&Economy<O>) -> i64,
{
    let mut best: Option<Scored<O>> = None;
    let mut consider = |state: &Economy<O>, trace: &[Exchange<O>], value: i64| {
        let better = |found: &Scored<O>| match tie {
            Tie::First => value > found.score,
            Tie::Last => value >= found.score,
        };
        if state.matches(goal) && best.as_ref().is_none_or(better) {
            best = Some(Scored {
                trace: trace.iter().cloned().collect(),
                score: value,
            });
        }
    };
    let initial_score = score(initial);
    consider(initial, &[], initial_score);
    let mut seen: HashSet<StateKey<O>> = HashSet::from([initial.state_key()]);
    let mut layer = vec![(initial.clone(), Vec::new())];
    let mut expanded = 0;
    for _ in 0..depth {
        let mut next: Vec<(i64, Economy<O>, Vec<Exchange<O>>)> = Vec::new();
        for (state, trace) in &layer {
            if state.matches(goal) {
                continue;
            }
            expanded += 1;
            for exchange in candidates(state) {
                let mut child = state.clone();
                if child.apply(exchange.clone()).is_ok() && seen.insert(child.state_key()) {
                    let mut longer = trace.clone();
                    longer.push(exchange);
                    next.push((score(&child), child, longer));
                }
            }
        }
        // Stable: ties keep candidate order.
        next.sort_by_key(|(score, _, _)| std::cmp::Reverse(*score));
        next.truncate(width);
        for (value, state, trace) in &next {
            consider(state, trace, *value);
        }
        if next.is_empty() {
            break;
        }
        layer = next
            .into_iter()
            .map(|(_, state, trace)| (state, trace))
            .collect();
    }
    Beamed { best, expanded }
}

/// The exchange minimax chooses and the value it expects, for player 0.
#[derive(Debug, Clone)]
pub struct Chosen<O: Ontology> {
    pub exchange: Exchange<O>,
    pub value: i64,
    /// The states the search evaluated or expanded.
    pub nodes: usize,
}

/// The exchange the player to move at `state` chooses by minimax with
/// alpha-beta pruning, looking `depth` exchanges ahead in a game of two
/// players: `actor` says which player, 0 or 1, moves at a state, and
/// `evaluate` scores a state for player 0, so that higher is better for
/// player 0 and worse for player 1. A state at the depth, or with no
/// candidates, is evaluated as it is. Players need not alternate. Ties go to
/// the earlier candidate; there is no choice when `state` has no candidates.
pub fn minimax<O, Candidates, Actor, Evaluate>(
    state: &Economy<O>,
    depth: usize,
    mut candidates: Candidates,
    mut actor: Actor,
    mut evaluate: Evaluate,
) -> Option<Chosen<O>>
where
    O: Ontology,
    Candidates: FnMut(&Economy<O>) -> Vec<Exchange<O>>,
    Actor: FnMut(&Economy<O>) -> usize,
    Evaluate: FnMut(&Economy<O>) -> i64,
{
    let mut game = Game {
        candidates: &mut candidates,
        actor: &mut actor,
        evaluate: &mut evaluate,
        nodes: 0,
    };
    let maximizing = (game.actor)(state) == 0;
    let mut chosen: Option<(Exchange<O>, i64)> = None;
    let (mut alpha, mut beta) = (i64::MIN, i64::MAX);
    for (exchange, child) in game.children(state) {
        let value = game.value(&child, depth.saturating_sub(1), alpha, beta);
        let better = chosen.as_ref().is_none_or(|(_, best)| {
            if maximizing {
                value > *best
            } else {
                value < *best
            }
        });
        if better {
            chosen = Some((exchange, value));
        }
        let best = chosen.as_ref().map_or(value, |(_, best)| *best);
        if maximizing {
            alpha = alpha.max(best);
        } else {
            beta = beta.min(best);
        }
    }
    let nodes = game.nodes;
    chosen.map(|(exchange, value)| Chosen {
        exchange,
        value,
        nodes,
    })
}

struct Game<'a, Candidates, Actor, Evaluate> {
    candidates: &'a mut Candidates,
    actor: &'a mut Actor,
    evaluate: &'a mut Evaluate,
    nodes: usize,
}

impl<Candidates, Actor, Evaluate> Game<'_, Candidates, Actor, Evaluate> {
    fn children<O: Ontology>(&mut self, state: &Economy<O>) -> Vec<(Exchange<O>, Economy<O>)>
    where
        Candidates: FnMut(&Economy<O>) -> Vec<Exchange<O>>,
    {
        (self.candidates)(state)
            .into_iter()
            .filter_map(|exchange| {
                let mut child = state.clone();
                child
                    .apply(exchange.clone())
                    .ok()
                    .map(|_| (exchange, child))
            })
            .collect()
    }

    fn value<O: Ontology>(
        &mut self,
        state: &Economy<O>,
        depth: usize,
        mut alpha: i64,
        mut beta: i64,
    ) -> i64
    where
        Candidates: FnMut(&Economy<O>) -> Vec<Exchange<O>>,
        Actor: FnMut(&Economy<O>) -> usize,
        Evaluate: FnMut(&Economy<O>) -> i64,
    {
        self.nodes += 1;
        if depth == 0 {
            return (self.evaluate)(state);
        }
        let children = self.children(state);
        if children.is_empty() {
            return (self.evaluate)(state);
        }
        if (self.actor)(state) == 0 {
            let mut best = i64::MIN;
            for (_, child) in children {
                best = best.max(self.value(&child, depth - 1, alpha, beta));
                alpha = alpha.max(best);
                if alpha >= beta {
                    break;
                }
            }
            best
        } else {
            let mut best = i64::MAX;
            for (_, child) in children {
                best = best.min(self.value(&child, depth - 1, alpha, beta));
                beta = beta.min(best);
                if alpha >= beta {
                    break;
                }
            }
            best
        }
    }
}
