#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Pattern, Quantity, Rate,
    basket,
};
mod dynamics;
#[cfg(feature = "packages")]
pub mod package;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Agent,
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    At(Node),
    Edge(Node, Node),
    Key,
    PowerTransit,
    PowerVault,
    Health,
    Injury,
    Shield,
    UsedShield,
    Medicine,
    UsedMedicine,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Actor,
    Environment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    DivertPower {
        at: Node,
        to_vault: bool,
    },
    TakeProtection {
        at: Node,
    },
    Treat {
        at: Node,
    },
    ProtectedMove {
        from: Node,
        to: Node,
        energy: u64,
    },
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

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `maze` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "maze";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

const MICRO_EDGES: [(Node, Node, u64, bool); 5] = [
    (Node::Start, Node::KeyRoom, 2, false),
    (Node::KeyRoom, Node::Gate, 2, true),
    (Node::Gate, Node::Exit, 2, false),
    (Node::Start, Node::Detour, 4, false),
    (Node::Detour, Node::Exit, 5, false),
];

const SHOWCASE_EDGES: [(Node, Node, u64, bool); 25] = [
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
    (Node::Workshop, Node::Garden, 1, false),
];

const STRESS_EDGES: [(Node, Node, u64, bool); 42] = [
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
    SHOWCASE_EDGES[24],
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
    (Node::Ruins, Node::Garden, 2, false),
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
    dynamics::build(build(
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
    ))
}

/// The showcase topology plus a second cyclic district that substantially
/// increases the exact-search frontier without changing the problem's rules.
pub fn initial_stress() -> World {
    dynamics::build(build(
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
    ))
}

/// A law of the maze: the Explorer acts on the world, each role fixed to its
/// account.
fn law() -> Rate<Ontology> {
    Rate::new()
        .account(Role::Actor, Pattern::literal(AccountId::Agent))
        .account(Role::Environment, Pattern::literal(AccountId::World))
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
            LinearInvariant::new(path!("one-agent-position")),
            |invariant, node| invariant.weight(Asset::At(node), 1),
        ))
        .invariant(
            LinearInvariant::new(path!("energy-is-only-spent"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("time-is-only-spent"))
                .weight(Asset::Time, 1)
                .weight(Asset::SpentTime, 1),
        )
        .invariant(
            LinearInvariant::new(path!("maze-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        );

    for &(from, to, energy, needs_open_door) in edges {
        let mut rate = law()
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
            .preserve(Role::Environment, basket([(Asset::Edge(from, to), 1)]));
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
            law()
                .preserve(
                    Role::Actor,
                    basket([(Asset::At(key_at), 1), (Asset::Active, 1)]),
                )
                .consume(Role::Actor, basket([(Asset::Time, 1)]))
                .produce(Role::Actor, basket([(Asset::SpentTime, 1)]))
                .consume(Role::Environment, basket([(Asset::Key, 1)]))
                .produce(Role::Actor, basket([(Asset::Key, 1)])),
        )
        .rate(
            RateId::UnlockDoor { at: unlock_at },
            law()
                .preserve(
                    Role::Actor,
                    basket([(Asset::At(unlock_at), 1), (Asset::Active, 1)]),
                )
                .consume(Role::Actor, basket([(Asset::Key, 1), (Asset::Time, 1)]))
                .produce(Role::Actor, basket([(Asset::SpentTime, 1)]))
                .consume(Role::Environment, basket([(Asset::Locked, 1)]))
                .produce(Role::Environment, basket([(Asset::Open, 1)])),
        )
        .rate(
            RateId::Finish,
            law()
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

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Agent, basket([(Asset::Solved, 1)]))
}

fn nodes_from_edges(edges: &[(Node, Node, u64, bool)]) -> Vec<Node> {
    let mut nodes = std::collections::BTreeSet::new();
    for (from, to, _, _) in edges {
        nodes.insert(*from);
        nodes.insert(*to);
    }
    nodes.into_iter().collect()
}

/// The exchange applying `rate` once: the Explorer acting on the world.
pub fn action(rate: RateId) -> Action {
    Exchange::new(rate, Quantity::new(1))
        .bind(Role::Actor, AccountId::Agent)
        .bind(Role::Environment, AccountId::World)
}

/// The rooms the world's passages join.
#[cfg(test)]
fn nodes(world: &World) -> Vec<Node> {
    let edges = world
        .rate_ids()
        .filter_map(|rate| rate.as_literal())
        .filter_map(|rate| match rate {
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

/// The room the Explorer stands in.
#[cfg(test)]
fn position(world: &World) -> Option<Node> {
    nodes(world).into_iter().find(|node| {
        !world
            .balance(&AccountId::Agent, &Asset::At(*node))
            .is_zero()
    })
}

#[cfg(test)]
fn spent_energy(world: &World) -> u64 {
    world
        .balance(&AccountId::Agent, &Asset::SpentEnergy)
        .to_u64()
}

#[cfg(test)]
fn spent_time(world: &World) -> u64 {
    world.balance(&AccountId::Agent, &Asset::SpentTime).to_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `world` after the Explorer plays `rates`, one exchange each.
    fn played(world: &World, rates: &[RateId]) -> World {
        world
            .replayed(&rates.iter().copied().map(action).collect())
            .expect("the route replays")
    }

    #[test]
    fn the_key_route_spends_less_energy_and_the_detour_less_time() {
        let key_route = [
            RateId::Move {
                from: Node::Start,
                to: Node::KeyRoom,
                energy: 2,
                needs_open_door: false,
            },
            RateId::TakeKey { at: Node::KeyRoom },
            RateId::UnlockDoor { at: Node::KeyRoom },
            RateId::Move {
                from: Node::KeyRoom,
                to: Node::Gate,
                energy: 2,
                needs_open_door: true,
            },
            RateId::Move {
                from: Node::Gate,
                to: Node::Exit,
                energy: 2,
                needs_open_door: false,
            },
            RateId::Finish,
        ];
        let detour = [
            RateId::Move {
                from: Node::Start,
                to: Node::Detour,
                energy: 4,
                needs_open_door: false,
            },
            RateId::Move {
                from: Node::Detour,
                to: Node::Exit,
                energy: 5,
                needs_open_door: false,
            },
            RateId::Finish,
        ];
        for (route, energy, time) in [(&key_route[..], 6, 6), (&detour[..], 9, 3)] {
            let solved = played(&initial(), route);
            assert!(solved.matches(&goal()));
            assert_eq!(spent_energy(&solved), energy);
            assert_eq!(spent_time(&solved), time);
            // A solved maze has no move left.
            assert!(
                solved
                    .rate_ids()
                    .filter_map(|rate| rate.as_literal())
                    .all(|rate| !solved.is_applicable(&action(*rate)))
            );
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
    fn stress_adds_routes_to_the_showcase_district() {
        let showcase = initial_showcase();
        let stress = initial_stress();
        assert!(nodes(&stress).len() > nodes(&showcase).len());
        assert!(stress.rate_ids().count() > showcase.rate_ids().count());

        let detour = played(
            &stress,
            &[
                RateId::Move {
                    from: Node::Start,
                    to: Node::Detour,
                    energy: 8,
                    needs_open_door: false,
                },
                RateId::Move {
                    from: Node::Detour,
                    to: Node::Exit,
                    energy: 7,
                    needs_open_door: false,
                },
                RateId::Finish,
            ],
        );
        assert!(detour.matches(&goal()));
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
                .map(|(_, quantity)| quantity.to_u64()),
            Some(1),
        );
    }
}
