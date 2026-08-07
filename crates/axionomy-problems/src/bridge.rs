//! Multi-agent negotiation for a capacity-one bridge.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, Trace,
    basket,
};
use axionomy_search::{
    SearchSolution, bfs,
    pareto::{self, Objective, ObjectiveVector, ParetoError, ParetoSearchResult},
};

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
    FirstTurn,
    SecondTurn,
    PriorityBenefit,
    Waiting,
    Crossed,
    CompletedTrip,
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
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type Solution = SearchSolution<RateId, Role, AccountId>;
pub type Proposal = Trace<RateId, Role, AccountId>;
pub type ParetoResult = ParetoSearchResult<RateId, Role, AccountId, u64, ObjectiveKey, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveKey {
    Priority(AgentId),
    Credit(AgentId),
}

pub fn initial() -> World {
    build(false)
}

/// Two consecutive capacity allocations force policies to reason about
/// repeated fairness, retained credit, and an atomic multi-agent round reset.
pub fn initial_showcase() -> World {
    build(true)
}

fn build(repeated: bool) -> World {
    let credit = if repeated { 4 } else { 2 };
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent(AgentId::A),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::A), 1),
                (Asset::At(Side::West), 1),
                (Asset::Energy, 1),
                (Asset::Credit, credit),
                (Asset::CanBid, 1),
            ])),
        )
        .account(
            AccountId::Agent(AgentId::B),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::B), 1),
                (Asset::At(Side::West), 1),
                (Asset::Energy, 1),
                (Asset::Credit, credit),
                (Asset::CanBid, 1),
            ])),
        )
        .account(
            AccountId::Bridge,
            Account::from(basket([
                (Asset::BridgeIdentity, 1),
                (Asset::CapacityFree, 1),
                (Asset::FirstTurn, 1),
                (Asset::Active, 1),
            ])),
        );

    for agent in [AgentId::A, AgentId::B] {
        for amount in 1..=2 {
            builder = builder.rate(
                RateId::SubmitBid { agent, amount },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(
                        Role::Bridge,
                        basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
                    )
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
                    )
                    .distinct(Role::Traveler, Role::Bridge),
            );
        }
        builder = builder
            .rate(
                RateId::ClaimFirst { agent },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(
                        Role::Bridge,
                        basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
                    )
                    .consume(Role::Traveler, basket([(Asset::CanBid, 1)]))
                    .consume(
                        Role::Bridge,
                        basket([(Asset::CapacityFree, 1), (Asset::FirstTurn, 1)]),
                    )
                    .produce(
                        Role::Traveler,
                        basket([(Asset::CrossingRight, 1), (Asset::PriorityBenefit, 1)]),
                    )
                    .distinct(Role::Traveler, Role::Bridge),
            )
            .rate(
                RateId::ClaimSecond { agent },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(
                        Role::Bridge,
                        basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
                    )
                    .consume(Role::Traveler, basket([(Asset::CanBid, 1)]))
                    .consume(
                        Role::Bridge,
                        basket([(Asset::CapacityFree, 1), (Asset::SecondTurn, 1)]),
                    )
                    .produce(Role::Traveler, basket([(Asset::CrossingRight, 1)]))
                    .distinct(Role::Traveler, Role::Bridge),
            )
            .rate(
                RateId::YieldToWaiting { agent },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(
                        Role::Bridge,
                        basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
                    )
                    .consume(Role::Traveler, basket([(Asset::Waiting, 1)]))
                    .consume(
                        Role::Bridge,
                        basket([(Asset::CapacityFree, 1), (Asset::SecondTurn, 1)]),
                    )
                    .produce(Role::Traveler, basket([(Asset::CrossingRight, 1)]))
                    .distinct(Role::Traveler, Role::Bridge),
            )
            .rate(
                RateId::Cross { agent },
                Rate::new()
                    .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
                    .preserve(
                        Role::Bridge,
                        basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
                    )
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
                    .produce(
                        Role::Bridge,
                        basket([(Asset::CapacityFree, 1), (Asset::SecondTurn, 1)]),
                    )
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
                    .preserve(
                        Role::Bridge,
                        basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
                    )
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
                            (Asset::PriorityBenefit, 1),
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
                    .consume(
                        Role::Bridge,
                        basket([(Asset::CapacityFree, 1), (Asset::FirstTurn, 1)]),
                    )
                    .distinct(Role::Winner, Role::Loser)
                    .distinct(Role::Winner, Role::Bridge)
                    .distinct(Role::Loser, Role::Bridge),
            );
        }
    }

    if repeated {
        builder = builder.rate(
            RateId::ResetRound,
            Rate::new()
                .preserve(
                    Role::AgentA,
                    basket([(Asset::AgentIdentity(AgentId::A), 1)]),
                )
                .preserve(
                    Role::AgentB,
                    basket([(Asset::AgentIdentity(AgentId::B), 1)]),
                )
                .preserve(
                    Role::Bridge,
                    basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
                )
                .consume(
                    Role::AgentA,
                    basket([
                        (Asset::At(Side::East), 1),
                        (Asset::Crossed, 1),
                        (Asset::SpentEnergy, 1),
                    ]),
                )
                .produce(
                    Role::AgentA,
                    basket([
                        (Asset::At(Side::West), 1),
                        (Asset::CanBid, 1),
                        (Asset::CompletedTrip, 1),
                        (Asset::Energy, 1),
                    ]),
                )
                .consume(
                    Role::AgentB,
                    basket([
                        (Asset::At(Side::East), 1),
                        (Asset::Crossed, 1),
                        (Asset::SpentEnergy, 1),
                    ]),
                )
                .produce(
                    Role::AgentB,
                    basket([
                        (Asset::At(Side::West), 1),
                        (Asset::CanBid, 1),
                        (Asset::CompletedTrip, 1),
                        (Asset::Energy, 1),
                    ]),
                )
                .consume(Role::Bridge, basket([(Asset::SecondTurn, 1)]))
                .produce(Role::Bridge, basket([(Asset::FirstTurn, 1)]))
                .distinct(Role::AgentA, Role::AgentB)
                .distinct(Role::AgentA, Role::Bridge)
                .distinct(Role::AgentB, Role::Bridge),
        );
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

    let mut finish = Rate::new()
        .preserve(
            Role::AgentA,
            basket([(Asset::AgentIdentity(AgentId::A), 1), (Asset::Crossed, 1)]),
        )
        .preserve(
            Role::AgentB,
            basket([(Asset::AgentIdentity(AgentId::B), 1), (Asset::Crossed, 1)]),
        );
    if repeated {
        finish = finish
            .preserve(Role::AgentA, basket([(Asset::CompletedTrip, 1)]))
            .preserve(Role::AgentB, basket([(Asset::CompletedTrip, 1)]));
    }
    finish = finish
        .preserve(Role::Bridge, basket([(Asset::BridgeIdentity, 1)]))
        .consume(Role::Bridge, basket([(Asset::Active, 1)]))
        .produce(Role::Bridge, basket([(Asset::Solved, 1)]))
        .distinct(Role::AgentA, Role::AgentB)
        .distinct(Role::AgentA, Role::Bridge)
        .distinct(Role::AgentB, Role::Bridge);

    builder
        .rate(RateId::Finish, finish)
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
            LinearInvariant::new("crossing turn")
                .weight(Asset::FirstTurn, 1)
                .weight(Asset::SecondTurn, 1)
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
    Goal::new().require(AccountId::Bridge, basket([(Asset::Solved, 1)]))
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut ids: Vec<_> = world.rate_ids().copied().collect();
    ids.sort();
    world.applicable(ids.into_iter().map(action))
}

pub fn solve(world: &World) -> Option<Solution> {
    bfs(world, &goal(), candidates)
}

/// Exhaustively compares valid allocation mechanisms using participant
/// priority and retained-credit assets.
pub fn pareto_front(world: &World) -> Result<ParetoResult, ParetoError> {
    pareto::search(world, &goal(), candidates, objectives)
}

pub fn objectives(world: &World) -> ObjectiveVector<ObjectiveKey, u64> {
    ObjectiveVector::try_new([
        Objective::maximize(
            ObjectiveKey::Priority(AgentId::A),
            priority(world, AgentId::A),
        ),
        Objective::maximize(
            ObjectiveKey::Priority(AgentId::B),
            priority(world, AgentId::B),
        ),
        Objective::maximize(ObjectiveKey::Credit(AgentId::A), credit(world, AgentId::A)),
        Objective::maximize(ObjectiveKey::Credit(AgentId::B), credit(world, AgentId::B)),
    ])
    .expect("bridge objective schema is static and unique")
}

pub fn priority(world: &World, agent: AgentId) -> u64 {
    world
        .balance(&AccountId::Agent(agent), &Asset::PriorityBenefit)
        .get()
}

pub fn credit(world: &World, agent: AgentId) -> u64 {
    world
        .balance(&AccountId::Agent(agent), &Asset::Credit)
        .get()
}

pub fn first_come_proposal(first: AgentId) -> Option<Proposal> {
    let second = other(first);
    validated_trace([
        RateId::ClaimFirst { agent: first },
        RateId::Cross { agent: first },
        RateId::ClaimSecond { agent: second },
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

pub fn first_come_showcase(first: AgentId) -> Option<Proposal> {
    let second = other(first);
    validated_trace_from(
        initial_showcase(),
        [
            RateId::ClaimFirst { agent: first },
            RateId::Cross { agent: first },
            RateId::ClaimSecond { agent: second },
            RateId::Cross { agent: second },
            RateId::ResetRound,
            RateId::ClaimFirst { agent: first },
            RateId::Cross { agent: first },
            RateId::ClaimSecond { agent: second },
            RateId::Cross { agent: second },
            RateId::Finish,
        ],
    )
}

pub fn auction_showcase() -> Option<Proposal> {
    validated_trace_from(
        initial_showcase(),
        [
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
            RateId::ResetRound,
            RateId::SubmitBid {
                agent: AgentId::A,
                amount: 1,
            },
            RateId::SubmitBid {
                agent: AgentId::B,
                amount: 2,
            },
            RateId::Resolve {
                winner: AgentId::B,
                winning_bid: 2,
                losing_bid: 1,
            },
            RateId::Cross { agent: AgentId::B },
            RateId::YieldToWaiting { agent: AgentId::A },
            RateId::Cross { agent: AgentId::A },
            RateId::Finish,
        ],
    )
}

fn validated_trace<const N: usize>(rates: [RateId; N]) -> Option<Proposal> {
    validated_trace_from(initial(), rates)
}

fn validated_trace_from<const N: usize>(mut world: World, rates: [RateId; N]) -> Option<Proposal> {
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
        | RateId::ClaimSecond { agent }
        | RateId::YieldToWaiting { agent }
        | RateId::Cross { agent } => Exchange::new(rate, Quantity::new(1))
            .bind(Role::Traveler, AccountId::Agent(agent))
            .bind(Role::Bridge, AccountId::Bridge),
        RateId::Resolve { winner, .. } => Exchange::new(rate, Quantity::new(1))
            .bind(Role::Winner, AccountId::Agent(winner))
            .bind(Role::Loser, AccountId::Agent(other(winner)))
            .bind(Role::Bridge, AccountId::Bridge),
        RateId::ResetRound | RateId::Finish => Exchange::new(rate, Quantity::new(1))
            .bind(Role::AgentA, AccountId::Agent(AgentId::A))
            .bind(Role::AgentB, AccountId::Agent(AgentId::B))
            .bind(Role::Bridge, AccountId::Bridge),
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
            assert!(candidates(&replay).is_empty());
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
        .bind(Role::Traveler, AccountId::Agent(AgentId::B))
        .bind(Role::Bridge, AccountId::Bridge);

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

    #[test]
    fn pareto_front_retains_both_priority_allocations_and_rejects_wasteful_payment() {
        let initial = initial();
        let result = pareto_front(&initial).unwrap();
        let mut outcomes = Vec::new();

        for entry in result.front().entries() {
            let replayed = initial.replayed(entry.payload()).unwrap();
            assert!(replayed.matches(&goal()));
            assert_eq!(&objectives(&replayed), entry.objectives());
            outcomes.push((
                priority(&replayed, AgentId::A),
                priority(&replayed, AgentId::B),
                credit(&replayed, AgentId::A),
                credit(&replayed, AgentId::B),
            ));
        }

        outcomes.sort_unstable();
        assert_eq!(outcomes, [(0, 1, 2, 2), (1, 0, 2, 2)]);
    }
}
