//! A key-door maze where action count and encoded energy cost disagree.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, basket,
};
use axionomy_search::{
    SearchSolution, astar, bfs, dijkstra,
    pareto::{self, Objective, ObjectiveVector, ParetoError, ParetoSearchResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Node {
    Start,
    Atrium,
    Library,
    Gallery,
    Archive,
    Scriptorium,
    KeyRoom,
    Gate,
    Vault,
    Garden,
    Market,
    Canal,
    Docks,
    Foundry,
    Tower,
    Observatory,
    Tunnel,
    Ridge,
    Ruins,
    Bridge,
    Chapel,
    Workshop,
    Detour,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Agent,
    World,
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
    Time,
    SpentTime,
    Target(Node),
    Distance(Node),
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Actor,
    Environment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Move {
        from: Node,
        to: Node,
        energy: u64,
        needs_open_door: bool,
    },
    TakeKey {
        at: Node,
    },
    UnlockDoor {
        at: Node,
    },
    Finish,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;
pub type ParetoResult = ParetoSearchResult<RateId, Role, AccountId, u64, ObjectiveKey, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveKey {
    Energy,
    Time,
}

const MICRO_EDGES: [(Node, Node, u64, bool); 5] = [
    (Node::Start, Node::KeyRoom, 2, false),
    (Node::KeyRoom, Node::Gate, 2, true),
    (Node::Gate, Node::Exit, 2, false),
    (Node::Start, Node::Detour, 4, false),
    (Node::Detour, Node::Exit, 5, false),
];

const SHOWCASE_EDGES: [(Node, Node, u64, bool); 24] = [
    // The efficient route deliberately revisits the library after collecting
    // the key. Search must reason over position, inventory, and gate state;
    // room alone is not a sufficient state key.
    (Node::Start, Node::Atrium, 1, false),
    (Node::Atrium, Node::Library, 1, false),
    (Node::Library, Node::KeyRoom, 1, false),
    (Node::KeyRoom, Node::Library, 1, false),
    (Node::Library, Node::Gallery, 1, false),
    (Node::Atrium, Node::Gallery, 4, false),
    (Node::Gallery, Node::Gate, 1, false),
    (Node::Gate, Node::Vault, 1, true),
    (Node::Vault, Node::Exit, 1, false),
    // A balanced route through the public district.
    (Node::Start, Node::Garden, 3, false),
    (Node::Garden, Node::Market, 2, false),
    (Node::Market, Node::Canal, 2, false),
    (Node::Canal, Node::Bridge, 2, false),
    (Node::Bridge, Node::Exit, 2, false),
    // Cross-links and reversible passages create decisions after the entrance
    // without making the onboarding map visually noisy.
    (Node::Market, Node::Gallery, 3, false),
    (Node::Gallery, Node::Market, 2, false),
    (Node::Garden, Node::Workshop, 3, false),
    (Node::Workshop, Node::Canal, 2, false),
    (Node::Start, Node::Tunnel, 6, false),
    (Node::Tunnel, Node::Workshop, 3, false),
    (Node::Tunnel, Node::Bridge, 4, false),
    (Node::Bridge, Node::Canal, 2, false),
    // The shortest sequence is intentionally the most expensive route.
    (Node::Start, Node::Detour, 8, false),
    (Node::Detour, Node::Exit, 7, false),
];

const STRESS_EDGES: [(Node, Node, u64, bool); 40] = [
    SHOWCASE_EDGES[0],
    SHOWCASE_EDGES[1],
    SHOWCASE_EDGES[2],
    SHOWCASE_EDGES[3],
    SHOWCASE_EDGES[4],
    SHOWCASE_EDGES[5],
    SHOWCASE_EDGES[6],
    SHOWCASE_EDGES[7],
    SHOWCASE_EDGES[8],
    SHOWCASE_EDGES[9],
    SHOWCASE_EDGES[10],
    SHOWCASE_EDGES[11],
    SHOWCASE_EDGES[12],
    SHOWCASE_EDGES[13],
    SHOWCASE_EDGES[14],
    SHOWCASE_EDGES[15],
    SHOWCASE_EDGES[16],
    SHOWCASE_EDGES[17],
    SHOWCASE_EDGES[18],
    SHOWCASE_EDGES[19],
    SHOWCASE_EDGES[20],
    SHOWCASE_EDGES[21],
    SHOWCASE_EDGES[22],
    SHOWCASE_EDGES[23],
    (Node::Atrium, Node::Archive, 2, false),
    (Node::Archive, Node::Scriptorium, 1, false),
    (Node::Scriptorium, Node::KeyRoom, 1, false),
    (Node::KeyRoom, Node::Scriptorium, 1, false),
    (Node::Scriptorium, Node::Archive, 1, false),
    (Node::Archive, Node::Gallery, 2, false),
    (Node::Market, Node::Docks, 2, false),
    (Node::Docks, Node::Foundry, 2, false),
    (Node::Foundry, Node::Tower, 2, false),
    (Node::Tower, Node::Observatory, 2, false),
    (Node::Observatory, Node::Exit, 3, false),
    (Node::Foundry, Node::Canal, 2, false),
    (Node::Tunnel, Node::Ridge, 2, false),
    (Node::Ridge, Node::Ruins, 2, false),
    (Node::Ruins, Node::Chapel, 2, false),
    (Node::Chapel, Node::Bridge, 2, false),
];

/// Builds the complete closed problem. Topology, lock state, energy, target,
/// and heuristic values are all assets held by accounts.
pub fn initial() -> World {
    build(
        &MICRO_EDGES,
        9,
        6,
        &[
            (Node::Start, 6),
            (Node::KeyRoom, 4),
            (Node::Gate, 2),
            (Node::Detour, 5),
            (Node::Exit, 0),
        ],
        Node::KeyRoom,
        Node::KeyRoom,
    )
}

/// A compact cyclic maze whose room, inventory, and gate state all matter.
pub fn initial_showcase() -> World {
    build(
        &SHOWCASE_EDGES,
        18,
        14,
        &[
            (Node::Start, 6),
            (Node::Atrium, 5),
            (Node::Library, 4),
            (Node::KeyRoom, 5),
            (Node::Gallery, 3),
            (Node::Gate, 2),
            (Node::Vault, 1),
            (Node::Garden, 8),
            (Node::Market, 6),
            (Node::Canal, 4),
            (Node::Bridge, 2),
            (Node::Workshop, 6),
            (Node::Tunnel, 6),
            (Node::Detour, 7),
            (Node::Exit, 0),
        ],
        Node::KeyRoom,
        Node::Gate,
    )
}

/// The showcase topology plus a second cyclic district that substantially
/// increases the exact-search frontier without changing the problem's rules.
pub fn initial_stress() -> World {
    build(
        &STRESS_EDGES,
        24,
        18,
        &[
            (Node::Start, 6),
            (Node::Atrium, 5),
            (Node::Library, 4),
            (Node::Gallery, 3),
            (Node::Archive, 5),
            (Node::Scriptorium, 6),
            (Node::KeyRoom, 5),
            (Node::Gate, 2),
            (Node::Vault, 1),
            (Node::Garden, 8),
            (Node::Market, 6),
            (Node::Canal, 4),
            (Node::Docks, 8),
            (Node::Foundry, 6),
            (Node::Tower, 5),
            (Node::Observatory, 3),
            (Node::Workshop, 6),
            (Node::Tunnel, 6),
            (Node::Ridge, 8),
            (Node::Ruins, 6),
            (Node::Bridge, 2),
            (Node::Chapel, 4),
            (Node::Detour, 7),
            (Node::Exit, 0),
        ],
        Node::KeyRoom,
        Node::Gate,
    )
}

fn build(
    edges: &[(Node, Node, u64, bool)],
    energy: u64,
    time: u64,
    distances: &[(Node, u64)],
    key_at: Node,
    unlock_at: Node,
) -> World {
    let mut environment = axionomy::Basket::new();
    for (from, to, _, _) in edges {
        environment.insert(Asset::Edge(*from, *to), Quantity::new(1));
    }
    environment.insert(Asset::Key, Quantity::new(1));
    environment.insert(Asset::Locked, Quantity::new(1));
    environment.insert(Asset::Target(Node::Exit), Quantity::new(1));
    for (node, distance) in distances {
        environment.insert(Asset::Distance(*node), Quantity::new(*distance));
    }

    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent,
            Account::from(basket([
                (Asset::At(Node::Start), 1),
                (Asset::Energy, energy),
                (Asset::Time, time),
                (Asset::Active, 1),
            ])),
        )
        .account(AccountId::World, Account::from(environment))
        .invariant(nodes_from_edges(edges).into_iter().fold(
            LinearInvariant::new("one agent position"),
            |invariant, node| invariant.weight(Asset::At(node), 1),
        ))
        .invariant(
            LinearInvariant::new("energy is only spent")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new("time is only spent")
                .weight(Asset::Time, 1)
                .weight(Asset::SpentTime, 1),
        )
        .invariant(
            LinearInvariant::new("maze lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        );

    for &(from, to, energy, needs_open_door) in edges {
        let mut rate = Rate::new()
            .preserve(Role::Actor, basket([(Asset::Active, 1)]))
            .consume(
                Role::Actor,
                basket([
                    (Asset::At(from), 1),
                    (Asset::Energy, energy),
                    (Asset::Time, 1),
                ]),
            )
            .produce(
                Role::Actor,
                basket([
                    (Asset::At(to), 1),
                    (Asset::SpentEnergy, energy),
                    (Asset::SpentTime, 1),
                ]),
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
            RateId::TakeKey { at: key_at },
            Rate::new()
                .preserve(
                    Role::Actor,
                    basket([(Asset::At(key_at), 1), (Asset::Active, 1)]),
                )
                .consume(Role::Actor, basket([(Asset::Time, 1)]))
                .produce(Role::Actor, basket([(Asset::SpentTime, 1)]))
                .consume(Role::Environment, basket([(Asset::Key, 1)]))
                .produce(Role::Actor, basket([(Asset::Key, 1)]))
                .distinct(Role::Actor, Role::Environment),
        )
        .rate(
            RateId::UnlockDoor { at: unlock_at },
            Rate::new()
                .preserve(
                    Role::Actor,
                    basket([(Asset::At(unlock_at), 1), (Asset::Active, 1)]),
                )
                .consume(Role::Actor, basket([(Asset::Key, 1), (Asset::Time, 1)]))
                .produce(Role::Actor, basket([(Asset::SpentTime, 1)]))
                .consume(Role::Environment, basket([(Asset::Locked, 1)]))
                .produce(Role::Environment, basket([(Asset::Open, 1)]))
                .distinct(Role::Actor, Role::Environment),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Actor, basket([(Asset::At(Node::Exit), 1)]))
                .preserve(Role::Environment, basket([(Asset::Target(Node::Exit), 1)]))
                .consume(Role::Actor, basket([(Asset::Active, 1), (Asset::Time, 1)]))
                .produce(
                    Role::Actor,
                    basket([(Asset::Solved, 1), (Asset::SpentTime, 1)]),
                ),
        )
        .build()
        .expect("maze model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Agent, basket([(Asset::Solved, 1)]))
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

/// Exhaustively exposes the valid energy/time tradeoffs as replayable traces.
pub fn pareto_front(world: &World) -> Result<ParetoResult, ParetoError> {
    pareto::search(world, &goal(), candidates, objectives)
}

pub fn objectives(world: &World) -> ObjectiveVector<ObjectiveKey, u64> {
    ObjectiveVector::try_new([
        Objective::minimize(ObjectiveKey::Energy, spent_energy(world)),
        Objective::minimize(ObjectiveKey::Time, spent_time(world)),
    ])
    .expect("maze objective schema is static and unique")
}

pub fn spent_energy(world: &World) -> u64 {
    world.balance(&AccountId::Agent, &Asset::SpentEnergy).get()
}

pub fn spent_time(world: &World) -> u64 {
    world.balance(&AccountId::Agent, &Asset::SpentTime).get()
}

pub fn heuristic(world: &World) -> u64 {
    position(world).map_or(0, |node| {
        world
            .balance(&AccountId::World, &Asset::Distance(node))
            .get()
    })
}

/// Returns the Explorer's authoritative room, if the lifecycle still carries
/// a position token.
pub fn position(world: &World) -> Option<Node> {
    nodes(world).into_iter().find(|node| {
        !world
            .balance(&AccountId::Agent, &Asset::At(*node))
            .is_zero()
    })
}

pub fn has_key(world: &World) -> bool {
    !world.balance(&AccountId::Agent, &Asset::Key).is_zero()
}

pub fn gate_is_open(world: &World) -> bool {
    !world.balance(&AccountId::World, &Asset::Open).is_zero()
}

/// Returns the topology nodes encoded by the world's edge assets.
pub fn nodes(world: &World) -> Vec<Node> {
    let edges = world.rate_ids().filter_map(|rate| match rate {
        RateId::Move { from, to, .. } => Some((*from, *to)),
        _ => None,
    });
    let mut nodes = std::collections::BTreeSet::new();
    for (from, to) in edges {
        nodes.insert(from);
        nodes.insert(to);
    }
    nodes.into_iter().collect()
}

fn nodes_from_edges(edges: &[(Node, Node, u64, bool)]) -> Vec<Node> {
    let mut nodes = std::collections::BTreeSet::new();
    for (from, to, _, _) in edges {
        nodes.insert(*from);
        nodes.insert(*to);
    }
    nodes.into_iter().collect()
}

pub fn move_energy(before: &World, _: &Action, after: &World) -> u64 {
    spent_energy(after).saturating_sub(spent_energy(before))
}

fn action(rate: RateId) -> Action {
    Exchange::new(rate, Quantity::new(1))
        .bind(Role::Actor, AccountId::Agent)
        .bind(Role::Environment, AccountId::World)
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
            assert!(candidates(&replay).is_empty());
        }
    }

    #[test]
    fn actor_and_environment_roles_cannot_be_swapped() {
        let world = initial();
        let rebound = Exchange::new(
            RateId::Move {
                from: Node::Start,
                to: Node::KeyRoom,
                energy: 2,
                needs_open_door: false,
            },
            Quantity::new(1),
        )
        .bind(Role::Actor, AccountId::World)
        .bind(Role::Environment, AccountId::Agent);

        assert!(!world.is_applicable(&rebound));
    }

    #[test]
    fn pareto_front_preserves_both_encoded_routes_and_replays_them() {
        let initial = initial();
        let result = pareto_front(&initial).unwrap();
        let mut outcomes = Vec::new();

        for entry in result.front().entries() {
            let replayed = initial.replayed(entry.payload()).unwrap();
            assert!(replayed.matches(&goal()));
            assert_eq!(&objectives(&replayed), entry.objectives());
            outcomes.push((spent_energy(&replayed), spent_time(&replayed)));
        }

        outcomes.sort_unstable();
        assert_eq!(outcomes, [(6, 6), (9, 3)]);
    }

    #[test]
    fn stress_profile_adds_routes_and_keeps_search_replayable() {
        let showcase = initial_showcase();
        let stress = initial_stress();
        assert!(nodes(&stress).len() > nodes(&showcase).len());
        assert!(stress.rate_ids().count() > showcase.rate_ids().count());

        for solution in [
            solve_bfs(&stress).expect("stress maze has a shortest route"),
            solve_astar(&stress).expect("stress maze has an energy route"),
        ] {
            let replayed = stress
                .replayed(solution.trace())
                .expect("stress route must replay");
            assert!(replayed.matches(&goal()));
        }

        let frontier = pareto_front(&stress).expect("stress frontier is finite");
        assert!(frontier.front().len() >= 2);
    }

    #[test]
    fn showcase_has_four_exact_tradeoffs_and_guidance_reduces_search() {
        let world = initial_showcase();
        let dijkstra = solve_dijkstra(&world).expect("showcase has an energy-optimal route");
        let astar = solve_astar(&world).expect("showcase has a guided energy-optimal route");
        assert_eq!(dijkstra.cost(), 8);
        assert_eq!(astar.cost(), dijkstra.cost());
        assert!(astar.expanded() < dijkstra.expanded());

        let mut outcomes = pareto_front(&world)
            .unwrap()
            .front()
            .entries()
            .iter()
            .map(|entry| {
                let replayed = world.replayed(entry.payload()).unwrap();
                assert!(replayed.matches(&goal()));
                (spent_energy(&replayed), spent_time(&replayed))
            })
            .collect::<Vec<_>>();
        outcomes.sort_unstable();
        assert_eq!(outcomes, [(8, 11), (11, 6), (12, 4), (15, 3)]);
    }

    #[test]
    fn gate_cannot_be_unlocked_remotely() {
        let mut world = initial_showcase();
        for rate in [
            RateId::Move {
                from: Node::Start,
                to: Node::Atrium,
                energy: 1,
                needs_open_door: false,
            },
            RateId::Move {
                from: Node::Atrium,
                to: Node::Library,
                energy: 1,
                needs_open_door: false,
            },
            RateId::Move {
                from: Node::Library,
                to: Node::KeyRoom,
                energy: 1,
                needs_open_door: false,
            },
            RateId::TakeKey { at: Node::KeyRoom },
        ] {
            world.apply(action(rate)).unwrap();
        }

        let remote_unlock = action(RateId::UnlockDoor { at: Node::Gate });
        let assessment = world.assess(&remote_unlock);
        assert!(!assessment.is_applicable());
        assert_eq!(
            assessment
                .shortfalls()
                .iter()
                .flat_map(|shortfall| shortfall.missing().iter())
                .find(|(asset, _)| *asset == &Asset::At(Node::Gate))
                .map(|(_, quantity)| quantity.get()),
            Some(1),
        );
    }
}
