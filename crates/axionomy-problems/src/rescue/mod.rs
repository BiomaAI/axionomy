#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, OneSegment, Param, Pattern,
    Quantity, QuantityExpression as Q, Rate, Relation, Rounding, Segment, Slot, basket,
};

pub mod incident;
#[cfg(feature = "packages")]
pub mod package;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Base,
    North,
    South,
    East,
    West,
    Harbor,
    Hills,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RescueSize {
    Micro,
    Showcase,
    Stress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Agent,
    Nature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    At(Location),
    Energy,
    SpentEnergy,
    Sensor,
    UsedSensor,
    Unresolved,
    Truth(Location),
    Seed(u8),
    Belief(Location),
    Planning,
    AwaitingObservation,
    Committed,
    Rescued,
    Evacuated,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Actor,
    Nature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Instantiate {
        truth: Location,
        seed: u8,
    },
    BeginObserve,
    /// Nature answers an observation: the sensor at `seed` reports
    /// `report` of a survivor at `truth`, and moves on to the seed after.
    ResolveObservation {
        truth: Location,
        seed: u8,
        report: Location,
    },
    Move {
        from: Location,
        to: Location,
    },
    Rescue {
        location: Location,
    },
    Evacuate {
        location: Location,
    },
    Finish,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `rescue` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "rescue";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

/// Builds one already-resolved deterministic world.
pub fn scenario(truth: Location, seed: u8) -> World {
    assert_rescue_location(truth);
    let size = if matches!(truth, Location::Harbor | Location::Hills) || seed >= 8 {
        RescueSize::Stress
    } else if matches!(truth, Location::East | Location::West) || seed >= 4 {
        RescueSize::Showcase
    } else {
        RescueSize::Micro
    };
    assert!(
        seed < seed_count(size),
        "seed is outside the encoded sensor states"
    );
    build(
        Account::from(basket([(Asset::Truth(truth), 1), (Asset::Seed(seed), 1)])),
        &[],
        size,
    )
}

/// Builds an unresolved world from user-provided, core-encoded prior weights:
/// the rows of the draw's relation, a scenario of no weight having none.
pub fn uncertain(prior: impl IntoIterator<Item = (Location, u8, u64)>) -> World {
    let prior = prior.into_iter().collect::<Vec<_>>();
    let size = if prior
        .iter()
        .any(|(truth, seed, _)| matches!(truth, Location::Harbor | Location::Hills) || *seed >= 8)
    {
        RescueSize::Stress
    } else if prior
        .iter()
        .any(|(truth, seed, _)| matches!(truth, Location::East | Location::West) || *seed >= 4)
    {
        RescueSize::Showcase
    } else {
        RescueSize::Micro
    };
    for (truth, seed, _) in &prior {
        assert_rescue_location(*truth);
        assert!(
            *seed < seed_count(size),
            "seed is outside the encoded sensor states"
        );
    }
    let prior: Vec<_> = prior
        .into_iter()
        .filter(|(_, _, weight)| *weight > 0)
        .collect();
    build(unresolved(), &prior, size)
}

/// Nature before it draws a scenario.
fn unresolved() -> Account<Asset> {
    Account::from(basket([(Asset::Unresolved, 1)]))
}

/// Every scenario of `size`'s sites and seeds, each as likely as another.
fn uniform(size: RescueSize) -> Vec<(Location, u8, u64)> {
    destinations(size)
        .iter()
        .flat_map(|truth| (0..seed_count(size)).map(move |seed| (*truth, seed, 1)))
        .collect()
}

pub fn uniform_uncertain() -> World {
    crate::fixtures::initial(axionomy::path!("rescue/micro"))
}

/// Four possible sites, twice as many sensor states, and a required return
/// evacuation after contact make this a longer partially observed mission.
pub fn uniform_uncertain_showcase() -> World {
    build(
        unresolved(),
        &uniform(RescueSize::Showcase),
        RescueSize::Showcase,
    )
}

/// Six possible sites and twelve sensor states per site pressure policy
/// evaluation across 72 explicit Nature scenarios.
pub fn uniform_uncertain_stress() -> World {
    build(
        unresolved(),
        &uniform(RescueSize::Stress),
        RescueSize::Stress,
    )
}

fn destinations(size: RescueSize) -> &'static [Location] {
    match size {
        RescueSize::Micro => &[Location::North, Location::South],
        RescueSize::Showcase => &[
            Location::North,
            Location::South,
            Location::East,
            Location::West,
        ],
        RescueSize::Stress => &[
            Location::North,
            Location::South,
            Location::East,
            Location::West,
            Location::Harbor,
            Location::Hills,
        ],
    }
}

const fn seed_count(size: RescueSize) -> u8 {
    match size {
        RescueSize::Micro => 4,
        RescueSize::Showcase => 8,
        RescueSize::Stress => 12,
    }
}

/// One unit of each asset `assets` name.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}

/// `rate` with the agent, its actor role fixed to the agent's account.
fn with_actor(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Actor, Pattern::literal(AccountId::Agent))
}

/// `rate` with Nature, its role fixed to Nature's account.
fn with_nature(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Nature, Pattern::literal(AccountId::Nature))
}

/// The world of `size` where Nature holds `nature` and draws among the
/// scenarios of `prior`, each `(truth, seed, weight)`.
fn build(nature: Account<Asset>, prior: &[(Location, u8, u64)], size: RescueSize) -> World {
    let destinations = destinations(size);
    let seed_count = seed_count(size);
    let extended = !matches!(size, RescueSize::Micro);
    let builder = EconomyBuilder::new()
        .account(
            AccountId::Agent,
            Account::from(basket([
                (Asset::At(Location::Base), 1),
                (Asset::Energy, if extended { 3 } else { 2 }),
                (Asset::Sensor, 1),
                (Asset::Planning, 1),
            ])),
        )
        .account(AccountId::Nature, nature);

    // The rates' variables: the survivor's site, the sensor's seed, the site
    // it reports, and the seed after it, which the law derives; and the
    // weight Nature draws each scenario by, which the prior's rows give.
    let truth = Param::<Location>::new(path!("truth"));
    let seed = Param::<u8>::new(path!("seed"));
    let report = Param::<Location>::new(path!("report"));
    let next = Param::<u8>::new(path!("next"));
    let to = Param::<Location>::new(path!("to"));
    let location = Param::<Location>::new(path!("location"));
    let weight = Param::<u64>::new(path!("weight"));
    let after = || seed.quantity() + Q::constant(1);
    // The sites the survivor may be at, in the order of their identifiers,
    // as the scenarios Nature draws among are ordered.
    let mut truths = destinations.to_vec();
    truths.sort_by_key(OneSegment::segment);
    // What the sensor reports of each site at each seed: the truth, but for
    // seed 0, which points elsewhere, even where the agent has no road.
    let readings: Vec<(Location, u8, Location)> = truths
        .iter()
        .flat_map(|&site| (0..seed_count).map(move |state| (site, state, signal(site, state))))
        .collect();
    let mut reported: Vec<Location> = readings.iter().map(|(_, _, report)| *report).collect();
    reported.sort_by_key(OneSegment::segment);
    reported.dedup();
    let readings = Relation::new(
        [
            truth.name().clone(),
            seed.name().clone(),
            report.name().clone(),
        ],
        readings.iter().map(|(site, state, report)| {
            [
                site.segment(),
                Segment::from(u64::from(*state)),
                report.segment(),
            ]
        }),
    );

    let mut builder = builder
        .rate_at(
            RateIdPattern::Instantiate {
                truth: truth.var(),
                seed: seed.var(),
            }
            .into(),
            with_nature(Rate::new())
                .variable(truth.values(truths.iter().copied()))
                .variable(seed.range(0, seed_count - 1))
                .relation(Relation::giving(
                    [truth.name().clone(), seed.name().clone()],
                    [weight.name().clone()],
                    prior.iter().map(|(site, state, odds)| {
                        [
                            site.segment(),
                            Segment::from(u64::from(*state)),
                            Segment::from(*odds),
                        ]
                    }),
                ))
                .consume(Role::Nature, basket([(Asset::Unresolved, 1)]))
                .produce_at(Role::Nature, one([AssetPattern::Truth(truth.var()).into()]))
                .produce_at(Role::Nature, one([AssetPattern::Seed(seed.var()).into()])),
        )
        .rate_at(
            RateIdPattern::ResolveObservation {
                truth: truth.var(),
                seed: seed.var(),
                report: report.var(),
            }
            .into(),
            with_nature(with_actor(Rate::new()))
                .variable(truth.values(truths.iter().copied()))
                .variable(seed.range(0, seed_count - 1))
                .variable(report.values(reported))
                .relation(readings)
                // The seed after it, the first again after the last.
                .derive(
                    next.name().clone(),
                    after()
                        - Q::divide(after(), Q::constant(seed_count.into()), Rounding::Floor)
                            * Q::constant(seed_count.into()),
                )
                .preserve(Role::Actor, basket([(Asset::At(Location::Base), 1)]))
                .consume(Role::Actor, basket([(Asset::AwaitingObservation, 1)]))
                .produce(Role::Actor, basket([(Asset::Planning, 1)]))
                .produce_at(
                    Role::Actor,
                    one([AssetPattern::Belief(report.var()).into()]),
                )
                .preserve_at(Role::Nature, one([AssetPattern::Truth(truth.var()).into()]))
                .consume_at(Role::Nature, one([AssetPattern::Seed(seed.var()).into()]))
                .produce_at(Role::Nature, one([AssetPattern::Seed(next.var()).into()])),
        )
        .rate(
            RateId::BeginObserve,
            with_actor(Rate::new())
                .preserve(Role::Actor, basket([(Asset::At(Location::Base), 1)]))
                .consume(
                    Role::Actor,
                    basket([(Asset::Planning, 1), (Asset::Sensor, 1)]),
                )
                .produce(
                    Role::Actor,
                    basket([(Asset::AwaitingObservation, 1), (Asset::UsedSensor, 1)]),
                ),
        )
        .rate_at(
            RateIdPattern::Move {
                from: Slot::Is(Location::Base),
                to: to.var(),
            }
            .into(),
            with_actor(Rate::new())
                .variable(to.values(destinations.iter().copied()))
                .consume(
                    Role::Actor,
                    basket([
                        (Asset::Planning, 1),
                        (Asset::At(Location::Base), 1),
                        (Asset::Energy, 1),
                    ]),
                )
                .produce(
                    Role::Actor,
                    basket([(Asset::Committed, 1), (Asset::SpentEnergy, 1)]),
                )
                .produce_at(Role::Actor, one([AssetPattern::At(to.var()).into()])),
        )
        .rate_at(
            RateIdPattern::Rescue {
                location: location.var(),
            }
            .into(),
            with_nature(with_actor(Rate::new()))
                .variable(location.values(destinations.iter().copied()))
                .consume(
                    Role::Actor,
                    basket([(Asset::Committed, 1), (Asset::Energy, 1)]),
                )
                .preserve_at(Role::Actor, one([AssetPattern::At(location.var()).into()]))
                .produce(
                    Role::Actor,
                    basket([(Asset::Rescued, 1), (Asset::SpentEnergy, 1)]),
                )
                .preserve_at(
                    Role::Nature,
                    one([AssetPattern::Truth(location.var()).into()]),
                ),
        );
    if extended {
        builder = builder.rate_at(
            RateIdPattern::Evacuate {
                location: location.var(),
            }
            .into(),
            with_actor(Rate::new())
                .variable(location.values(destinations.iter().copied()))
                .consume_at(
                    Role::Actor,
                    one([
                        Pattern::literal(Asset::Rescued),
                        AssetPattern::At(location.var()).into(),
                        Pattern::literal(Asset::Energy),
                    ]),
                )
                .produce(
                    Role::Actor,
                    basket([
                        (Asset::Evacuated, 1),
                        (Asset::At(Location::Base), 1),
                        (Asset::SpentEnergy, 1),
                    ]),
                ),
        );
    }

    let completion = if extended {
        Asset::Evacuated
    } else {
        Asset::Rescued
    };

    builder
        .rate(
            RateId::Finish,
            with_actor(Rate::new())
                .consume(Role::Actor, basket([(completion, 1)]))
                .produce(Role::Actor, basket([(Asset::Solved, 1)])),
        )
        .invariant(
            std::iter::once(Location::Base)
                .chain(destinations.iter().copied())
                .fold(
                    LinearInvariant::new(path!("one-agent-position")),
                    |invariant, location| invariant.weight(Asset::At(location), 1),
                ),
        )
        .invariant(
            LinearInvariant::new(path!("energy-accounting"))
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new(path!("sensor-accounting"))
                .weight(Asset::Sensor, 1)
                .weight(Asset::UsedSensor, 1),
        )
        .invariant((0..seed_count).fold(
            LinearInvariant::new(path!("one-nature-seed-state")).weight(Asset::Unresolved, 1),
            |invariant, seed| invariant.weight(Asset::Seed(seed), 1),
        ))
        .invariant(destinations.iter().copied().fold(
            LinearInvariant::new(path!("one-hidden-truth-state")).weight(Asset::Unresolved, 1),
            |invariant, location| invariant.weight(Asset::Truth(location), 1),
        ))
        .invariant(
            LinearInvariant::new(path!("rescue-lifecycle"))
                .weight(Asset::Planning, 1)
                .weight(Asset::AwaitingObservation, 1)
                .weight(Asset::Committed, 1)
                .weight(Asset::Rescued, 1)
                .weight(Asset::Evacuated, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("rescue model is valid")
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Agent, basket([(Asset::Solved, 1)]))
}

fn signal(truth: Location, seed: u8) -> Location {
    if seed == 0 {
        match truth {
            Location::North => Location::South,
            Location::South => Location::East,
            Location::East => Location::West,
            Location::West => Location::North,
            Location::Harbor => Location::Hills,
            Location::Hills => Location::North,
            Location::Base => unreachable!("base is not a rescue truth"),
        }
    } else {
        truth
    }
}

fn assert_rescue_location(location: Location) {
    assert!(
        matches!(
            location,
            Location::North
                | Location::South
                | Location::East
                | Location::West
                | Location::Harbor
                | Location::Hills
        ),
        "base is not a possible rescue truth"
    );
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
        .unwrap_or_else(|_| panic!("the rescue has no rate {rate:?}"));
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

    /// Observing, then following the report, with the survivor south and
    /// the sensor's seed 1: the public intent, Nature's resolution, the move
    /// the report points to, the rescue and the finish.
    const SENSED_SOUTH: [RateId; 5] = [
        RateId::BeginObserve,
        RateId::ResolveObservation {
            truth: Location::South,
            seed: 1,
            report: Location::South,
        },
        RateId::Move {
            from: Location::Base,
            to: Location::South,
        },
        RateId::Rescue {
            location: Location::South,
        },
        RateId::Finish,
    ];

    fn trace(world: &World, rates: impl IntoIterator<Item = RateId>) -> Trace<Ontology> {
        rates.into_iter().map(|rate| action(world, rate)).collect()
    }

    #[test]
    fn nature_resolves_an_observation_only_after_its_public_intent_and_replays() {
        let world = scenario(Location::South, 1);
        assert!(!world.is_applicable(&action(&world, SENSED_SOUTH[1])));

        let replay = world
            .replayed(&trace(&world, SENSED_SOUTH))
            .expect("the observed rescue replays deterministically");
        assert!(replay.matches(&goal()));
        assert_eq!(
            replay.balance(&AccountId::Agent, &Asset::SpentEnergy),
            Quantity::new(2)
        );
        assert_eq!(
            replay.balance(&AccountId::Agent, &Asset::UsedSensor),
            Quantity::new(1)
        );
    }

    #[test]
    fn hidden_nature_and_actor_roles_cannot_be_swapped() {
        let world = scenario(Location::South, 1);
        let mut awaiting = world.clone();
        awaiting
            .apply(action(&awaiting, RateId::BeginObserve))
            .expect("public observation intent applies");
        let rebound = Exchange::new(
            RateId::ResolveObservation {
                truth: Location::South,
                seed: 1,
                report: Location::South,
            },
            Quantity::new(1),
        )
        .bind(Role::Actor, AccountId::Nature)
        .bind(Role::Nature, AccountId::Agent);

        assert!(!awaiting.is_applicable(&rebound));
    }

    #[test]
    fn the_agent_holds_the_same_before_observing_whatever_the_truth() {
        let north = scenario(Location::North, 1);
        let south = scenario(Location::South, 0);

        assert_ne!(north.state_key(), south.state_key());
        assert_eq!(
            north.view([AccountId::Agent]).observation_key(),
            south.view([AccountId::Agent]).observation_key()
        );
        let intent = action(&north, RateId::BeginObserve);
        assert!(north.is_applicable(&intent));
        assert!(south.is_applicable(&intent));
    }

    #[test]
    fn sampled_trace_replays_from_the_uncertain_model() {
        let model = uniform_uncertain();
        let sampled = trace(
            &model,
            std::iter::once(RateId::Instantiate {
                truth: Location::South,
                seed: 1,
            })
            .chain(SENSED_SOUTH),
        );

        let replay = model
            .replayed(&sampled)
            .expect("sample and rescue must replay together");
        assert!(replay.matches(&goal()));
        assert!(
            replay
                .rate_ids()
                .flat_map(|key| replay.instances_of(key))
                .all(|rate| !replay.is_applicable(&action(&replay, rate)))
        );
    }

    #[test]
    fn stress_profile_encodes_more_hidden_scenarios() {
        let scenarios = |world: &World| {
            world
                .rate_ids()
                .flat_map(|key| world.instances_of(key))
                .filter(|rate| {
                    matches!(rate, RateId::Instantiate { .. })
                        && world.is_applicable(&action(world, *rate))
                })
                .count()
        };
        assert_eq!(scenarios(&uniform_uncertain()), 8);
        assert_eq!(scenarios(&uniform_uncertain_showcase()), 32);
        assert_eq!(scenarios(&uniform_uncertain_stress()), 72);
    }
}
