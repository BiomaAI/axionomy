//! A key-door maze where action count and encoded energy cost disagree.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, basket,
};
use axionomy_search::{SearchSolution, astar, bfs, dijkstra};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Node {
    Start,
    KeyRoom,
    Door,
    Detour,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Agent,
    World,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    At(Node),
    Edge(Node, Node),
    Key,
    Locked,
    Open,
    Energy,
    SpentEnergy,
    Target(Node),
    Distance(Node),
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Actor,
    Environment,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Move {
        from: Node,
        to: Node,
        energy: u64,
        needs_open_door: bool,
    },
    TakeKey,
    UnlockDoor,
    Finish,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;

const EDGES: [(Node, Node, u64, bool); 5] = [
    (Node::Start, Node::KeyRoom, 2, false),
    (Node::KeyRoom, Node::Door, 2, true),
    (Node::Door, Node::Exit, 2, false),
    (Node::Start, Node::Detour, 4, false),
    (Node::Detour, Node::Exit, 5, false),
];

/// Builds the complete closed problem. Topology, lock state, energy, target,
/// and heuristic values are all assets held by accounts.
pub fn initial() -> World {
    let mut environment = axionomy::Basket::new();
    for (from, to, _, _) in EDGES {
        environment.insert(Asset::Edge(from, to), Quantity::new(1));
    }
    environment.insert(Asset::Key, Quantity::new(1));
    environment.insert(Asset::Locked, Quantity::new(1));
    environment.insert(Asset::Target(Node::Exit), Quantity::new(1));
    for (node, distance) in [
        (Node::Start, 6),
        (Node::KeyRoom, 4),
        (Node::Door, 2),
        (Node::Detour, 5),
        (Node::Exit, 0),
    ] {
        environment.insert(Asset::Distance(node), Quantity::new(distance));
    }

    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent,
            Account::from(basket([(Asset::At(Node::Start), 1), (Asset::Energy, 9)])),
        )
        .account(AccountId::World, Account::from(environment))
        .account(AccountId::Success, Account::default())
        .invariant(
            [
                Node::Start,
                Node::KeyRoom,
                Node::Door,
                Node::Detour,
                Node::Exit,
            ]
            .into_iter()
            .fold(
                LinearInvariant::new("one agent position"),
                |invariant, node| invariant.weight(Asset::At(node), 1),
            ),
        )
        .invariant(
            LinearInvariant::new("energy is only spent")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        );

    for (from, to, energy, needs_open_door) in EDGES {
        let mut rate = Rate::new()
            .consume(
                Role::Actor,
                basket([(Asset::At(from), 1), (Asset::Energy, energy)]),
            )
            .produce(
                Role::Actor,
                basket([(Asset::At(to), 1), (Asset::SpentEnergy, energy)]),
            )
            .preserve(Role::Environment, basket([(Asset::Edge(from, to), 1)]))
            .distinct(Role::Actor, Role::Environment);
        if needs_open_door {
            rate = rate.preserve(Role::Environment, basket([(Asset::Open, 1)]));
        }
        builder = builder.rate(
            RateId::Move {
                from,
                to,
                energy,
                needs_open_door,
            },
            rate,
        );
    }

    builder
        .rate(
            RateId::TakeKey,
            Rate::new()
                .preserve(Role::Actor, basket([(Asset::At(Node::KeyRoom), 1)]))
                .consume(Role::Environment, basket([(Asset::Key, 1)]))
                .produce(Role::Actor, basket([(Asset::Key, 1)]))
                .distinct(Role::Actor, Role::Environment),
        )
        .rate(
            RateId::UnlockDoor,
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Key, 1)]))
                .consume(Role::Environment, basket([(Asset::Locked, 1)]))
                .produce(Role::Environment, basket([(Asset::Open, 1)]))
                .distinct(Role::Actor, Role::Environment),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Actor, basket([(Asset::At(Node::Exit), 1)]))
                .preserve(Role::Environment, basket([(Asset::Target(Node::Exit), 1)]))
                .produce(Role::Goal, basket([(Asset::Solved, 1)]))
                .distinct(Role::Actor, Role::Goal),
        )
        .build()
        .expect("maze model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

/// Derives executable exchanges from the rate identifiers encoded in the economy.
pub fn candidates(world: &World) -> Vec<Action> {
    let mut rate_ids: Vec<_> = world.rate_ids().copied().collect();
    rate_ids.sort();
    world.applicable(rate_ids.into_iter().map(action))
}

pub fn solve_bfs(world: &World) -> Option<Solution> {
    bfs(world, &goal(), candidates)
}

pub fn solve_dijkstra(world: &World) -> Option<Solution> {
    dijkstra(world, &goal(), candidates, move_energy)
}

pub fn solve_astar(world: &World) -> Option<Solution> {
    astar(world, &goal(), candidates, move_energy, heuristic)
}

pub fn spent_energy(world: &World) -> u64 {
    world.balance(&AccountId::Agent, &Asset::SpentEnergy).get()
}

pub fn heuristic(world: &World) -> u64 {
    [
        Node::Start,
        Node::KeyRoom,
        Node::Door,
        Node::Detour,
        Node::Exit,
    ]
    .into_iter()
    .find(|node| {
        !world
            .balance(&AccountId::Agent, &Asset::At(*node))
            .is_zero()
    })
    .map_or(0, |node| {
        world
            .balance(&AccountId::World, &Asset::Distance(node))
            .get()
    })
}

fn move_energy(_: &World, action: &Action, _: &World) -> u64 {
    match action.rate() {
        RateId::Move { energy, .. } => *energy,
        RateId::TakeKey | RateId::UnlockDoor | RateId::Finish => 0,
    }
}

fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1))
        .bind(Role::Actor, AccountId::Agent)
        .bind(Role::Environment, AccountId::World);
    if rate == RateId::Finish {
        exchange.bind(Role::Goal, AccountId::Success)
    } else {
        exchange
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_and_resource_optimizers_choose_different_valid_paths() {
        let world = initial();
        let shallow = solve_bfs(&world).expect("detour is feasible");
        let cheapest = solve_dijkstra(&world).expect("key route is feasible");
        let astar = solve_astar(&world).expect("key route is feasible");

        assert_eq!(shallow.trace().exchanges().len(), 3);
        assert_eq!(cheapest.cost(), 6);
        assert_eq!(astar.cost(), cheapest.cost());

        for solution in [shallow, cheapest, astar] {
            let mut replay = initial();
            replay.replay(solution.trace()).expect("trace must replay");
            assert!(replay.matches(&goal()));
        }
    }
}
