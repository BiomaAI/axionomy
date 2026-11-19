//! Independent teammates, private observations and timed cooperative extraction.
use axionomy::{Account, Economy, Exchange, Goal, Quantity, basket};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Agent {
    Scout,
    Medic,
}
pub const AGENTS: [Agent; 2] = [Agent::Scout, Agent::Medic];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Base,
    North,
    East,
    South,
    West,
}
pub const LOCATIONS: [Location; 5] = [
    Location::Base,
    Location::North,
    Location::East,
    Location::South,
    Location::West,
];
pub const ROADS: [(Location, Location); 6] = [
    (Location::Base, Location::North),
    (Location::Base, Location::South),
    (Location::North, Location::East),
    (Location::East, Location::South),
    (Location::South, Location::West),
    (Location::West, Location::North),
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    World,
    Nature,
    Agent(Agent),
    Objective(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Active,
    Closed,
    Time,
    TimeRemaining,
    Resolved,
    Completed,
    FailedCount,
    SavedVitality,
    Permit(Agent),
    UsedPermit(Agent),
    EnvironmentReady,
    WeatherDue(bool),
    HazardSite(Location),
    WeatherOpen(Location),
    WeatherBlocked(Location),
    At(Location),
    Travelling(Location),
    TeamAt(Agent, Location),
    TeamTravelling(Agent),
    Ready,
    ReadyAt,
    Collecting(u8),
    Extracting(u8),
    AwaitingReport,
    Energy,
    SpentEnergy,
    MedicalKit,
    UsedMedicalKit,
    Health,
    Harm,
    Radio,
    UsedRadio,
    Sensor,
    UsedSensor,
    ScanRound(u8),
    Signal(u8, Location),
    Hint(u8, Location),
    Unsearched(Location),
    Surveyed(Location),
    Known(u8, Location),
    Shared(u8, Location),
    Truth(u8, Location),
    Hidden,
    Located,
    Working,
    Carried,
    Complete,
    Failed,
    Stable,
    Unstable,
    ClinicalDue(u8),
    ClinicalSettled(u8),
    NextClinical,
    ClinicalCount,
    Cargo(u8),
    Seat,
    ReservedSeat(u8),
    Supply,
    UsedSupply,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    World,
    Scout,
    Medic,
    Nature,
    Objective,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Phase {
    Hidden,
    Located,
    Carried,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Depart {
        agent: Agent,
        from: Location,
        to: Location,
    },
    Arrive {
        agent: Agent,
        to: Location,
        hazardous: bool,
    },
    Survey(Location),
    Discover {
        objective: u8,
        location: Location,
    },
    Scan,
    Report {
        round: u8,
        location: Location,
    },
    Share {
        objective: u8,
        location: Location,
        remote: bool,
    },
    Collect {
        objective: u8,
        location: Location,
    },
    FinishCollect(u8),
    TreatPatient {
        objective: u8,
        location: Location,
        restore: bool,
    },
    Extract {
        objective: u8,
        location: Location,
    },
    FinishExtract(u8),
    Deliver(u8),
    TreatAgent {
        target: Agent,
        location: Location,
    },
    Resupply(Agent),
    Weather {
        location: Location,
        opening: bool,
    },
    Clinical {
        objective: u8,
        day: u8,
        active: bool,
    },
    FinishClinical(u8),
    Fail {
        objective: u8,
        phase: Phase,
        dead: bool,
    },
    Tick(u8),
    Close,
}
pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `mission/expedition` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "mission/expedition";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;
pub fn patient(id: u8) -> bool {
    id.is_multiple_of(2)
}
pub fn value(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
}
pub fn now(world: &World) -> u64 {
    value(world, AccountId::World, Asset::Time)
}
pub fn distance(from: Location, to: Location) -> u64 {
    if from == to {
        return 0;
    }
    let mut queue = std::collections::VecDeque::from([(from, 0)]);
    let mut seen = std::collections::BTreeSet::from([from]);
    while let Some((at, cost)) = queue.pop_front() {
        for (a, b) in ROADS {
            let next = if a == at {
                b
            } else if b == at {
                a
            } else {
                continue;
            };
            if next == to {
                return cost + 1;
            }
            if seen.insert(next) {
                queue.push_back((next, cost + 1));
            }
        }
    }
    unreachable!("connected mission roads")
}
pub fn initial(stress: bool, seed: u64) -> World {
    let count: u8 = if stress { 3 } else { 2 };
    let sites = &LOCATIONS[1..];
    let offset = (seed % 4) as usize;
    let stride = if seed & 4 == 0 { 1 } else { 3 };
    let locations: Vec<_> = (0..count as usize)
        .map(|id| sites[(offset + id * stride) % 4])
        .collect();
    let hazard = locations[((seed / 4) % u64::from(count)) as usize];
    let mut hidden = basket([(Asset::HazardSite(hazard), 1)]);
    for (id, location) in locations.iter().copied().enumerate() {
        hidden.insert(Asset::Truth(id as u8, location), Quantity::new(1));
    }
    for round in 0..2 {
        hidden.insert(
            Asset::Signal(
                round,
                if (seed % 4 + u64::from(round)).is_multiple_of(4) {
                    sites[(offset + count as usize) % 4]
                } else {
                    locations[usize::from(round)]
                },
            ),
            Quantity::new(1),
        );
    }
    let reference = if stress {
        axionomy::path!("mission/stress")
    } else {
        axionomy::path!("mission/showcase")
    };
    crate::fixtures::variant(reference, |instance| {
        *crate::fixtures::account(instance, AccountId::Nature) = Account::from(hidden);
    })
}

/// The exchange of `rate` once, each of its roles bound to the account
/// `world`'s rate fixes it to.
///
/// # Panics
///
/// If `rate` names no instance of `world`'s rates.
pub fn action(world: &World, rate: RateId) -> Action {
    let instance = world
        .instance(&rate)
        .unwrap_or_else(|_| panic!("the expedition has no rate {rate:?}"));
    instance.fixed_accounts().fold(
        Exchange::new(rate, Quantity::new(1)),
        |exchange, (role, account)| {
            exchange.bind(
                *role,
                *account
                    .as_literal()
                    .expect("an instance's accounts are written out"),
            )
        },
    )
}
pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::World, basket([(Asset::Closed, 1)]))
}
/// The most steps an expedition takes before it is cut short.
pub const HORIZON: usize = 500;

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_all(world: &mut World, rates: &[RateId]) {
        for rate in rates {
            world
                .apply(action(world, *rate))
                .unwrap_or_else(|error| panic!("{rate:?}: {error}"));
        }
    }

    #[test]
    fn hidden_scenarios_differ_only_in_what_the_agents_cannot_see() {
        let a = initial(false, 0);
        let b = initial(false, 17);
        assert_ne!(a.state_key(), b.state_key());
        for agent in AGENTS {
            let seen = [AccountId::World, AccountId::Agent(agent)];
            assert_eq!(
                a.view(seen).observation_key(),
                b.view(seen).observation_key()
            );
        }
    }
    #[test]
    fn base_resupply_does_not_teleport_medicine_to_an_absent_medic() {
        let mut world = initial(false, 17);
        world
            .apply(action(
                &world,
                RateId::Depart {
                    agent: Agent::Medic,
                    from: Location::Base,
                    to: Location::North,
                },
            ))
            .unwrap();
        world
            .apply(action(&world, RateId::Resupply(Agent::Scout)))
            .unwrap();
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Medic), Asset::MedicalKit),
            2
        );
        assert_eq!(value(&world, AccountId::World, Asset::MedicalKit), 2);
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Scout), Asset::Energy),
            20
        );
        assert!(!world.is_applicable(&action(&world, RateId::Resupply(Agent::Medic))));
    }
    #[test]
    fn private_discovery_requires_a_real_briefing_before_joint_extraction() {
        let east = Location::East;
        let known =
            |world: &World, agent| value(world, AccountId::Agent(agent), Asset::Known(0, east));
        let mut world = initial(false, 17);
        apply_all(
            &mut world,
            &[
                RateId::Scan,
                RateId::Report {
                    round: 0,
                    location: east,
                },
                RateId::Tick(0),
                RateId::Depart {
                    agent: Agent::Scout,
                    from: Location::Base,
                    to: east,
                },
                RateId::Tick(1),
                RateId::Tick(2),
                RateId::Arrive {
                    agent: Agent::Scout,
                    to: east,
                    hazardous: false,
                },
                RateId::Survey(east),
                RateId::Discover {
                    objective: 0,
                    location: east,
                },
            ],
        );
        let extract = action(
            &world,
            RateId::Extract {
                objective: 0,
                location: east,
            },
        );
        assert_eq!(known(&world, Agent::Scout), 1);
        assert_eq!(known(&world, Agent::Medic), 0);
        assert!(!world.is_applicable(&extract));

        apply_all(
            &mut world,
            &[
                RateId::Tick(3),
                RateId::Share {
                    objective: 0,
                    location: east,
                    remote: true,
                },
            ],
        );
        assert_eq!(known(&world, Agent::Medic), 1);
        assert_eq!(
            value(&world, AccountId::Agent(Agent::Scout), Asset::UsedRadio),
            1
        );

        apply_all(
            &mut world,
            &[
                RateId::Depart {
                    agent: Agent::Medic,
                    from: Location::Base,
                    to: east,
                },
                RateId::Tick(4),
                RateId::Weather {
                    location: east,
                    opening: false,
                },
                RateId::Tick(5),
                RateId::Arrive {
                    agent: Agent::Medic,
                    to: east,
                    hazardous: true,
                },
                RateId::TreatPatient {
                    objective: 0,
                    location: east,
                    restore: false,
                },
                RateId::Tick(6),
            ],
        );
        assert_eq!(value(&world, AccountId::Objective(0), Asset::Stable), 1);
        for agent in AGENTS {
            assert_eq!(value(&world, AccountId::Agent(agent), Asset::At(east)), 1);
        }
        let rebound = extract
            .clone()
            .bind(Role::Medic, AccountId::Agent(Agent::Scout));
        assert!(!world.is_applicable(&rebound));
        world.apply(extract).unwrap();
    }
    #[test]
    fn movement_occupies_agents_and_scheduled_updates_cannot_be_skipped() {
        let mut world = initial(false, 0);
        world
            .apply(action(
                &world,
                RateId::Depart {
                    agent: Agent::Scout,
                    from: Location::Base,
                    to: Location::North,
                },
            ))
            .unwrap();
        world
            .apply(action(
                &world,
                RateId::Depart {
                    agent: Agent::Medic,
                    from: Location::Base,
                    to: Location::South,
                },
            ))
            .unwrap();
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Arrive {
                agent: Agent::Scout,
                to: Location::North,
                hazardous: false
            }
        )));
        assert!(!world.is_applicable(&action(&world, RateId::Survey(Location::North))));
        apply_all(
            &mut world,
            &[
                RateId::Tick(0),
                RateId::Arrive {
                    agent: Agent::Scout,
                    to: Location::North,
                    hazardous: false,
                },
                RateId::Arrive {
                    agent: Agent::Medic,
                    to: Location::South,
                    hazardous: false,
                },
            ],
        );
        assert_eq!(
            value(
                &world,
                AccountId::Agent(Agent::Scout),
                Asset::At(Location::North)
            ),
            1
        );
        assert_eq!(
            value(
                &world,
                AccountId::Agent(Agent::Medic),
                Asset::At(Location::South)
            ),
            1
        );
        for tick in 1..5 {
            world.apply(action(&world, RateId::Tick(tick))).unwrap();
        }
        assert!(!world.is_applicable(&action(&world, RateId::Tick(5))));
        assert!(!world.is_applicable(&action(&world, RateId::Survey(Location::North))));
        apply_all(
            &mut world,
            &[RateId::Weather {
                location: Location::North,
                opening: false,
            }],
        );
        for tick in 5..8 {
            world.apply(action(&world, RateId::Tick(tick))).unwrap();
        }
        assert!(!world.is_applicable(&action(&world, RateId::Tick(8))));
        assert!(!world.is_applicable(&action(&world, RateId::Survey(Location::North))));
        apply_all(
            &mut world,
            &[
                RateId::Clinical {
                    objective: 0,
                    day: 8,
                    active: true,
                },
                RateId::FinishClinical(8),
            ],
        );
        assert_eq!(value(&world, AccountId::Objective(0), Asset::Health), 2);
        assert!(world.is_applicable(&action(&world, RateId::Tick(8))));
    }
}
