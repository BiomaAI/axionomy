#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern, Quantity,
    QuantityExpression as Q, Rate, Relation, Slot, basket,
};

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
    build(1)
}

/// Two consecutive capacity allocations force policies to reason about
/// repeated fairness, retained credit, and an atomic multi-agent round reset.
pub fn initial_showcase() -> World {
    timed::build(build(2))
}

/// Three consecutive allocations make retained credit and repeated priority
/// materially affect later rounds while keeping the state space finite.
pub fn initial_stress() -> World {
    timed::build(build(3))
}

/// The traveller a rate's identifier names, `{agent}`.
fn agent() -> Param<AgentId> {
    Param::new(path!("agent"))
}

/// The amount a bid's identifier names, `{amount}`: one or two credits.
fn amount() -> Param<u8> {
    Param::new(path!("amount"))
}

/// One unit of each asset `assets` names.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}

/// An event at the bridge while it is active, the bridge role fixed to the
/// bridge.
fn at_the_bridge() -> Rate<Ontology> {
    Rate::new()
        .account(Role::Bridge, Pattern::literal(AccountId::Bridge))
        .preserve(Role::Bridge, basket([(Asset::Active, 1)]))
}

/// A move of either traveller, `{agent}`, the traveller role fixed to its
/// account.
fn traveller() -> Rate<Ontology> {
    at_the_bridge().variable(agent().every()).account(
        Role::Traveler,
        AccountIdPattern::Agent(agent().var()).into(),
    )
}

/// An event of both travellers, each role fixed to its traveller.
fn both() -> Rate<Ontology> {
    Rate::new()
        .account(Role::AgentA, Pattern::literal(AccountId::Agent(AgentId::A)))
        .account(Role::AgentB, Pattern::literal(AccountId::Agent(AgentId::B)))
        .account(Role::Bridge, Pattern::literal(AccountId::Bridge))
}

fn build(rounds: u64) -> World {
    assert!(rounds > 0, "a bridge problem needs at least one round");
    let credit = rounds * 2;
    let mut bridge = axionomy::Basket::from([
        (Asset::CapacityFree, Quantity::new(1)),
        (Asset::FirstTurn, Quantity::new(1)),
        (Asset::Active, Quantity::new(1)),
    ]);
    if rounds > 1 {
        bridge.insert(Asset::RoundsRemaining, Quantity::new(rounds - 1));
    }
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent(AgentId::A),
            Account::from(basket([
                (Asset::At(Side::West), 1),
                (Asset::Energy, 1),
                (Asset::Credit, credit),
                (Asset::CanBid, 1),
            ])),
        )
        .account(
            AccountId::Agent(AgentId::B),
            Account::from(basket([
                (Asset::At(Side::West), 1),
                (Asset::Energy, 1),
                (Asset::Credit, credit),
                (Asset::CanBid, 1),
            ])),
        )
        .account(AccountId::Bridge, Account::from(bridge));

    builder = builder
        .rate_at(
            RateIdPattern::SubmitBid {
                agent: agent().var(),
                amount: amount().var(),
            }
            .into(),
            traveller()
                .variable(amount().range(1, 2))
                .consume(Role::Traveler, basket([(Asset::CanBid, 1)]))
                .consume_computed(Role::Traveler, Asset::Credit, amount().quantity())
                .produce_at(
                    Role::Traveler,
                    one([
                        Pattern::literal(Asset::Submitted),
                        AssetPattern::Bid(amount().var()).into(),
                    ]),
                )
                .produce_computed(Role::Traveler, Asset::Escrow, amount().quantity()),
        )
        .rate_at(
            RateIdPattern::ClaimFirst {
                agent: agent().var(),
            }
            .into(),
            traveller()
                .consume(Role::Traveler, basket([(Asset::CanBid, 1)]))
                .consume(
                    Role::Bridge,
                    basket([(Asset::CapacityFree, 1), (Asset::FirstTurn, 1)]),
                )
                .produce(
                    Role::Traveler,
                    basket([(Asset::CrossingRight, 1), (Asset::PriorityBenefit, 1)]),
                ),
        )
        .rate_at(
            RateIdPattern::ClaimSecond {
                agent: agent().var(),
            }
            .into(),
            traveller()
                .consume(Role::Traveler, basket([(Asset::CanBid, 1)]))
                .consume(
                    Role::Bridge,
                    basket([(Asset::CapacityFree, 1), (Asset::SecondTurn, 1)]),
                )
                .produce(Role::Traveler, basket([(Asset::CrossingRight, 1)])),
        )
        .rate_at(
            RateIdPattern::YieldToWaiting {
                agent: agent().var(),
            }
            .into(),
            traveller()
                .consume(Role::Traveler, basket([(Asset::Waiting, 1)]))
                .consume(
                    Role::Bridge,
                    basket([(Asset::CapacityFree, 1), (Asset::SecondTurn, 1)]),
                )
                .produce(Role::Traveler, basket([(Asset::CrossingRight, 1)])),
        )
        .rate_at(
            RateIdPattern::Cross {
                agent: agent().var(),
            }
            .into(),
            traveller()
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
                ),
        );

    // An auction settles two bids, the higher winning and A winning a tie:
    // a law for each winner, whose loser is the other traveller.
    let (winning, losing) = (
        Param::<u8>::new(path!("winning-bid")),
        Param::<u8>::new(path!("losing-bid")),
    );
    for winner in [AgentId::A, AgentId::B] {
        let bids = (1..=2u8)
            .flat_map(|bid_a| (1..=2u8).map(move |bid_b| (bid_a, bid_b)))
            .filter_map(|(bid_a, bid_b)| match (winner, bid_a >= bid_b) {
                (AgentId::A, true) => Some((bid_a, bid_b)),
                (AgentId::B, false) => Some((bid_b, bid_a)),
                _ => None,
            });
        builder = builder.rate_at(
            RateIdPattern::Resolve {
                winner: Slot::Is(winner),
                winning_bid: winning.var(),
                losing_bid: losing.var(),
            }
            .into(),
            at_the_bridge()
                .variable(winning.range(1, 2))
                .variable(losing.range(1, 2))
                .relation(Relation::pairs(&winning, &losing, bids))
                .account(Role::Winner, Pattern::literal(AccountId::Agent(winner)))
                .account(
                    Role::Loser,
                    Pattern::literal(AccountId::Agent(other(winner))),
                )
                .consume_at(
                    Role::Winner,
                    one([
                        Pattern::literal(Asset::Submitted),
                        AssetPattern::Bid(winning.var()).into(),
                    ]),
                )
                .consume_computed(Role::Winner, Asset::Escrow, winning.quantity())
                .produce(
                    Role::Winner,
                    basket([(Asset::CrossingRight, 1), (Asset::PriorityBenefit, 1)]),
                )
                .produce_computed(Role::Winner, Asset::SpentCredit, winning.quantity())
                .consume_at(
                    Role::Loser,
                    one([
                        Pattern::literal(Asset::Submitted),
                        AssetPattern::Bid(losing.var()).into(),
                    ]),
                )
                .consume_computed(Role::Loser, Asset::Escrow, losing.quantity())
                .produce(Role::Loser, basket([(Asset::Waiting, 1)]))
                .produce_computed(Role::Loser, Asset::Credit, losing.quantity())
                .consume(
                    Role::Bridge,
                    basket([(Asset::CapacityFree, 1), (Asset::FirstTurn, 1)]),
                ),
        );
    }

    if rounds > 1 {
        let mut reset = both()
            .preserve(Role::Bridge, basket([(Asset::Active, 1)]))
            .consume(
                Role::Bridge,
                basket([(Asset::SecondTurn, 1), (Asset::RoundsRemaining, 1)]),
            )
            .produce(Role::Bridge, basket([(Asset::FirstTurn, 1)]));
        for role in [Role::AgentA, Role::AgentB] {
            reset = reset
                .consume(
                    role,
                    basket([
                        (Asset::At(Side::East), 1),
                        (Asset::Crossed, 1),
                        (Asset::SpentEnergy, 1),
                    ]),
                )
                .produce(
                    role,
                    basket([
                        (Asset::At(Side::West), 1),
                        (Asset::CanBid, 1),
                        (Asset::CompletedTrip, 1),
                        (Asset::Energy, 1),
                    ]),
                );
        }
        builder = builder.rate(RateId::ResetRound, reset);
    }

    let positions = LinearInvariant::new(path!("two-agent-positions"))
        .weight(Asset::At(Side::West), 1)
        .weight(Asset::At(Side::East), 1);
    let status = LinearInvariant::new(path!("two-agent-status-tokens"))
        .weight(Asset::CanBid, 1)
        .weight(Asset::Submitted, 1)
        .weight(Asset::CrossingRight, 1)
        .weight(Asset::Waiting, 1)
        .weight(Asset::Crossed, 1);

    let mut finish = both()
        .preserve(Role::AgentA, basket([(Asset::Crossed, 1)]))
        .preserve(Role::AgentB, basket([(Asset::Crossed, 1)]));
    if rounds > 1 {
        finish = finish
            .preserve(Role::AgentA, basket([(Asset::CompletedTrip, rounds - 1)]))
            .preserve(Role::AgentB, basket([(Asset::CompletedTrip, rounds - 1)]));
    }
    finish = finish
        .consume(Role::Bridge, basket([(Asset::Active, 1)]))
        .produce(Role::Bridge, basket([(Asset::Solved, 1)]));

    builder
        .rate(RateId::Finish, finish)
        .invariant(positions)
        .invariant(status)
        .invariant(
            LinearInvariant::new(path!("bridge-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .invariant(
            LinearInvariant::new(path!("bridge-capacity"))
                .weight(Asset::CapacityFree, 1)
                .weight(Asset::CrossingRight, 1),
        )
        .invariant(
            LinearInvariant::new(path!("crossing-turn"))
                .weight(Asset::FirstTurn, 1)
                .weight(Asset::SecondTurn, 1)
                .weight(Asset::CrossingRight, 1),
        )
        .invariant(
            LinearInvariant::new(path!("energy-accounting"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("credit-accounting"))
                .weight(Asset::Credit, 1)
                .weight(Asset::Escrow, 1)
                .weight(Asset::SpentCredit, 1),
        )
        .build()
        .expect("bridge model is valid")
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Bridge, basket([(Asset::Solved, 1)]))
}

fn other(agent: AgentId) -> AgentId {
    match agent {
        AgentId::A => AgentId::B,
        AgentId::B => AgentId::A,
    }
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
