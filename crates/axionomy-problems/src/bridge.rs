//! Multi-agent negotiation for a capacity-one bridge.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, Trace,
    basket,
};
use axionomy_search::{SearchSolution, bfs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentId {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    West,
    East,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Agent(AgentId),
    Bridge,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    AgentIdentity(AgentId),
    BridgeIdentity,
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
    Waiting,
    Crossed,
    CapacityFree,
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Traveler,
    Winner,
    Loser,
    Bridge,
    AgentA,
    AgentB,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    YieldToWaiting {
        agent: AgentId,
    },
    Cross {
        agent: AgentId,
    },
    Finish,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;
pub type Proposal = Trace<RateId, Role, AccountId>;

pub fn initial() -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent(AgentId::A),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::A), 1),
                (Asset::At(Side::West), 1),
                (Asset::Energy, 1),
                (Asset::Credit, 2),
                (Asset::CanBid, 1),
            ])),
        )
        .account(
            AccountId::Agent(AgentId::B),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::B), 1),
                (Asset::At(Side::West), 1),
                (Asset::Energy, 1),
                (Asset::Credit, 2),
                (Asset::CanBid, 1),
            ])),
        )
        .account(
            AccountId::Bridge,
            Account::from(basket([
                (Asset::BridgeIdentity, 1),
                (Asset::CapacityFree, 1),
            ])),
        )
        .account(
            AccountId::Success,
            Account::from(basket([(Asset::Active, 1)])),
        );

    for agent in [AgentId::A, AgentId::B] {
        for amount in 1..=2 {
            builder = builder.rate(
                RateId::SubmitBid { agent, amount },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .consume(
                        Role::Traveler,
                        basket([(Asset::CanBid, 1), (Asset::Credit, u64::from(amount))]),
                    )
                    .produce(
                        Role::Traveler,
                        basket([
                            (Asset::Submitted, 1),
                            (Asset::Bid(amount), 1),
                            (Asset::Escrow, u64::from(amount)),
                        ]),
                    ),
            );
        }
        builder = builder
            .rate(
                RateId::ClaimFirst { agent },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(Role::Bridge, basket([(Asset::BridgeIdentity, 1)]))
                    .consume(Role::Traveler, basket([(Asset::CanBid, 1)]))
                    .consume(Role::Bridge, basket([(Asset::CapacityFree, 1)]))
                    .produce(Role::Traveler, basket([(Asset::CrossingRight, 1)]))
                    .distinct(Role::Traveler, Role::Bridge),
            )
            .rate(
                RateId::YieldToWaiting { agent },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(Role::Bridge, basket([(Asset::BridgeIdentity, 1)]))
                    .consume(Role::Traveler, basket([(Asset::Waiting, 1)]))
                    .consume(Role::Bridge, basket([(Asset::CapacityFree, 1)]))
                    .produce(Role::Traveler, basket([(Asset::CrossingRight, 1)]))
                    .distinct(Role::Traveler, Role::Bridge),
            )
            .rate(
                RateId::Cross { agent },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(Role::Bridge, basket([(Asset::BridgeIdentity, 1)]))
                    .consume(
                        Role::Traveler,
                        basket([
                            (Asset::CrossingRight, 1),
                            (Asset::At(Side::West), 1),
                            (Asset::Energy, 1),
                        ]),
                    )
                    .produce(
                        Role::Traveler,
                        basket([
                            (Asset::Crossed, 1),
                            (Asset::At(Side::East), 1),
                            (Asset::SpentEnergy, 1),
                        ]),
                    )
                    .produce(Role::Bridge, basket([(Asset::CapacityFree, 1)]))
                    .distinct(Role::Traveler, Role::Bridge),
            );
    }

    for bid_a in 1..=2 {
        for bid_b in 1..=2 {
            let winner = if bid_a >= bid_b {
                AgentId::A
            } else {
                AgentId::B
            };
            let (winning_bid, losing_bid) = if winner == AgentId::A {
                (bid_a, bid_b)
            } else {
                (bid_b, bid_a)
            };
            builder = builder.rate(
                RateId::Resolve {
                    winner,
                    winning_bid,
                    losing_bid,
                },
                Rate::new()
                    .preserve(Role::Winner, basket([(Asset::AgentIdentity(winner), 1)]))
                    .preserve(
                        Role::Loser,
                        basket([(Asset::AgentIdentity(other(winner)), 1)]),
                    )
                    .preserve(Role::Bridge, basket([(Asset::BridgeIdentity, 1)]))
                    .consume(
                        Role::Winner,
                        basket([
                            (Asset::Submitted, 1),
                            (Asset::Bid(winning_bid), 1),
                            (Asset::Escrow, u64::from(winning_bid)),
                        ]),
                    )
                    .produce(
                        Role::Winner,
                        basket([
                            (Asset::CrossingRight, 1),
                            (Asset::SpentCredit, u64::from(winning_bid)),
                        ]),
                    )
                    .consume(
                        Role::Loser,
                        basket([
                            (Asset::Submitted, 1),
                            (Asset::Bid(losing_bid), 1),
                            (Asset::Escrow, u64::from(losing_bid)),
                        ]),
                    )
                    .produce(
                        Role::Loser,
                        basket([(Asset::Waiting, 1), (Asset::Credit, u64::from(losing_bid))]),
                    )
                    .consume(Role::Bridge, basket([(Asset::CapacityFree, 1)]))
                    .distinct(Role::Winner, Role::Loser)
                    .distinct(Role::Winner, Role::Bridge)
                    .distinct(Role::Loser, Role::Bridge),
            );
        }
    }

    let positions = LinearInvariant::new("two agent positions")
        .weight(Asset::At(Side::West), 1)
        .weight(Asset::At(Side::East), 1);
    let status = LinearInvariant::new("two agent status tokens")
        .weight(Asset::CanBid, 1)
        .weight(Asset::Submitted, 1)
        .weight(Asset::CrossingRight, 1)
        .weight(Asset::Waiting, 1)
        .weight(Asset::Crossed, 1);

    builder
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(
                    Role::AgentA,
                    basket([(Asset::AgentIdentity(AgentId::A), 1), (Asset::Crossed, 1)]),
                )
                .preserve(
                    Role::AgentB,
                    basket([(Asset::AgentIdentity(AgentId::B), 1), (Asset::Crossed, 1)]),
                )
                .consume(Role::Goal, basket([(Asset::Active, 1)]))
                .produce(Role::Goal, basket([(Asset::Solved, 1)]))
                .distinct(Role::AgentA, Role::AgentB)
                .distinct(Role::AgentA, Role::Goal)
                .distinct(Role::AgentB, Role::Goal),
        )
        .invariant(positions)
        .invariant(status)
        .invariant(
            LinearInvariant::new("bridge lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .invariant(
            LinearInvariant::new("bridge capacity")
                .weight(Asset::CapacityFree, 1)
                .weight(Asset::CrossingRight, 1),
        )
        .invariant(
            LinearInvariant::new("energy accounting")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new("credit accounting")
                .weight(Asset::Credit, 1)
                .weight(Asset::Escrow, 1)
                .weight(Asset::SpentCredit, 1),
        )
        .build()
        .expect("bridge model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut ids: Vec<_> = world.rate_ids().copied().collect();
    ids.sort();
    world.applicable(ids.into_iter().map(action))
}

pub fn solve(world: &World) -> Option<Solution> {
    bfs(world, &goal(), candidates)
}

pub fn first_come_proposal(first: AgentId) -> Option<Proposal> {
    let second = other(first);
    validated_trace([
        RateId::ClaimFirst { agent: first },
        RateId::Cross { agent: first },
        RateId::ClaimFirst { agent: second },
        RateId::Cross { agent: second },
        RateId::Finish,
    ])
}

pub fn auction_proposal(bid_a: u8, bid_b: u8) -> Option<Proposal> {
    let winner = if bid_a >= bid_b {
        AgentId::A
    } else {
        AgentId::B
    };
    let loser = other(winner);
    let (winning_bid, losing_bid) = if winner == AgentId::A {
        (bid_a, bid_b)
    } else {
        (bid_b, bid_a)
    };
    validated_trace([
        RateId::SubmitBid {
            agent: AgentId::A,
            amount: bid_a,
        },
        RateId::SubmitBid {
            agent: AgentId::B,
            amount: bid_b,
        },
        RateId::Resolve {
            winner,
            winning_bid,
            losing_bid,
        },
        RateId::Cross { agent: winner },
        RateId::YieldToWaiting { agent: loser },
        RateId::Cross { agent: loser },
        RateId::Finish,
    ])
}

fn validated_trace<const N: usize>(rates: [RateId; N]) -> Option<Proposal> {
    let mut world = initial();
    let mut trace = Trace::new();
    for rate in rates {
        let exchange = action(rate);
        world.apply(exchange.clone()).ok()?;
        trace.push(exchange);
    }
    world.matches(&goal()).then_some(trace)
}

fn other(agent: AgentId) -> AgentId {
    match agent {
        AgentId::A => AgentId::B,
        AgentId::B => AgentId::A,
    }
}

pub fn action(rate: RateId) -> Action {
    match rate {
        RateId::SubmitBid { agent, .. }
        | RateId::ClaimFirst { agent }
        | RateId::YieldToWaiting { agent }
        | RateId::Cross { agent } => {
            let exchange =
                Exchange::new(rate, Quantity::new(1)).bind(Role::Traveler, AccountId::Agent(agent));
            if matches!(
                rate,
                RateId::ClaimFirst { .. } | RateId::YieldToWaiting { .. } | RateId::Cross { .. }
            ) {
                exchange.bind(Role::Bridge, AccountId::Bridge)
            } else {
                exchange
            }
        }
        RateId::Resolve { winner, .. } => Exchange::new(rate, Quantity::new(1))
            .bind(Role::Winner, AccountId::Agent(winner))
            .bind(Role::Loser, AccountId::Agent(other(winner)))
            .bind(Role::Bridge, AccountId::Bridge),
        RateId::Finish => Exchange::new(rate, Quantity::new(1))
            .bind(Role::AgentA, AccountId::Agent(AgentId::A))
            .bind(Role::AgentB, AccountId::Agent(AgentId::B))
            .bind(Role::Goal, AccountId::Success),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auction_and_first_come_are_both_valid_mechanisms() {
        let first_come = first_come_proposal(AgentId::B).expect("mechanism is feasible");
        let auction = auction_proposal(2, 1).expect("mechanism is feasible");
        assert_eq!(first_come.exchanges().len(), 5);
        assert_eq!(auction.exchanges().len(), 7);
        assert!(auction.exchanges().iter().any(|exchange| {
            matches!(
                exchange.rate(),
                RateId::Resolve {
                    winner: AgentId::A,
                    ..
                }
            )
        }));

        for trace in [&first_come, &auction] {
            let mut replay = initial();
            replay.replay(trace).expect("mechanism must replay");
            assert!(replay.matches(&goal()));
        }
    }

    #[test]
    fn capacity_prevents_two_simultaneous_rights_atomically() {
        let mut world = initial();
        world
            .apply(action(RateId::ClaimFirst { agent: AgentId::A }))
            .expect("first claim succeeds");
        let before = world.state_key();
        assert!(
            world
                .apply(action(RateId::ClaimFirst { agent: AgentId::B }))
                .is_err()
        );
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn encoded_identity_rejects_bidder_impersonation() {
        let world = initial();
        let impersonation = Exchange::new(
            RateId::SubmitBid {
                agent: AgentId::A,
                amount: 1,
            },
            Quantity::new(1),
        )
        .bind(Role::Traveler, AccountId::Agent(AgentId::B));

        assert!(!world.is_applicable(&impersonation));
    }

    #[test]
    fn encoded_identity_rejects_wrong_auction_winner() {
        let mut world = initial();
        world
            .apply(action(RateId::SubmitBid {
                agent: AgentId::A,
                amount: 2,
            }))
            .expect("A submits");
        world
            .apply(action(RateId::SubmitBid {
                agent: AgentId::B,
                amount: 1,
            }))
            .expect("B submits");
        let swapped = Exchange::new(
            RateId::Resolve {
                winner: AgentId::A,
                winning_bid: 2,
                losing_bid: 1,
            },
            Quantity::new(1),
        )
        .bind(Role::Winner, AccountId::Agent(AgentId::B))
        .bind(Role::Loser, AccountId::Agent(AgentId::A))
        .bind(Role::Bridge, AccountId::Bridge);

        assert!(!world.is_applicable(&swapped));
    }
}
