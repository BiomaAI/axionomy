//! Dijkstra and A* answer with the cheapest plan the economy accepts, and the
//! plan they answer with costs what they report. A walker crosses a weighted
//! directed graph whose edges are rates; a state first reached dearly and
//! later cheaply, through another parent or another edge, must leave the
//! cheaper step in the plan. Many small seeded graphs are checked against
//! brute force.

use axionomy::{Account, Economy, EconomyBuilder, Exchange, Goal, Quantity, Rate, Trace, basket};
use axionomy_search::session::{Continue, WorkBudget};
use axionomy_search::{AStarSession, SearchSolution, astar, dijkstra};
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeSet;

type Node = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Walker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    At(Node),
}

/// A directed edge, carrying what crossing it costs the search. Parallel
/// edges between the same nodes differ in weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Edge {
    from: Node,
    to: Node,
    weight: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Walker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Walk {}

impl axionomy::Ontology for Walk {
    const NAME: &'static str = "walk";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = Edge;
    type Role = Role;
}

type World = Economy<Walk>;
type Step = Exchange<Walk>;
type Solution = SearchSolution<Walk>;

fn step(edge: Edge) -> Step {
    Exchange::new(edge, Quantity::new(1)).bind(Role::Walker, AccountId::Walker)
}

fn weight(_: &World, step: &Step, _: &World) -> u64 {
    step.rate().weight
}

fn position(world: &World) -> Node {
    let walker = world
        .account(&AccountId::Walker)
        .expect("the walker never leaves");
    walker
        .balances()
        .iter()
        .find_map(|(Asset::At(node), held)| (!held.is_zero()).then_some(*node))
        .expect("the walker stands somewhere")
}

/// The walker starts at node 0 and wants to reach the last node.
struct Graph {
    nodes: Node,
    /// In the order the search is offered them.
    edges: Vec<Edge>,
}

impl Graph {
    fn new(nodes: Node, edges: impl IntoIterator<Item = (Node, Node, u64)>) -> Self {
        let edges = edges
            .into_iter()
            .map(|(from, to, weight)| Edge { from, to, weight })
            .collect();
        Self { nodes, edges }
    }

    /// Up to seven nodes; each ordered pair is joined by no edge, one, or two
    /// of different weights, zero included, offered in a shuffled order.
    fn random(random: &mut ChaCha8Rng) -> Self {
        let nodes = random.random_range(2..=7);
        let mut edges = Vec::new();
        for from in 0..nodes {
            for to in (0..nodes).filter(|to| *to != from) {
                if !random.random_bool(0.4) {
                    continue;
                }
                let lanes = random.random_range(1..=2);
                let weights: BTreeSet<u64> =
                    (0..lanes).map(|_| random.random_range(0..=9)).collect();
                edges.extend(weights.into_iter().map(|weight| Edge { from, to, weight }));
            }
        }
        edges.shuffle(random);
        Self { nodes, edges }
    }

    fn target(&self) -> Node {
        self.nodes - 1
    }

    fn world(&self) -> World {
        self.edges
            .iter()
            .fold(EconomyBuilder::new(), |builder, edge| {
                builder.rate(
                    *edge,
                    Rate::new()
                        .consume(Role::Walker, basket([(Asset::At(edge.from), 1)]))
                        .produce(Role::Walker, basket([(Asset::At(edge.to), 1)])),
                )
            })
            .account(
                AccountId::Walker,
                Account::from(basket([(Asset::At(0), 1)])),
            )
            .build()
            .unwrap()
    }

    fn goal(&self) -> Goal<Walk> {
        Goal::new().require(AccountId::Walker, basket([(Asset::At(self.target()), 1)]))
    }

    /// Every edge, in the graph's order; the economy rejects those that do
    /// not leave where the walker stands.
    fn candidates(&self) -> impl FnMut(&World) -> Vec<Step> + '_ {
        |_| self.edges.iter().copied().map(step).collect()
    }

    /// The cheapest walk from `from` to the target, by trying every simple
    /// path.
    fn cheapest_from(&self, from: Node) -> Option<u64> {
        fn walk(graph: &Graph, at: Node, seen: u32, spent: u64, best: &mut Option<u64>) {
            if at == graph.target() {
                *best = Some(best.map_or(spent, |best| best.min(spent)));
                return;
            }
            for edge in &graph.edges {
                if edge.from == at && seen & (1 << edge.to) == 0 {
                    walk(
                        graph,
                        edge.to,
                        seen | 1 << edge.to,
                        spent + edge.weight,
                        best,
                    );
                }
            }
        }
        let mut best = None;
        walk(self, from, 1 << from, 0, &mut best);
        best
    }

    fn cheapest(&self) -> Option<u64> {
        self.cheapest_from(0)
    }

    /// More than any walk costs: what a node that cannot reach the target
    /// may be estimated at.
    fn beyond(&self) -> u64 {
        self.edges.iter().map(|edge| edge.weight).sum::<u64>() + 1
    }

    /// The exact cost still to pay from each node, scaled by
    /// `numerator / denominator` at most one and rounded down: consistent.
    fn scaled(&self, numerator: u64, denominator: u64) -> Vec<u64> {
        (0..self.nodes)
            .map(|node| self.cheapest_from(node).unwrap_or(self.beyond()) * numerator / denominator)
            .collect()
    }

    /// Each node's estimate drawn at random below its exact cost: admissible,
    /// and usually inconsistent, so A* reopens states it already expanded.
    fn underestimated(&self, random: &mut ChaCha8Rng) -> Vec<u64> {
        (0..self.nodes)
            .map(|node| random.random_range(0..=self.cheapest_from(node).unwrap_or(self.beyond())))
            .collect()
    }

    /// What the search pays for a plan.
    fn paid(&self, trace: &Trace<Walk>) -> u64 {
        trace
            .exchanges()
            .iter()
            .map(|step| step.rate().weight)
            .sum()
    }

    /// The answer replays from the start to the goal, costs what the search
    /// reports, and is the cheapest there is.
    fn check(&self, context: &str, found: Option<Solution>) {
        let found = match (found, self.cheapest()) {
            (None, None) => return,
            (Some(found), Some(cheapest)) => {
                assert_eq!(found.cost(), cheapest, "{context}: not the cheapest");
                found
            }
            (found, cheapest) => panic!(
                "{context}: found {:?}, but the cheapest walk is {cheapest:?}",
                found.map(|found| found.cost())
            ),
        };
        let end = self
            .world()
            .replayed(found.trace())
            .unwrap_or_else(|error| panic!("{context}: the plan does not replay: {error:?}"));
        assert!(end.matches(&self.goal()), "{context}: the plan stops short");
        assert_eq!(
            self.paid(found.trace()),
            found.cost(),
            "{context}: the plan costs other than reported"
        );
    }

    fn dijkstra(&self) -> Option<Solution> {
        dijkstra(&self.world(), &self.goal(), self.candidates(), weight)
    }

    fn astar(&self, estimates: &[u64]) -> Option<Solution> {
        astar(
            &self.world(),
            &self.goal(),
            self.candidates(),
            weight,
            |world: &World| estimates[usize::from(position(world))],
        )
    }

    /// A* as a session advanced one expansion at a time.
    fn session(&self, estimates: &[u64]) -> Option<Solution> {
        let mut session = AStarSession::new(
            &self.world(),
            self.goal(),
            self.candidates(),
            weight,
            |world: &World| estimates[usize::from(position(world))],
        );
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut Continue);
        }
        session.into_solution()
    }

    fn check_all(&self, context: &str, estimates: &[u64]) {
        self.check(&format!("{context}, dijkstra"), self.dijkstra());
        self.check(&format!("{context}, astar"), self.astar(estimates));
        self.check(&format!("{context}, session"), self.session(estimates));
    }
}

/// From S, A is the nearer node but its way to X is dear; B is farther but
/// its way to X is cheap. Expanding A first reaches X dearly, and expanding B
/// then reaches it cheaply through another parent. The goal lies beyond X.
#[test]
fn a_state_reached_more_cheaply_through_another_parent_keeps_that_parent() {
    let (s, a, b, x, g) = (0, 1, 2, 3, 4);
    let graph = Graph::new(5, [(s, a, 1), (s, b, 2), (a, x, 10), (b, x, 1), (x, g, 1)]);
    assert_eq!(graph.cheapest(), Some(4));
    // Consistent, and still expands A before B.
    let estimates = [0, 0, 1, 1, 0];
    graph.check_all("another parent", &estimates);
    graph.check_all("another parent, no estimate", &[0; 5]);

    let cheapest = graph.dijkstra().unwrap();
    let to = |from, to, weight| step(Edge { from, to, weight });
    assert_eq!(
        cheapest.trace().exchanges(),
        [to(s, b, 2), to(b, x, 1), to(x, g, 1)]
    );
}

/// The dear edge from S to X is offered before the cheap one: X is first
/// reached dearly and then cheaply from the same parent.
#[test]
fn a_state_reached_more_cheaply_by_another_edge_keeps_that_edge() {
    let (s, x, g) = (0, 1, 2);
    let graph = Graph::new(3, [(s, x, 5), (s, x, 1), (x, g, 1)]);
    assert_eq!(graph.cheapest(), Some(2));
    graph.check_all("another edge", &[0, 1, 0]);
}

/// An admissible estimate that is not consistent closes X dearly, through A,
/// before B reaches it cheaply: A* must reopen X and plan through B.
#[test]
fn a_state_reopened_by_an_inconsistent_estimate_keeps_the_cheaper_parent() {
    let (s, a, b, x, g) = (0, 1, 2, 3, 4);
    let graph = Graph::new(5, [(s, a, 1), (s, b, 1), (a, x, 3), (b, x, 1), (x, g, 4)]);
    assert_eq!(graph.cheapest(), Some(6));
    // B looks dear (5 of its true 5), A cheap (0 of 7): X is expanded at
    // cost 4 through A before B is, then reached at cost 2 through B.
    let estimates = [0, 0, 5, 0, 0];
    graph.check_all("reopened", &estimates);
}

#[test]
fn the_plans_are_the_cheapest_and_cost_what_is_reported_on_random_graphs() {
    let mut random = ChaCha8Rng::seed_from_u64(0x5ea2c4);
    for index in 0..400 {
        let graph = Graph::random(&mut random);
        let context = format!("graph {index} {:?}", graph.edges);
        graph.check_all(
            &format!("{context}, no estimate"),
            &vec![0; graph.nodes.into()],
        );
        graph.check_all(&format!("{context}, exact"), &graph.scaled(1, 1));
        graph.check_all(&format!("{context}, half"), &graph.scaled(1, 2));
        let underestimated = graph.underestimated(&mut random);
        graph.check_all(
            &format!("{context}, below {underestimated:?}"),
            &underestimated,
        );
    }
}
