//! Shared-time traffic: finite leases, occupied crossings, deadlines and a detour.
use super::*;

pub fn enabled(world: &World) -> bool {
    world.rate(&RateId::Advance { to: 1 }).is_some()
}
/// How many ticks a traveller's crossing holds the lane: law structure,
/// which the crossing's relation gives as `{crossing-time}`.
pub fn duration(agent: AgentId) -> u64 {
    if agent == AgentId::A { 3 } else { 2 }
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
