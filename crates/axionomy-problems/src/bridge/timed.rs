//! Shared-time traffic: finite leases, occupied crossings, deadlines and a detour.
use super::*;
use axionomy::QuantityComparison as C;
use axionomy::path;

pub fn enabled(world: &World) -> bool {
    world.rate(&RateId::Advance { to: 1 }).is_some()
}
fn clock() -> Q<Ontology> {
    Q::balance(Role::Bridge, Asset::Now)
}
fn clear(rate: Rate<Ontology>, role: Role, asset: Asset) -> Rate<Ontology> {
    rate.consume_computed(role, asset, Q::balance(role, asset))
}
/// One event at a time.
fn one_event(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.condition(path!("one-event"), Q::units(), C::Equal, Q::constant(1))
}
fn law() -> Rate<Ontology> {
    one_event(at_the_bridge())
}
/// A move of either traveller, `{agent}`, one event at a time.
fn timed_traveller() -> Rate<Ontology> {
    one_event(traveller())
}
fn arrived(rate: Rate<Ontology>, role: Role) -> Rate<Ontology> {
    rate.condition(
        path!("traveler-has-arrived"),
        clock(),
        C::GreaterThanOrEqual,
        Q::balance(role, Asset::Arrival),
    )
}
/// How many ticks a traveller's crossing holds the lane: law structure,
/// which the crossing's relation gives as `{crossing-time}`.
pub fn duration(agent: AgentId) -> u64 {
    if agent == AgentId::A { 3 } else { 2 }
}

/// How many ticks the paid detour takes, `{detour-time}`: the same for
/// either traveller, a constant.
fn detour_time() -> Param<u64> {
    Param::new(path!("detour-time"))
}

pub(super) fn build(source: World) -> World {
    let mut builder = EconomyBuilder::new().constant(&detour_time(), 3);
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
    // Each untimed rate keeps its law, timed: the family its instances
    // belong to says how.
    for (key, rate) in source.rates() {
        let family = source
            .instances_of(key)
            .next()
            .expect("every untimed rate has an instance");
        let mut rate = one_event(rate.clone());
        match family {
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
            // A crossing holds the lane for the traveller's crossing time.
            RateId::Cross { .. } => {
                let crossing = Param::<u64>::new(path!("crossing-time"));
                rate = clear(timed_traveller(), Role::Traveler, Asset::LeaseUntil)
                    .relation(Relation::each_giving(
                        &agent(),
                        [crossing.name().clone()],
                        [AgentId::A, AgentId::B].map(|agent| (agent, [duration(agent)])),
                    ))
                    .preserve(Role::Traveler, basket([(Asset::CrossingRight, 1)]))
                    .condition(
                        path!("reservation-has-not-expired"),
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
                        clock() + crossing.quantity(),
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
        builder = builder.rate_at(key.clone(), rate);
    }
    for late in [false, true] {
        for detour in [false, true] {
            let mut complete = clear(timed_traveller(), Role::Traveler, Asset::ReadyAt)
                .condition(
                    path!("travel-duration-elapsed"),
                    clock(),
                    C::GreaterThanOrEqual,
                    Q::balance(Role::Traveler, Asset::ReadyAt),
                )
                .condition(
                    path!("deadline-outcome"),
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
            builder = builder.rate_at(
                RateIdPattern::Complete {
                    agent: agent().var(),
                    detour: Slot::Is(detour),
                    late: Slot::Is(late),
                }
                .into(),
                complete,
            );
        }
    }
    for waiting in [false, true] {
        builder = builder.rate_at(
            RateIdPattern::Detour {
                agent: agent().var(),
                waiting: Slot::Is(waiting),
            }
            .into(),
            arrived(timed_traveller(), Role::Traveler)
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
                .produce_computed(
                    Role::Traveler,
                    Asset::ReadyAt,
                    clock() + detour_time().quantity(),
                ),
        );
    }
    builder = builder
        .rate_at(
            RateIdPattern::ExpireReservation {
                agent: agent().var(),
            }
            .into(),
            clear(timed_traveller(), Role::Traveler, Asset::LeaseUntil)
                .preserve(Role::Traveler, basket([(Asset::At(Side::West), 1)]))
                .condition(
                    path!("unused-reservation-expired"),
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
        )
        .rate_at(
            RateIdPattern::WithdrawBid {
                agent: agent().var(),
                amount: amount().var(),
            }
            .into(),
            timed_traveller()
                .variable(amount().range(1, 2))
                .consume_at(
                    Role::Traveler,
                    one([
                        Pattern::literal(Asset::Submitted),
                        AssetPattern::Bid(amount().var()).into(),
                    ]),
                )
                .consume_computed(Role::Traveler, Asset::Escrow, amount().quantity())
                .produce(Role::Traveler, basket([(Asset::CanBid, 1)]))
                .produce_computed(Role::Traveler, Asset::Credit, amount().quantity()),
        );
    let to = Param::<u8>::new(path!("to"));
    builder = builder.rate_at(
        RateIdPattern::Advance { to: to.var() }.into(),
        law()
            .variable(to.range(1, 64))
            .condition(
                path!("clock-moves-forward"),
                clock() + Q::constant(1),
                C::LessThanOrEqual,
                to.quantity(),
            )
            .consume_computed(Role::Bridge, Asset::TimeRemaining, to.quantity() - clock())
            .produce_computed(Role::Bridge, Asset::Now, to.quantity() - clock()),
    );
    for invariant in source.invariants() {
        let invariant = match invariant.name().as_str() {
            "two-agent-positions" => invariant
                .clone()
                .weight(Asset::InTransit, 1)
                .weight(Asset::OnDetour, 1),
            "two-agent-status-tokens" => invariant.clone().weight(Asset::Detouring, 1),
            _ => invariant.clone(),
        };
        builder = builder.invariant(invariant);
    }
    builder
        .invariant(
            LinearInvariant::new(path!("bounded-shared-clock"))
                .weight(Asset::Now, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .build()
        .expect("timed bridge is closed")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn balance(world: &World, agent: AgentId, asset: Asset) -> u64 {
        world.balance(&AccountId::Agent(agent), &asset).to_u64()
    }

    /// `world` after `rates`, each applied in turn.
    fn played(world: &World, rates: &[RateId]) -> World {
        let mut world = world.clone();
        for rate in rates {
            world
                .apply(action(&world, *rate))
                .unwrap_or_else(|error| panic!("{rate:?}: {error}"));
        }
        world
    }

    /// The first round, A first: A crosses on time, and B, who waits for the
    /// lane, crosses late.
    const WAITED_ROUND: [RateId; 9] = [
        RateId::ClaimFirst { agent: AgentId::A },
        RateId::Cross { agent: AgentId::A },
        RateId::Advance { to: 1 },
        RateId::Advance { to: 3 },
        RateId::Complete {
            agent: AgentId::A,
            detour: false,
            late: false,
        },
        RateId::ClaimSecond { agent: AgentId::B },
        RateId::Cross { agent: AgentId::B },
        RateId::Advance { to: 5 },
        RateId::Complete {
            agent: AgentId::B,
            detour: false,
            late: true,
        },
    ];

    /// The first round, A first, while B pays for the detour and arrives on
    /// time.
    const DETOURED_ROUND: [RateId; 8] = [
        RateId::ClaimFirst { agent: AgentId::A },
        RateId::Cross { agent: AgentId::A },
        RateId::Advance { to: 1 },
        RateId::Detour {
            agent: AgentId::B,
            waiting: false,
        },
        RateId::Advance { to: 3 },
        RateId::Complete {
            agent: AgentId::A,
            detour: false,
            late: false,
        },
        RateId::Advance { to: 4 },
        RateId::Complete {
            agent: AgentId::B,
            detour: true,
            late: false,
        },
    ];

    /// A later round, reset and started at `at`: A crosses first, then B,
    /// both on time.
    fn later_round(at: u8) -> [RateId; 11] {
        [
            RateId::ResetRound,
            RateId::Advance { to: at },
            RateId::ClaimFirst { agent: AgentId::A },
            RateId::Cross { agent: AgentId::A },
            RateId::Advance { to: at + 2 },
            RateId::Advance { to: at + 3 },
            RateId::Complete {
                agent: AgentId::A,
                detour: false,
                late: false,
            },
            RateId::ClaimSecond { agent: AgentId::B },
            RateId::Cross { agent: AgentId::B },
            RateId::Advance { to: at + 5 },
            RateId::Complete {
                agent: AgentId::B,
                detour: false,
                late: false,
            },
        ]
    }

    #[test]
    fn lane_occupancy_and_expiring_reservations_are_enforced_by_core() {
        let mut world = initial_showcase();
        assert!(!world.is_applicable(&action(&world, RateId::ClaimFirst { agent: AgentId::B })));
        world
            .apply(action(&world, RateId::ClaimFirst { agent: AgentId::A }))
            .unwrap();
        let mut expired = world.clone();
        expired
            .apply(action(&expired, RateId::Advance { to: 3 }))
            .unwrap();
        assert!(!expired.is_applicable(&action(&expired, RateId::Cross { agent: AgentId::A })));
        expired
            .apply(action(
                &expired,
                RateId::ExpireReservation { agent: AgentId::A },
            ))
            .unwrap();
        assert_eq!(
            expired
                .balance(&AccountId::Bridge, &Asset::CapacityFree)
                .to_u64(),
            1
        );
        world
            .apply(action(&world, RateId::Cross { agent: AgentId::A }))
            .unwrap();
        world
            .apply(action(&world, RateId::Advance { to: 1 }))
            .unwrap();
        assert!(!world.is_applicable(&action(&world, RateId::ClaimSecond { agent: AgentId::B })));
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Complete {
                agent: AgentId::A,
                detour: false,
                late: false
            }
        )));
        world
            .apply(action(
                &world,
                RateId::Detour {
                    agent: AgentId::B,
                    waiting: false,
                },
            ))
            .unwrap();
        assert_eq!(balance(&world, AgentId::A, Asset::InTransit), 1);
        assert_eq!(balance(&world, AgentId::B, Asset::OnDetour), 1);
    }
    #[test]
    fn a_paid_detour_avoids_a_late_trip_on_the_same_traffic() {
        let world = initial_showcase();
        let waited = played(
            &world,
            &[&WAITED_ROUND[..], &later_round(6), &[RateId::Finish]].concat(),
        );
        let detoured = played(
            &world,
            &[&DETOURED_ROUND[..], &later_round(5), &[RateId::Finish]].concat(),
        );
        let late = |end: &World| {
            balance(end, AgentId::A, Asset::Late) + balance(end, AgentId::B, Asset::Late)
        };
        let credit = |end: &World| {
            balance(end, AgentId::A, Asset::Credit) + balance(end, AgentId::B, Asset::Credit)
        };
        for end in [&waited, &detoured] {
            assert!(end.matches(&goal()));
        }
        assert_eq!((late(&waited), credit(&waited)), (1, 8));
        assert_eq!((late(&detoured), credit(&detoured)), (0, 7));
    }

    #[test]
    fn stress_needs_a_third_round() {
        let two_rounds = [&WAITED_ROUND[..], &later_round(6)].concat();
        let finish = action(&initial_showcase(), RateId::Finish);
        assert!(played(&initial_showcase(), &two_rounds).is_applicable(&finish));
        let stress = played(&initial_stress(), &two_rounds);
        assert!(!stress.is_applicable(&finish));
        let solved = played(&stress, &[&later_round(12)[..], &[RateId::Finish]].concat());
        assert!(solved.matches(&goal()));
        assert_eq!(
            solved
                .balance(&AccountId::Bridge, &Asset::RoundsRemaining)
                .to_u64(),
            0
        );
    }
}
