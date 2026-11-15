#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, basket,
};
pub mod expedition;
#[cfg(feature = "packages")]
pub mod package;

/// The most exchanges a Micro run takes, Nature's draw among them.
pub const HORIZON: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AgentId {
    Scout,
    Medic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Base,
    North,
    South,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Hazard {
    Safe,
    Injury,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Agent(AgentId),
    Nature,
    Mission,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    AgentIdentity(AgentId),
    NatureIdentity,
    MissionIdentity,
    SuccessIdentity,
    At(Location),
    Energy,
    SpentEnergy,
    Sensor,
    UsedSensor,
    MedicalKit,
    UsedMedicalKit,
    Intel(Location),
    SharedIntel(Location),
    Injured,
    Unresolved,
    ScenarioWeight(Location, u8, Hazard),
    Truth(Location),
    Seed(u8),
    Hazard(Hazard),
    HazardResolved,
    TimeRemaining,
    ElapsedTime,
    Planning,
    AwaitingScan,
    AwaitingEncounter,
    NeedsTreatment,
    AreaSafe,
    Rescued,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Scout,
    Medic,
    Nature,
    Mission,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Instantiate {
        truth: Location,
        seed: u8,
        hazard: Hazard,
    },
    BeginScan,
    ResolveScan {
        truth: Location,
        seed: u8,
        report: Location,
        next_seed: u8,
    },
    Share(Location),
    MoveTogether(Location),
    MoveDirect(Location),
    Encounter {
        location: Location,
        hazard: Hazard,
    },
    Treat,
    Rescue(Location),
    Finish,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `mission` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "mission";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn initial() -> World {
    let mut nature = basket([(Asset::Unresolved, 1)]);
    for truth in [Location::North, Location::South] {
        for seed in 0..4 {
            for hazard in [Hazard::Safe, Hazard::Injury] {
                nature.insert(Asset::ScenarioWeight(truth, seed, hazard), Quantity::new(1));
            }
        }
    }

    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent(AgentId::Scout),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::Scout), 1),
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::Sensor, 1),
            ])),
        )
        .account(
            AccountId::Agent(AgentId::Medic),
            Account::from(basket([
                (Asset::AgentIdentity(AgentId::Medic), 1),
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::MedicalKit, 1),
            ])),
        )
        .account(AccountId::Nature, {
            nature.insert(Asset::NatureIdentity, Quantity::new(1));
            Account::from(nature)
        })
        .account(
            AccountId::Mission,
            Account::from(basket([
                (Asset::MissionIdentity, 1),
                (Asset::TimeRemaining, 10),
                (Asset::Planning, 1),
            ])),
        )
        .account(
            AccountId::Success,
            Account::from(basket([(Asset::SuccessIdentity, 1)])),
        );

    for truth in [Location::North, Location::South] {
        for seed in 0..4 {
            for hazard in [Hazard::Safe, Hazard::Injury] {
                builder = builder.rate(
                    RateId::Instantiate {
                        truth,
                        seed,
                        hazard,
                    },
                    Rate::new()
                        .preserve(Role::Nature, basket([(Asset::NatureIdentity, 1)]))
                        .consume(Role::Nature, basket([(Asset::Unresolved, 1)]))
                        .preserve(
                            Role::Nature,
                            basket([(Asset::ScenarioWeight(truth, seed, hazard), 1)]),
                        )
                        .produce(
                            Role::Nature,
                            basket([
                                (Asset::Truth(truth), 1),
                                (Asset::Seed(seed), 1),
                                (Asset::Hazard(hazard), 1),
                            ]),
                        ),
                );
            }

            let report = signal(truth, seed);
            builder = builder.rate(
                RateId::ResolveScan {
                    truth,
                    seed,
                    report,
                    next_seed: (seed + 1) % 4,
                },
                Rate::new()
                    .preserve(
                        Role::Scout,
                        basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
                    )
                    .preserve(Role::Nature, basket([(Asset::NatureIdentity, 1)]))
                    .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                    .produce(Role::Scout, basket([(Asset::Intel(report), 1)]))
                    .preserve(Role::Nature, basket([(Asset::Truth(truth), 1)]))
                    .consume(Role::Nature, basket([(Asset::Seed(seed), 1)]))
                    .produce(Role::Nature, basket([(Asset::Seed((seed + 1) % 4), 1)]))
                    .consume(Role::Mission, basket([(Asset::AwaitingScan, 1)]))
                    .produce(Role::Mission, basket([(Asset::Planning, 1)]))
                    .distinct(Role::Scout, Role::Nature)
                    .distinct(Role::Scout, Role::Mission)
                    .distinct(Role::Nature, Role::Mission),
            );
        }
    }

    builder = builder.rate(
        RateId::BeginScan,
        Rate::new()
            .preserve(
                Role::Scout,
                basket([
                    (Asset::AgentIdentity(AgentId::Scout), 1),
                    (Asset::At(Location::Base), 1),
                ]),
            )
            .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
            .consume(Role::Scout, basket([(Asset::Sensor, 1)]))
            .produce(Role::Scout, basket([(Asset::UsedSensor, 1)]))
            .consume(
                Role::Mission,
                basket([(Asset::Planning, 1), (Asset::TimeRemaining, 1)]),
            )
            .produce(
                Role::Mission,
                basket([(Asset::AwaitingScan, 1), (Asset::ElapsedTime, 1)]),
            )
            .distinct(Role::Scout, Role::Mission),
    );

    for location in [Location::North, Location::South] {
        builder = builder
            .rate(
                RateId::Share(location),
                Rate::new()
                    .preserve(
                        Role::Scout,
                        basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
                    )
                    .preserve(
                        Role::Medic,
                        basket([(Asset::AgentIdentity(AgentId::Medic), 1)]),
                    )
                    .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                    .consume(Role::Scout, basket([(Asset::Intel(location), 1)]))
                    .produce(Role::Scout, basket([(Asset::SharedIntel(location), 1)]))
                    .produce(Role::Medic, basket([(Asset::Intel(location), 1)]))
                    .preserve(Role::Mission, basket([(Asset::Planning, 1)]))
                    .consume(Role::Mission, basket([(Asset::TimeRemaining, 1)]))
                    .produce(Role::Mission, basket([(Asset::ElapsedTime, 1)]))
                    .distinct(Role::Scout, Role::Medic)
                    .distinct(Role::Scout, Role::Mission)
                    .distinct(Role::Medic, Role::Mission),
            )
            .rate(
                RateId::MoveTogether(location),
                movement_rate(location, true),
            )
            .rate(RateId::MoveDirect(location), movement_rate(location, false));

        for hazard in [Hazard::Safe, Hazard::Injury] {
            let rate = Rate::new()
                .preserve(
                    Role::Scout,
                    basket([
                        (Asset::AgentIdentity(AgentId::Scout), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Medic,
                    basket([
                        (Asset::AgentIdentity(AgentId::Medic), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Nature,
                    basket([(Asset::NatureIdentity, 1), (Asset::Truth(location), 1)]),
                )
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .consume(Role::Nature, basket([(Asset::Hazard(hazard), 1)]))
                .produce(Role::Nature, basket([(Asset::HazardResolved, 1)]))
                .consume(
                    Role::Mission,
                    basket([(Asset::AwaitingEncounter, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(Role::Mission, basket([(Asset::ElapsedTime, 1)]))
                .distinct(Role::Scout, Role::Medic)
                .distinct(Role::Scout, Role::Nature)
                .distinct(Role::Scout, Role::Mission)
                .distinct(Role::Medic, Role::Nature)
                .distinct(Role::Medic, Role::Mission)
                .distinct(Role::Nature, Role::Mission);
            builder = builder.rate(
                RateId::Encounter { location, hazard },
                match hazard {
                    Hazard::Safe => rate.produce(Role::Mission, basket([(Asset::AreaSafe, 1)])),
                    Hazard::Injury => rate
                        .produce(Role::Scout, basket([(Asset::Injured, 1)]))
                        .produce(Role::Mission, basket([(Asset::NeedsTreatment, 1)])),
                },
            );
        }

        builder = builder.rate(
            RateId::Rescue(location),
            Rate::new()
                .preserve(
                    Role::Scout,
                    basket([
                        (Asset::AgentIdentity(AgentId::Scout), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Medic,
                    basket([
                        (Asset::AgentIdentity(AgentId::Medic), 1),
                        (Asset::At(location), 1),
                    ]),
                )
                .preserve(
                    Role::Nature,
                    basket([(Asset::NatureIdentity, 1), (Asset::Truth(location), 1)]),
                )
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .consume(
                    Role::Mission,
                    basket([(Asset::AreaSafe, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(
                    Role::Mission,
                    basket([(Asset::Rescued, 1), (Asset::ElapsedTime, 1)]),
                )
                .distinct(Role::Scout, Role::Medic)
                .distinct(Role::Scout, Role::Nature)
                .distinct(Role::Scout, Role::Mission)
                .distinct(Role::Medic, Role::Nature)
                .distinct(Role::Medic, Role::Mission)
                .distinct(Role::Nature, Role::Mission),
        );
    }

    builder
        .rate(
            RateId::Treat,
            Rate::new()
                .preserve(
                    Role::Scout,
                    basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
                )
                .preserve(
                    Role::Medic,
                    basket([(Asset::AgentIdentity(AgentId::Medic), 1)]),
                )
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .consume(Role::Scout, basket([(Asset::Injured, 1)]))
                .consume(Role::Medic, basket([(Asset::MedicalKit, 1)]))
                .produce(Role::Medic, basket([(Asset::UsedMedicalKit, 1)]))
                .consume(
                    Role::Mission,
                    basket([(Asset::NeedsTreatment, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(
                    Role::Mission,
                    basket([(Asset::AreaSafe, 1), (Asset::ElapsedTime, 1)]),
                )
                .distinct(Role::Scout, Role::Medic)
                .distinct(Role::Scout, Role::Mission)
                .distinct(Role::Medic, Role::Mission),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
                .preserve(Role::Goal, basket([(Asset::SuccessIdentity, 1)]))
                .consume(Role::Mission, basket([(Asset::Rescued, 1)]))
                .produce(Role::Goal, basket([(Asset::Solved, 1)]))
                .distinct(Role::Mission, Role::Goal),
        )
        .invariant(
            [Location::Base, Location::North, Location::South]
                .into_iter()
                .fold(
                    LinearInvariant::new(path!("two-agent-positions")),
                    |invariant, location| invariant.weight(Asset::At(location), 1),
                ),
        )
        .invariant(
            LinearInvariant::new(path!("energy-accounting"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("time-accounting"))
                .weight(Asset::TimeRemaining, 1)
                .weight(Asset::ElapsedTime, 1),
        )
        .invariant(
            LinearInvariant::new(path!("sensor-accounting"))
                .weight(Asset::Sensor, 1)
                .weight(Asset::UsedSensor, 1),
        )
        .invariant(
            LinearInvariant::new(path!("medical-kit-accounting"))
                .weight(Asset::MedicalKit, 1)
                .weight(Asset::UsedMedicalKit, 1),
        )
        .invariant(
            LinearInvariant::new(path!("mission-lifecycle"))
                .weight(Asset::Planning, 1)
                .weight(Asset::AwaitingScan, 1)
                .weight(Asset::AwaitingEncounter, 1)
                .weight(Asset::NeedsTreatment, 1)
                .weight(Asset::AreaSafe, 1)
                .weight(Asset::Rescued, 1)
                .weight(Asset::Solved, 1),
        )
        .invariant([Location::North, Location::South].into_iter().fold(
            LinearInvariant::new(path!("one-hidden-truth")).weight(Asset::Unresolved, 1),
            |invariant, location| invariant.weight(Asset::Truth(location), 1),
        ))
        .invariant((0..4).fold(
            LinearInvariant::new(path!("one-hidden-seed")).weight(Asset::Unresolved, 1),
            |invariant, seed| invariant.weight(Asset::Seed(seed), 1),
        ))
        .invariant(
            [Hazard::Safe, Hazard::Injury].into_iter().fold(
                LinearInvariant::new(path!("one-hidden-hazard"))
                    .weight(Asset::Unresolved, 1)
                    .weight(Asset::HazardResolved, 1),
                |invariant, hazard| invariant.weight(Asset::Hazard(hazard), 1),
            ),
        )
        .build()
        .expect("mission model is valid")
}

fn movement_rate(location: Location, coordinated: bool) -> Rate<Ontology> {
    let rate = Rate::new()
        .preserve(
            Role::Scout,
            basket([(Asset::AgentIdentity(AgentId::Scout), 1)]),
        )
        .preserve(
            Role::Medic,
            basket([(Asset::AgentIdentity(AgentId::Medic), 1)]),
        )
        .preserve(Role::Mission, basket([(Asset::MissionIdentity, 1)]))
        .consume(
            Role::Scout,
            basket([(Asset::At(Location::Base), 1), (Asset::Energy, 1)]),
        )
        .produce(
            Role::Scout,
            basket([(Asset::At(location), 1), (Asset::SpentEnergy, 1)]),
        )
        .consume(
            Role::Medic,
            basket([(Asset::At(Location::Base), 1), (Asset::Energy, 1)]),
        )
        .produce(
            Role::Medic,
            basket([(Asset::At(location), 1), (Asset::SpentEnergy, 1)]),
        )
        .consume(Role::Mission, basket([(Asset::Planning, 1)]))
        .consume(Role::Mission, basket([(Asset::TimeRemaining, 1)]))
        .produce(
            Role::Mission,
            basket([(Asset::AwaitingEncounter, 1), (Asset::ElapsedTime, 1)]),
        )
        .distinct(Role::Scout, Role::Medic)
        .distinct(Role::Scout, Role::Mission)
        .distinct(Role::Medic, Role::Mission);
    if coordinated {
        rate.preserve(Role::Scout, basket([(Asset::SharedIntel(location), 1)]))
            .preserve(Role::Medic, basket([(Asset::Intel(location), 1)]))
    } else {
        // The unused sensor proves this direct commitment was made before a
        // scan. Once the Scout observes, coordination requires explicit share.
        rate.preserve(Role::Scout, basket([(Asset::Sensor, 1)]))
    }
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Success, basket([(Asset::Solved, 1)]))
}

fn signal(truth: Location, seed: u8) -> Location {
    if seed == 0 {
        match truth {
            Location::North => Location::South,
            Location::South => Location::North,
            Location::Base => unreachable!("base is not a hidden mission site"),
        }
    } else {
        truth
    }
}

/// The exchange of one unit of `rate`, each of its roles bound to the
/// account that plays it.
pub fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    match rate {
        RateId::Instantiate { .. } => exchange.bind(Role::Nature, AccountId::Nature),
        RateId::BeginScan => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Mission, AccountId::Mission),
        RateId::ResolveScan { .. } => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission),
        RateId::Share(_) | RateId::MoveTogether(_) | RateId::MoveDirect(_) | RateId::Treat => {
            exchange
                .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
                .bind(Role::Medic, AccountId::Agent(AgentId::Medic))
                .bind(Role::Mission, AccountId::Mission)
        }
        RateId::Encounter { .. } | RateId::Rescue(_) => exchange
            .bind(Role::Scout, AccountId::Agent(AgentId::Scout))
            .bind(Role::Medic, AccountId::Agent(AgentId::Medic))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission),
        RateId::Finish => exchange
            .bind(Role::Mission, AccountId::Mission)
            .bind(Role::Goal, AccountId::Success),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::Trace;

    /// What the Scout's view holds: its own account and the team's.
    const SCOUT_SEES: [AccountId; 3] = [
        AccountId::Agent(AgentId::Scout),
        AccountId::Mission,
        AccountId::Success,
    ];

    #[test]
    fn the_scout_holds_the_same_before_scanning_whatever_the_truth() {
        let instantiated = |truth| {
            let mut world = initial();
            world
                .apply(action(RateId::Instantiate {
                    truth,
                    seed: 0,
                    hazard: Hazard::Injury,
                }))
                .expect("the prior encodes the scenario");
            world
        };
        let north = instantiated(Location::North);
        let south = instantiated(Location::South);

        assert_ne!(north.state_key(), south.state_key());
        assert_eq!(
            north.view(SCOUT_SEES).observation_key(),
            south.view(SCOUT_SEES).observation_key()
        );
        let scan = action(RateId::BeginScan);
        assert!(north.is_applicable(&scan));
        assert!(south.is_applicable(&scan));
    }

    #[test]
    fn coordinated_sample_replays_with_intelligence_transfer() {
        let model = initial();
        let rates = [
            RateId::Instantiate {
                truth: Location::North,
                seed: 1,
                hazard: Hazard::Safe,
            },
            RateId::BeginScan,
            RateId::ResolveScan {
                truth: Location::North,
                seed: 1,
                report: Location::North,
                next_seed: 2,
            },
            RateId::Share(Location::North),
            RateId::MoveTogether(Location::North),
            RateId::Encounter {
                location: Location::North,
                hazard: Hazard::Safe,
            },
            RateId::Rescue(Location::North),
            RateId::Finish,
        ];
        let trace: Trace<Ontology> = rates.into_iter().map(action).collect();

        // The Medic follows the Scout's intelligence only once it is shared.
        let scanned = model
            .replayed(&trace.exchanges()[..3].iter().cloned().collect())
            .expect("the scan replays");
        assert!(!scanned.is_applicable(&action(RateId::MoveTogether(Location::North))));

        let replayed = model.replayed(&trace).expect("mission trace must replay");
        assert!(replayed.matches(&goal()));
        assert!(
            replayed
                .rate_ids()
                .filter_map(|rate| rate.as_literal())
                .all(|rate| !replayed.is_applicable(&action(*rate)))
        );
    }

    #[test]
    fn encoded_roles_reject_resolving_a_scan_into_the_medics_account() {
        let mut world = initial();
        world
            .apply(action(RateId::Instantiate {
                truth: Location::North,
                seed: 0,
                hazard: Hazard::Injury,
            }))
            .expect("the prior encodes the scenario");
        world
            .apply(action(RateId::BeginScan))
            .expect("scout starts scan");
        let resolution = RateId::ResolveScan {
            truth: Location::North,
            seed: 0,
            report: Location::South,
            next_seed: 1,
        };
        assert!(world.is_applicable(&action(resolution)));
        let rebound = Exchange::new(resolution, Quantity::new(1))
            .bind(Role::Scout, AccountId::Agent(AgentId::Medic))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission);

        assert!(!world.is_applicable(&rebound));
    }
}
