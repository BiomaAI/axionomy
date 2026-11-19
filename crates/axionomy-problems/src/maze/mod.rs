#![doc = include_str!("README.md")]

use axionomy::{Economy, Exchange, Goal, Quantity, basket};
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

/// Builds the complete closed problem. Topology, lock state, energy, target,
/// and heuristic values are all assets held by accounts.
pub fn initial() -> World {
    crate::fixtures::initial(axionomy::path!("maze/micro"))
}

/// A compact cyclic maze whose room, inventory, and gate state all matter.
pub fn initial_showcase() -> World {
    crate::fixtures::initial(axionomy::path!("maze/showcase"))
}

/// The showcase topology plus a second cyclic district that substantially
/// increases the exact-search frontier without changing the problem's rules.
pub fn initial_stress() -> World {
    crate::fixtures::initial(axionomy::path!("maze/stress"))
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Agent, basket([(Asset::Solved, 1)]))
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
