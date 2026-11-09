//! Shared-time traffic: finite leases, occupied crossings, deadlines and a detour.
use super::*;
use axionomy::{QuantityComparison as C, QuantityExpression as Q};

pub fn enabled(world: &World) -> bool {
    world.rate(&RateId::Advance { to: 1 }).is_some()
}
pub fn now(world: &World) -> u64 {
    world.balance(&AccountId::Bridge, &Asset::Now).to_u64()
}
fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
    world.balance(&AccountId::Agent(agent), &asset).to_u64()
}
fn clock() -> Q<Ontology> {
    Q::balance(Role::Bridge, Asset::Now)
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, Q::balance(role, asset))
}
fn law() -> Rate<Ontology> {
    Rate::new()
        .preserve(
            Role::Bridge,
            basket([(Asset::BridgeIdentity, 1), (Asset::Active, 1)]),
        )
        .condition("one event", Q::units(), C::Equal, Q::constant(1))
}
fn traveler(agent: AgentId) -> Rate<Ontology> {
    law()
        .preserve(Role::Traveler, basket([(Asset::AgentIdentity(agent), 1)]))
        .distinct(Role::Traveler, Role::Bridge)
}
fn arrived(rate: Rate<Ontology>, role: Role) -> Rate<Ontology> {
    rate.condition(
        "traveler has arrived",
        clock(),
        C::GreaterThanOrEqual,
        Q::balance(role, Asset::Arrival),
    )
}
pub fn duration(agent: AgentId) -> u64 {
    if agent == AgentId::A { 3 } else { 2 }
}

pub(super) fn build(source: World) -> World {
    let mut builder = EconomyBuilder::new();
    for (id, account) in source.accounts() {
        let mut account = account.clone();
        account
            .deposit(&match id {
                AccountId::Bridge => basket([(Asset::TimeRemaining, 64)]),
                AccountId::Agent(agent) => basket([
                    (Asset::Arrival, u64::from(*agent == AgentId::B)),
                    (Asset::Deadline, if *agent == AgentId::A { 7 } else { 4 }),
                ]),
            })
            .unwrap();
        builder = builder.account(*id, account);
    }
    for id in source.rate_ids() {
        let mut rate = source.rate(id).unwrap().clone().condition(
            "one event",
            Q::units(),
            C::Equal,
            Q::constant(1),
        );
        match *id {
            RateId::SubmitBid { .. } => rate = arrived(rate, Role::Traveler),
            RateId::ClaimFirst { .. }
            | RateId::ClaimSecond { .. }
            | RateId::YieldToWaiting { .. } => {
                rate = arrived(rate, Role::Traveler).produce_computed(
                    Role::Traveler,
                    Asset::LeaseUntil,
                    clock() + Q::constant(2),
                );
            }
            RateId::Resolve { .. } => {
                rate = rate.produce_computed(
                    Role::Winner,
                    Asset::LeaseUntil,
                    clock() + Q::constant(2),
                );
            }
            RateId::Cross { agent } => {
                rate = clear(traveler(agent), Role::Traveler, Asset::LeaseUntil)
                    .preserve(Role::Traveler, basket([(Asset::CrossingRight, 1)]))
                    .condition(
                        "reservation has not expired",
                        clock(),
                        C::LessThanOrEqual,
                        Q::balance(Role::Traveler, Asset::LeaseUntil),
                    )
                    .consume(
                        Role::Traveler,
                        basket([(Asset::At(Side::West), 1), (Asset::Energy, 1)]),
                    )
                    .produce(
                        Role::Traveler,
                        basket([(Asset::InTransit, 1), (Asset::SpentEnergy, 1)]),
                    )
                    .produce_computed(
                        Role::Traveler,
                        Asset::ReadyAt,
                        clock() + Q::constant(duration(agent)),
                    );
            }
            RateId::ResetRound => {
                for (role, arrival, deadline) in [(Role::AgentA, 1, 5), (Role::AgentB, 3, 10)] {
                    rate = clear(clear(rate, role, Asset::Arrival), role, Asset::Deadline)
                        .produce_computed(role, Asset::Arrival, clock() + Q::constant(arrival))
                        .produce_computed(role, Asset::Deadline, clock() + Q::constant(deadline));
                }
                rate = rate.produce(Role::Bridge, basket([(Asset::Round, 1)]));
            }
            _ => {}
        }
        builder = builder.rate(*id, rate);
    }
    for agent in [AgentId::A, AgentId::B] {
        for late in [false, true] {
            for detour in [false, true] {
                let mut complete = clear(traveler(agent), Role::Traveler, Asset::ReadyAt)
                    .condition(
                        "travel duration elapsed",
                        clock(),
                        C::GreaterThanOrEqual,
                        Q::balance(Role::Traveler, Asset::ReadyAt),
                    )
                    .condition(
                        "deadline outcome",
                        if late {
                            clock()
                        } else {
                            Q::balance(Role::Traveler, Asset::Deadline)
                        },
                        C::GreaterThanOrEqual,
                        if late {
                            Q::balance(Role::Traveler, Asset::Deadline) + Q::constant(1)
                        } else {
                            clock()
                        },
                    )
                    .consume(
                        Role::Traveler,
                        basket([
                            (
                                if detour {
                                    Asset::Detouring
                                } else {
                                    Asset::CrossingRight
                                },
                                1,
                            ),
                            (
                                if detour {
                                    Asset::OnDetour
                                } else {
                                    Asset::InTransit
                                },
                                1,
                            ),
                        ]),
                    )
                    .produce(
                        Role::Traveler,
                        basket([
                            (Asset::At(Side::East), 1),
                            (Asset::Crossed, 1),
                            (if late { Asset::Late } else { Asset::OnTime }, 1),
                        ]),
                    );
                if detour {
                    // If nobody used the bridge this round, close its first turn.
                    complete = complete
                        .consume_computed(
                            Role::Bridge,
                            Asset::FirstTurn,
                            Q::balance(Role::Bridge, Asset::FirstTurn),
                        )
                        .produce_computed(
                            Role::Bridge,
                            Asset::SecondTurn,
                            Q::balance(Role::Bridge, Asset::FirstTurn),
                        );
                } else {
                    complete = complete.produce(
                        Role::Bridge,
                        basket([(Asset::CapacityFree, 1), (Asset::SecondTurn, 1)]),
                    );
                }
                builder = builder.rate(
                    RateId::Complete {
                        agent,
                        detour,
                        late,
                    },
                    complete,
                );
            }
        }
        for waiting in [false, true] {
            builder = builder.rate(
                RateId::Detour { agent, waiting },
                arrived(traveler(agent), Role::Traveler)
                    .consume(
                        Role::Traveler,
                        basket([
                            (
                                if waiting {
                                    Asset::Waiting
                                } else {
                                    Asset::CanBid
                                },
                                1,
                            ),
                            (Asset::At(Side::West), 1),
                            (Asset::Energy, 1),
                            (Asset::Credit, 1),
                        ]),
                    )
                    .produce(
                        Role::Traveler,
                        basket([
                            (Asset::Detouring, 1),
                            (Asset::OnDetour, 1),
                            (Asset::SpentEnergy, 1),
                            (Asset::SpentCredit, 1),
                        ]),
                    )
                    .produce_computed(Role::Traveler, Asset::ReadyAt, clock() + Q::constant(3)),
            );
        }
        builder = builder.rate(
            RateId::ExpireReservation { agent },
            clear(traveler(agent), Role::Traveler, Asset::LeaseUntil)
                .preserve(Role::Traveler, basket([(Asset::At(Side::West), 1)]))
                .condition(
                    "unused reservation expired",
                    clock(),
                    C::GreaterThanOrEqual,
                    Q::balance(Role::Traveler, Asset::LeaseUntil) + Q::constant(1),
                )
                .consume(Role::Traveler, basket([(Asset::CrossingRight, 1)]))
                .produce(
                    Role::Traveler,
                    basket([(Asset::Waiting, 1), (Asset::MissedReservation, 1)]),
                )
                .produce(
                    Role::Bridge,
                    basket([(Asset::CapacityFree, 1), (Asset::SecondTurn, 1)]),
                ),
        );
        for amount in 1..=2 {
            builder = builder.rate(
                RateId::WithdrawBid { agent, amount },
                traveler(agent)
                    .consume(
                        Role::Traveler,
                        basket([
                            (Asset::Submitted, 1),
                            (Asset::Bid(amount), 1),
                            (Asset::Escrow, u64::from(amount)),
                        ]),
                    )
                    .produce(
                        Role::Traveler,
                        basket([(Asset::CanBid, 1), (Asset::Credit, u64::from(amount))]),
                    ),
            );
        }
    }
    for to in 1..=64 {
        builder = builder.rate(
            RateId::Advance { to },
            law()
                .condition(
                    "clock moves forward",
                    clock() + Q::constant(1),
                    C::LessThanOrEqual,
                    Q::constant(u64::from(to)),
                )
                .consume_computed(
                    Role::Bridge,
                    Asset::TimeRemaining,
                    Q::constant(u64::from(to)) - clock(),
                )
                .produce_computed(
                    Role::Bridge,
                    Asset::Now,
                    Q::constant(u64::from(to)) - clock(),
                ),
        );
    }
    for invariant in source.invariants() {
        let invariant = match invariant.name() {
            "two agent positions" => invariant
                .clone()
                .weight(Asset::InTransit, 1)
                .weight(Asset::OnDetour, 1),
            "two agent status tokens" => invariant.clone().weight(Asset::Detouring, 1),
            _ => invariant.clone(),
        };
        builder = builder.invariant(invariant);
    }
    builder
        .invariant(
            LinearInvariant::new("bounded shared clock")
                .weight(Asset::Now, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .build()
        .expect("timed bridge is closed")
}

/// Propose the next meaningful time boundary; the core also validates arbitrary
/// user-selected waits. Arrival, completion, and lease times come from accounts.
pub fn next_time(world: &World) -> Option<u8> {
    let now = now(world);
    [AgentId::A, AgentId::B]
        .into_iter()
        .flat_map(|agent| {
            let mut events = Vec::new();
            if balance(world, agent, Asset::At(Side::West)) > 0 {
                events.push(balance(world, agent, Asset::Arrival));
            }
            if balance(world, agent, Asset::InTransit) + balance(world, agent, Asset::OnDetour) > 0
            {
                events.push(balance(world, agent, Asset::ReadyAt));
            }
            if balance(world, agent, Asset::CrossingRight) > 0
                && balance(world, agent, Asset::At(Side::West)) > 0
            {
                events.push(balance(world, agent, Asset::LeaseUntil) + 1);
            }
            events
        })
        .filter(|time| *time > now && *time <= 64)
        .min()
        .map(|time| time as u8)
}
pub fn candidates(world: &World) -> Vec<Action> {
    let next = next_time(world);
    let mut ids: Vec<_> = world
        .rate_ids()
        .copied()
        .filter(|id| match id {
            RateId::Advance { to } => Some(*to) == next,
            _ => true,
        })
        .collect();
    ids.sort();
    world.applicable(ids.into_iter().map(action))
}
pub fn search_candidates(world: &World) -> Vec<Action> {
    let candidates = candidates(world);
    let ready = candidates.iter().find(|a| {
        matches!(
            a.rate(),
            RateId::Complete { .. } | RateId::ResetRound | RateId::Finish | RateId::Cross { .. }
        )
    });
    if let Some(ready) = ready {
        return vec![ready.clone()];
    }
    candidates
        .into_iter()
        .filter(|a| {
            !matches!(
                a.rate(),
                RateId::SubmitBid { .. }
                    | RateId::WithdrawBid { .. }
                    | RateId::Resolve { .. }
                    | RateId::ExpireReservation { .. }
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub enum Policy {
    Priority(AgentId),
    Auction,
    Adaptive,
}

pub fn run(initial: &World, policy: Policy) -> Option<Proposal> {
    let mut world = initial.clone();
    let mut trace = Trace::new();
    for _ in 0..150 {
        if world.matches(&goal()) {
            return Some(trace);
        }
        let choices = candidates(&world);
        let mut chosen = choices
            .iter()
            .find(|a| {
                matches!(
                    a.rate(),
                    RateId::Complete { .. }
                        | RateId::ResetRound
                        | RateId::Finish
                        | RateId::Cross { .. }
                        | RateId::Resolve { .. }
                )
            })
            .cloned();
        if chosen.is_none() {
            match policy {
                Policy::Priority(first) => {
                    let is_first = world
                        .balance(&AccountId::Bridge, &Asset::FirstTurn)
                        .to_u64()
                        > 0;
                    chosen = choices.iter().find(|a| matches!(a.rate(), RateId::ClaimFirst { agent } if *agent == first && is_first) || matches!(a.rate(), RateId::ClaimSecond { .. } | RateId::YieldToWaiting { .. })).cloned();
                }
                Policy::Auction => {
                    if world
                        .balance(&AccountId::Bridge, &Asset::FirstTurn)
                        .to_u64()
                        > 0
                    {
                        if [AgentId::A, AgentId::B]
                            .iter()
                            .all(|agent| now(&world) >= balance(&world, *agent, Asset::Arrival))
                        {
                            for agent in [AgentId::A, AgentId::B] {
                                let slack = balance(&world, agent, Asset::Deadline)
                                    .saturating_sub(now(&world) + duration(agent));
                                let amount = (if slack <= 2 { 2 } else { 1 })
                                    .min(credit(&world, agent))
                                    as u8;
                                let bid = action(RateId::SubmitBid { agent, amount });
                                if world.is_applicable(&bid) {
                                    chosen = Some(bid);
                                    break;
                                }
                            }
                            if chosen.is_none() {
                                chosen = choices
                                    .iter()
                                    .find(|a| matches!(a.rate(), RateId::ClaimFirst { .. }))
                                    .cloned();
                            }
                        }
                    } else {
                        chosen = choices
                            .iter()
                            .find(|a| {
                                matches!(
                                    a.rate(),
                                    RateId::YieldToWaiting { .. } | RateId::ClaimSecond { .. }
                                )
                            })
                            .cloned();
                    }
                }
                Policy::Adaptive => {
                    // Pay for the alternate route only when congestion makes
                    // the bridge miss a deadline that the detour can still meet.
                    chosen = choices
                        .iter()
                        .filter(|a| matches!(a.rate(), RateId::Detour { .. }))
                        .find(|a| {
                            let RateId::Detour { agent, .. } = *a.rate() else {
                                unreachable!()
                            };
                            let deadline = balance(&world, agent, Asset::Deadline);
                            let occupied_until =
                                balance(&world, other(agent), Asset::ReadyAt).max(now(&world));
                            now(&world) + 3 <= deadline
                                && occupied_until + duration(agent) > deadline
                        })
                        .cloned();
                    if chosen.is_none() {
                        chosen = choices
                            .iter()
                            .filter(|a| {
                                matches!(
                                    a.rate(),
                                    RateId::ClaimFirst { .. }
                                        | RateId::ClaimSecond { .. }
                                        | RateId::YieldToWaiting { .. }
                                )
                            })
                            .min_by_key(|a| {
                                let agent = match *a.rate() {
                                    RateId::ClaimFirst { agent }
                                    | RateId::ClaimSecond { agent }
                                    | RateId::YieldToWaiting { agent } => agent,
                                    _ => unreachable!(),
                                };
                                balance(&world, agent, Asset::Deadline)
                            })
                            .cloned();
                    }
                }
            }
        }
        let chosen = chosen.or_else(|| {
            choices
                .iter()
                .find(|a| matches!(a.rate(), RateId::Advance { .. }))
                .cloned()
        })?;
        world.apply(chosen.clone()).ok()?;
        trace.push(chosen);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lane_occupancy_and_expiring_reservations_are_enforced_by_core() {
        let mut world = initial_showcase();
        assert!(!world.is_applicable(&action(RateId::ClaimFirst { agent: AgentId::B })));
        world
            .apply(action(RateId::ClaimFirst { agent: AgentId::A }))
            .unwrap();
        let mut expired = world.clone();
        expired.apply(action(RateId::Advance { to: 3 })).unwrap();
        assert!(!expired.is_applicable(&action(RateId::Cross { agent: AgentId::A })));
        expired
            .apply(action(RateId::ExpireReservation { agent: AgentId::A }))
            .unwrap();
        assert_eq!(
            expired
                .balance(&AccountId::Bridge, &Asset::CapacityFree)
                .to_u64(),
            1
        );
        world
            .apply(action(RateId::Cross { agent: AgentId::A }))
            .unwrap();
        world.apply(action(RateId::Advance { to: 1 })).unwrap();
        assert!(!world.is_applicable(&action(RateId::ClaimSecond { agent: AgentId::B })));
        assert!(!world.is_applicable(&action(RateId::Complete {
            agent: AgentId::A,
            detour: false,
            late: false
        })));
        world
            .apply(action(RateId::Detour {
                agent: AgentId::B,
                waiting: false,
            }))
            .unwrap();
        assert_eq!(balance(&world, AgentId::A, Asset::InTransit), 1);
        assert_eq!(balance(&world, AgentId::B, Asset::OnDetour), 1);
    }
    #[test]
    fn policies_replay_and_trade_deadlines_against_credit_on_identical_traffic() {
        for initial in [initial_showcase(), initial_stress()] {
            let policies = [
                Policy::Priority(AgentId::A),
                Policy::Priority(AgentId::B),
                Policy::Auction,
                Policy::Adaptive,
            ];
            let mut outcomes = Vec::new();
            for policy in policies {
                let trace = run(&initial, policy).expect("policy completes traffic");
                assert_eq!(trace, run(&initial, policy).unwrap());
                let replay = initial.replayed(&trace).unwrap();
                assert!(replay.matches(&goal()));
                assert!(candidates(&replay).is_empty());
                outcomes.push((
                    [AgentId::A, AgentId::B]
                        .into_iter()
                        .map(|a| balance(&replay, a, Asset::Late))
                        .sum::<u64>(),
                    credit(&replay, AgentId::A) + credit(&replay, AgentId::B),
                ));
            }
            assert!(
                outcomes[0].0 > outcomes[3].0,
                "responsive detours avoid lateness: {outcomes:?}"
            );
            assert!(outcomes[0].1 > outcomes[3].1, "detours have a real price");
            let solved = solve(&initial).unwrap();
            assert!(initial.replayed(solved.trace()).unwrap().matches(&goal()));
        }
    }
}
