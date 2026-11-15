#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, OneSegment, Param, Pattern,
    Quantity, QuantityExpression as Q, Rate, Relation, Rounding, Segment, Slot, basket,
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
    /// Nature answers a scan: the sensor at `seed` reports `report` of a
    /// victim at `truth`, and moves on to the seed after.
    ResolveScan {
        truth: Location,
        seed: u8,
        report: Location,
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

/// The sites the victim may be at, in the order of their identifiers.
const SITES: [Location; 2] = [Location::North, Location::South];
/// The seeds a scan's sensor cycles through.
const SEEDS: u8 = 4;

/// One unit of each asset `assets` name.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}

/// `rate` with each of `roles` fixed to the account that plays it.
fn played_by(rate: Rate<Ontology>, roles: &[Role]) -> Rate<Ontology> {
    roles.iter().fold(rate, |rate, role| {
        let account = match role {
            Role::Scout => AccountId::Agent(AgentId::Scout),
            Role::Medic => AccountId::Agent(AgentId::Medic),
            Role::Nature => AccountId::Nature,
            Role::Mission => AccountId::Mission,
            Role::Goal => AccountId::Success,
        };
        rate.account(*role, Pattern::literal(account))
    })
}

pub fn initial() -> World {
    let mut nature = basket([(Asset::Unresolved, 1)]);
    for truth in SITES {
        for seed in 0..SEEDS {
            for hazard in [Hazard::Safe, Hazard::Injury] {
                nature.insert(Asset::ScenarioWeight(truth, seed, hazard), Quantity::new(1));
            }
        }
    }

    let builder = EconomyBuilder::new()
        .account(
            AccountId::Agent(AgentId::Scout),
            Account::from(basket([
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::Sensor, 1),
            ])),
        )
        .account(
            AccountId::Agent(AgentId::Medic),
            Account::from(basket([
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::MedicalKit, 1),
            ])),
        )
        .account(AccountId::Nature, Account::from(nature))
        .account(
            AccountId::Mission,
            Account::from(basket([(Asset::TimeRemaining, 10), (Asset::Planning, 1)])),
        )
        .account(AccountId::Success, Account::from(axionomy::Basket::new()));

    // The rates' variables: the victim's site, the sensor's seed, the
    // site's hazard, the site a scan reports, the seed after it, which the
    // law derives, and a site the team goes to.
    let truth = Param::<Location>::new(path!("truth"));
    let seed = Param::<u8>::new(path!("seed"));
    let hazard = Param::<Hazard>::new(path!("hazard"));
    let report = Param::<Location>::new(path!("report"));
    let next = Param::<u8>::new(path!("next"));
    let site = Param::<Location>::new(path!("location"));
    // What a scan reports of each site at each seed: the truth, but for
    // seed 0, which reports the other site.
    let readings = Relation::new(
        [
            truth.name().clone(),
            seed.name().clone(),
            report.name().clone(),
        ],
        SITES.into_iter().flat_map(|place| {
            (0..SEEDS).map(move |state| {
                [
                    place.segment(),
                    Segment::from(u64::from(state)),
                    signal(place, state).segment(),
                ]
            })
        }),
    );
    let after = || seed.quantity() + Q::constant(1);
    let team = [Role::Scout, Role::Medic, Role::Mission];
    let at_site = || one([AssetPattern::At(site.var()).into()]);

    let mut builder = builder
        .rate_at(
            RateIdPattern::Instantiate {
                truth: truth.var(),
                seed: seed.var(),
                hazard: hazard.var(),
            }
            .into(),
            played_by(Rate::new(), &[Role::Nature])
                .variable(truth.values(SITES))
                .variable(seed.range(0, SEEDS - 1))
                // In the order of their identifiers, as Nature's prior lists
                // the scenarios.
                .variable(hazard.values([Hazard::Injury, Hazard::Safe]))
                .consume(Role::Nature, basket([(Asset::Unresolved, 1)]))
                .preserve_at(
                    Role::Nature,
                    one([
                        AssetPattern::ScenarioWeight(truth.var(), seed.var(), hazard.var()).into(),
                    ]),
                )
                .produce_at(
                    Role::Nature,
                    one([
                        AssetPattern::Truth(truth.var()).into(),
                        AssetPattern::Seed(seed.var()).into(),
                        AssetPattern::Hazard(hazard.var()).into(),
                    ]),
                ),
        )
        .rate_at(
            RateIdPattern::ResolveScan {
                truth: truth.var(),
                seed: seed.var(),
                report: report.var(),
            }
            .into(),
            played_by(Rate::new(), &[Role::Scout, Role::Nature, Role::Mission])
                .variable(truth.values(SITES))
                .variable(seed.range(0, SEEDS - 1))
                .variable(report.values(SITES))
                .relation(readings)
                // The seed after it, the first again after the last.
                .derive(
                    next.name().clone(),
                    after()
                        - Q::divide(after(), Q::constant(SEEDS.into()), Rounding::Floor)
                            * Q::constant(SEEDS.into()),
                )
                .produce_at(Role::Scout, one([AssetPattern::Intel(report.var()).into()]))
                .preserve_at(Role::Nature, one([AssetPattern::Truth(truth.var()).into()]))
                .consume_at(Role::Nature, one([AssetPattern::Seed(seed.var()).into()]))
                .produce_at(Role::Nature, one([AssetPattern::Seed(next.var()).into()]))
                .consume(Role::Mission, basket([(Asset::AwaitingScan, 1)]))
                .produce(Role::Mission, basket([(Asset::Planning, 1)])),
        )
        .rate(
            RateId::BeginScan,
            played_by(Rate::new(), &[Role::Scout, Role::Mission])
                .preserve(Role::Scout, basket([(Asset::At(Location::Base), 1)]))
                .consume(Role::Scout, basket([(Asset::Sensor, 1)]))
                .produce(Role::Scout, basket([(Asset::UsedSensor, 1)]))
                .consume(
                    Role::Mission,
                    basket([(Asset::Planning, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(
                    Role::Mission,
                    basket([(Asset::AwaitingScan, 1), (Asset::ElapsedTime, 1)]),
                ),
        )
        .rate_at(
            RateIdPattern::Share(site.var()).into(),
            played_by(Rate::new(), &team)
                .variable(site.values(SITES))
                .consume_at(Role::Scout, one([AssetPattern::Intel(site.var()).into()]))
                .produce_at(
                    Role::Scout,
                    one([AssetPattern::SharedIntel(site.var()).into()]),
                )
                .produce_at(Role::Medic, one([AssetPattern::Intel(site.var()).into()]))
                .preserve(Role::Mission, basket([(Asset::Planning, 1)]))
                .consume(Role::Mission, basket([(Asset::TimeRemaining, 1)]))
                .produce(Role::Mission, basket([(Asset::ElapsedTime, 1)])),
        );
    for coordinated in [true, false] {
        let movement = movement_rate(&site, coordinated);
        builder = builder.rate_at(
            if coordinated {
                RateIdPattern::MoveTogether(site.var())
            } else {
                RateIdPattern::MoveDirect(site.var())
            }
            .into(),
            movement,
        );
    }
    // At the true site Nature resolves the hazard: safe, or an injury the
    // Medic must treat.
    let on_site = || {
        played_by(
            Rate::new(),
            &[Role::Scout, Role::Medic, Role::Nature, Role::Mission],
        )
        .variable(site.values(SITES))
        .preserve_at(Role::Scout, at_site())
        .preserve_at(Role::Medic, at_site())
        .preserve_at(Role::Nature, one([AssetPattern::Truth(site.var()).into()]))
    };
    for found in [Hazard::Safe, Hazard::Injury] {
        let rate = on_site()
            .consume(Role::Nature, basket([(Asset::Hazard(found), 1)]))
            .produce(Role::Nature, basket([(Asset::HazardResolved, 1)]))
            .consume(
                Role::Mission,
                basket([(Asset::AwaitingEncounter, 1), (Asset::TimeRemaining, 1)]),
            )
            .produce(Role::Mission, basket([(Asset::ElapsedTime, 1)]));
        builder = builder.rate_at(
            RateIdPattern::Encounter {
                location: site.var(),
                hazard: Slot::Is(found),
            }
            .into(),
            match found {
                Hazard::Safe => rate.produce(Role::Mission, basket([(Asset::AreaSafe, 1)])),
                Hazard::Injury => rate
                    .produce(Role::Scout, basket([(Asset::Injured, 1)]))
                    .produce(Role::Mission, basket([(Asset::NeedsTreatment, 1)])),
            },
        );
    }
    builder
        .rate_at(
            RateIdPattern::Rescue(site.var()).into(),
            on_site()
                .consume(
                    Role::Mission,
                    basket([(Asset::AreaSafe, 1), (Asset::TimeRemaining, 1)]),
                )
                .produce(
                    Role::Mission,
                    basket([(Asset::Rescued, 1), (Asset::ElapsedTime, 1)]),
                ),
        )
        .rate(
            RateId::Treat,
            played_by(Rate::new(), &team)
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
                ),
        )
        .rate(
            RateId::Finish,
            played_by(Rate::new(), &[Role::Mission, Role::Goal])
                .consume(Role::Mission, basket([(Asset::Rescued, 1)]))
                .produce(Role::Goal, basket([(Asset::Solved, 1)])),
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
        .invariant(SITES.into_iter().fold(
            LinearInvariant::new(path!("one-hidden-truth")).weight(Asset::Unresolved, 1),
            |invariant, location| invariant.weight(Asset::Truth(location), 1),
        ))
        .invariant((0..SEEDS).fold(
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

/// Both teammates going from base to the site `{location}` names: on the
/// intelligence the Scout shared, or directly, before any scan.
fn movement_rate(site: &Param<Location>, coordinated: bool) -> Rate<Ontology> {
    let at_site = || one([AssetPattern::At(site.var()).into()]);
    let rate = played_by(Rate::new(), &[Role::Scout, Role::Medic, Role::Mission])
        .variable(site.values(SITES))
        .consume(
            Role::Scout,
            basket([(Asset::At(Location::Base), 1), (Asset::Energy, 1)]),
        )
        .produce_at(Role::Scout, at_site())
        .produce(Role::Scout, basket([(Asset::SpentEnergy, 1)]))
        .consume(
            Role::Medic,
            basket([(Asset::At(Location::Base), 1), (Asset::Energy, 1)]),
        )
        .produce_at(Role::Medic, at_site())
        .produce(Role::Medic, basket([(Asset::SpentEnergy, 1)]))
        .consume(Role::Mission, basket([(Asset::Planning, 1)]))
        .consume(Role::Mission, basket([(Asset::TimeRemaining, 1)]))
        .produce(
            Role::Mission,
            basket([(Asset::AwaitingEncounter, 1), (Asset::ElapsedTime, 1)]),
        );
    if coordinated {
        rate.preserve_at(
            Role::Scout,
            one([AssetPattern::SharedIntel(site.var()).into()]),
        )
        .preserve_at(Role::Medic, one([AssetPattern::Intel(site.var()).into()]))
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

/// The exchange of `rate` once, each of its roles bound to the account
/// `world`'s rate fixes it to.
///
/// # Panics
///
/// If `rate` names no instance of `world`'s rates.
pub fn action(world: &World, rate: RateId) -> Action {
    let instance = world
        .instance(&rate)
        .unwrap_or_else(|_| panic!("the mission has no rate {rate:?}"));
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
                .apply(action(
                    &world,
                    RateId::Instantiate {
                        truth,
                        seed: 0,
                        hazard: Hazard::Injury,
                    },
                ))
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
        let scan = action(&north, RateId::BeginScan);
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
        let trace: Trace<Ontology> = rates.into_iter().map(|rate| action(&model, rate)).collect();

        // The Medic follows the Scout's intelligence only once it is shared.
        let scanned = model
            .replayed(&trace.exchanges()[..3].iter().cloned().collect())
            .expect("the scan replays");
        assert!(!scanned.is_applicable(&action(&scanned, RateId::MoveTogether(Location::North))));

        let replayed = model.replayed(&trace).expect("mission trace must replay");
        assert!(replayed.matches(&goal()));
        assert!(
            replayed
                .rate_ids()
                .flat_map(|key| replayed.instances_of(key))
                .all(|rate| !replayed.is_applicable(&action(&replayed, rate)))
        );
    }

    #[test]
    fn encoded_roles_reject_resolving_a_scan_into_the_medics_account() {
        let mut world = initial();
        world
            .apply(action(
                &world,
                RateId::Instantiate {
                    truth: Location::North,
                    seed: 0,
                    hazard: Hazard::Injury,
                },
            ))
            .expect("the prior encodes the scenario");
        world
            .apply(action(&world, RateId::BeginScan))
            .expect("scout starts scan");
        let resolution = RateId::ResolveScan {
            truth: Location::North,
            seed: 0,
            report: Location::South,
        };
        assert!(world.is_applicable(&action(&world, resolution)));
        let rebound = Exchange::new(resolution, Quantity::new(1))
            .bind(Role::Scout, AccountId::Agent(AgentId::Medic))
            .bind(Role::Nature, AccountId::Nature)
            .bind(Role::Mission, AccountId::Mission);

        assert!(!world.is_applicable(&rebound));
    }
}
