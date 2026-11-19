#![doc = include_str!("README.md")]

use axionomy::{Economy, Exchange, Goal, Quantity, basket};

#[cfg(feature = "packages")]
pub mod package;
pub mod timed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AgentId {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Side {
    West,
    East,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Agent(AgentId),
    Bridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    At(Side),
    Energy,
    SpentEnergy,
    Credit,
    Escrow,
    SpentCredit,
    Bid(u8),
    CanBid,
    Submitted,
    CrossingRight,
    FirstTurn,
    SecondTurn,
    PriorityBenefit,
    Waiting,
    Crossed,
    CompletedTrip,
    RoundsRemaining,
    CapacityFree,
    Active,
    Solved,
    Now,
    TimeRemaining,
    Arrival,
    Deadline,
    LeaseUntil,
    ReadyAt,
    InTransit,
    OnDetour,
    Detouring,
    OnTime,
    Late,
    MissedReservation,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Traveler,
    Winner,
    Loser,
    Bridge,
    AgentA,
    AgentB,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    SubmitBid {
        agent: AgentId,
        amount: u8,
    },
    Resolve {
        winner: AgentId,
        winning_bid: u8,
        losing_bid: u8,
    },
    ClaimFirst {
        agent: AgentId,
    },
    ClaimSecond {
        agent: AgentId,
    },
    YieldToWaiting {
        agent: AgentId,
    },
    Cross {
        agent: AgentId,
    },
    ResetRound,
    Finish,
    Advance {
        to: u8,
    },
    Complete {
        agent: AgentId,
        detour: bool,
        late: bool,
    },
    Detour {
        agent: AgentId,
        waiting: bool,
    },
    ExpireReservation {
        agent: AgentId,
    },
    WithdrawBid {
        agent: AgentId,
        amount: u8,
    },
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `bridge` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "bridge";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn initial() -> World {
    crate::fixtures::initial(axionomy::path!("bridge/micro"))
}

/// Two consecutive capacity allocations force policies to reason about
/// repeated fairness, retained credit, and an atomic multi-agent round reset.
pub fn initial_showcase() -> World {
    crate::fixtures::initial(axionomy::path!("bridge/showcase"))
}

/// Three consecutive allocations make retained credit and repeated priority
/// materially affect later rounds while keeping the state space finite.
pub fn initial_stress() -> World {
    crate::fixtures::initial(axionomy::path!("bridge/stress"))
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Bridge, basket([(Asset::Solved, 1)]))
}

/// The exchange of one unit of `rate`, each role bound to the account
/// `world`'s rate fixes it to: a traveller's own move to its account and the
/// bridge, a resolution to its winner and loser, a round's reset or the
/// finish to both travellers. Bound to nothing where `rate` names no
/// instance of `world`'s rates, so that `world` refuses it.
pub fn action(world: &World, rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    let Ok(instance) = world.instance(&rate) else {
        return exchange;
    };
    instance
        .fixed_accounts()
        .fold(exchange, |exchange, (role, account)| {
            exchange.bind(
                *role,
                *account
                    .as_literal()
                    .expect("an instance's accounts are written out"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `world` after each of `rates` in turn.
    fn replayed(world: &World, rates: &[RateId]) -> Option<World> {
        world
            .replayed(&rates.iter().map(|rate| action(world, *rate)).collect())
            .ok()
    }

    #[test]
    fn auction_and_first_come_are_both_valid_mechanisms() {
        let first_come = [
            RateId::ClaimFirst { agent: AgentId::B },
            RateId::Cross { agent: AgentId::B },
            RateId::ClaimSecond { agent: AgentId::A },
            RateId::Cross { agent: AgentId::A },
            RateId::Finish,
        ];
        let auction = [
            RateId::SubmitBid {
                agent: AgentId::A,
                amount: 2,
            },
            RateId::SubmitBid {
                agent: AgentId::B,
                amount: 1,
            },
            RateId::Resolve {
                winner: AgentId::A,
                winning_bid: 2,
                losing_bid: 1,
            },
            RateId::Cross { agent: AgentId::A },
            RateId::YieldToWaiting { agent: AgentId::B },
            RateId::Cross { agent: AgentId::B },
            RateId::Finish,
        ];
        // A bid of A=2 and B=1 resolves to A, never to B.
        let bid = replayed(&initial(), &auction[..2]).unwrap();
        let to_b = RateId::Resolve {
            winner: AgentId::B,
            winning_bid: 2,
            losing_bid: 1,
        };
        assert!(!bid.is_applicable(&action(&bid, to_b)));

        for rates in [&first_come[..], &auction[..]] {
            let settled = replayed(&initial(), rates).expect("the mechanism replays");
            assert!(settled.matches(&goal()));
            assert!(
                settled
                    .rate_ids()
                    .flat_map(|key| settled.instances_of(key))
                    .all(|rate| !settled.is_applicable(&action(&settled, rate)))
            );
        }
    }

    #[test]
    fn capacity_prevents_two_simultaneous_rights_atomically() {
        let mut world = initial();
        world
            .apply(action(&world, RateId::ClaimFirst { agent: AgentId::A }))
            .expect("first claim succeeds");
        let before = world.state_key();
        assert!(
            world
                .apply(action(&world, RateId::ClaimFirst { agent: AgentId::B }))
                .is_err()
        );
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn a_bid_binds_the_traveller_it_names() {
        let world = initial();
        let impersonation = action(
            &world,
            RateId::SubmitBid {
                agent: AgentId::A,
                amount: 1,
            },
        )
        .bind(Role::Traveler, AccountId::Agent(AgentId::B));

        assert!(!world.is_applicable(&impersonation));
    }

    #[test]
    fn a_resolution_binds_the_winner_it_names_and_the_other_as_loser() {
        let bids = [
            RateId::SubmitBid {
                agent: AgentId::A,
                amount: 2,
            },
            RateId::SubmitBid {
                agent: AgentId::B,
                amount: 1,
            },
        ];
        let world = replayed(&initial(), &bids).expect("both submit");
        let to_a = RateId::Resolve {
            winner: AgentId::A,
            winning_bid: 2,
            losing_bid: 1,
        };
        let swapped = action(&world, to_a)
            .bind(Role::Winner, AccountId::Agent(AgentId::B))
            .bind(Role::Loser, AccountId::Agent(AgentId::A));

        assert!(!world.is_applicable(&swapped));
        assert!(world.is_applicable(&action(&world, to_a)));
        // No identifier names a winner who bid less.
        assert!(
            world
                .instance(&RateId::Resolve {
                    winner: AgentId::B,
                    winning_bid: 1,
                    losing_bid: 2,
                })
                .is_err()
        );
    }
}
