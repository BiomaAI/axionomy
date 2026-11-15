#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Basket, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    basket,
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
    ScenarioWeight(Location, u8),
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
    ResolveObservation {
        truth: Location,
        seed: u8,
        report: Location,
        next_seed: u8,
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
        size,
    )
}

/// Builds an unresolved world from user-provided, core-encoded prior weights.
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
    let mut nature = Basket::from([(Asset::Unresolved, Quantity::new(1))]);
    for (truth, seed, weight) in prior {
        assert_rescue_location(truth);
        assert!(
            seed < seed_count(size),
            "seed is outside the encoded sensor states"
        );
        nature.insert(Asset::ScenarioWeight(truth, seed), Quantity::new(weight));
    }
    build(Account::from(nature), size)
}

pub fn uniform_uncertain() -> World {
    uncertain(
        [Location::North, Location::South]
            .into_iter()
            .flat_map(|truth| (0..4).map(move |seed| (truth, seed, 1))),
    )
}

/// Four possible sites, twice as many sensor states, and a required return
/// evacuation after contact make this a longer partially observed mission.
pub fn uniform_uncertain_showcase() -> World {
    let mut nature = Basket::from([(Asset::Unresolved, Quantity::new(1))]);
    for truth in [
        Location::North,
        Location::South,
        Location::East,
        Location::West,
    ] {
        for seed in 0..8 {
            nature.insert(Asset::ScenarioWeight(truth, seed), Quantity::new(1));
        }
    }
    build(Account::from(nature), RescueSize::Showcase)
}

/// Six possible sites and twelve sensor states per site pressure policy
/// evaluation across 72 explicit Nature scenarios.
pub fn uniform_uncertain_stress() -> World {
    let mut nature = Basket::from([(Asset::Unresolved, Quantity::new(1))]);
    for truth in destinations(RescueSize::Stress) {
        for seed in 0..seed_count(RescueSize::Stress) {
            nature.insert(Asset::ScenarioWeight(*truth, seed), Quantity::new(1));
        }
    }
    build(Account::from(nature), RescueSize::Stress)
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

fn build(nature: Account<Asset>, size: RescueSize) -> World {
    let destinations = destinations(size);
    let seed_count = seed_count(size);
    let extended = !matches!(size, RescueSize::Micro);
    let mut builder = EconomyBuilder::new()
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

    for &encoded_truth in destinations {
        for encoded_seed in 0..seed_count {
            builder = builder.rate(
                RateId::Instantiate {
                    truth: encoded_truth,
                    seed: encoded_seed,
                },
                Rate::new()
                    .consume(Role::Nature, basket([(Asset::Unresolved, 1)]))
                    .preserve(
                        Role::Nature,
                        basket([(Asset::ScenarioWeight(encoded_truth, encoded_seed), 1)]),
                    )
                    .produce(
                        Role::Nature,
                        basket([
                            (Asset::Truth(encoded_truth), 1),
                            (Asset::Seed(encoded_seed), 1),
                        ]),
                    ),
            );

            let report = signal(encoded_truth, encoded_seed);
            let next_seed = (encoded_seed + 1) % seed_count;
            builder = builder.rate(
                RateId::ResolveObservation {
                    truth: encoded_truth,
                    seed: encoded_seed,
                    report,
                    next_seed,
                },
                Rate::new()
                    .preserve(Role::Actor, basket([(Asset::At(Location::Base), 1)]))
                    .consume(Role::Actor, basket([(Asset::AwaitingObservation, 1)]))
                    .produce(
                        Role::Actor,
                        basket([(Asset::Planning, 1), (Asset::Belief(report), 1)]),
                    )
                    .preserve(Role::Nature, basket([(Asset::Truth(encoded_truth), 1)]))
                    .consume(Role::Nature, basket([(Asset::Seed(encoded_seed), 1)]))
                    .produce(Role::Nature, basket([(Asset::Seed(next_seed), 1)]))
                    .distinct(Role::Actor, Role::Nature),
            );
        }
    }

    builder = builder.rate(
        RateId::BeginObserve,
        Rate::new()
            .preserve(Role::Actor, basket([(Asset::At(Location::Base), 1)]))
            .consume(
                Role::Actor,
                basket([(Asset::Planning, 1), (Asset::Sensor, 1)]),
            )
            .produce(
                Role::Actor,
                basket([(Asset::AwaitingObservation, 1), (Asset::UsedSensor, 1)]),
            ),
    );

    for &destination in destinations {
        builder = builder
            .rate(
                RateId::Move {
                    from: Location::Base,
                    to: destination,
                },
                Rate::new()
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
                        basket([
                            (Asset::Committed, 1),
                            (Asset::At(destination), 1),
                            (Asset::SpentEnergy, 1),
                        ]),
                    ),
            )
            .rate(
                RateId::Rescue {
                    location: destination,
                },
                Rate::new()
                    .consume(
                        Role::Actor,
                        basket([(Asset::Committed, 1), (Asset::Energy, 1)]),
                    )
                    .preserve(Role::Actor, basket([(Asset::At(destination), 1)]))
                    .produce(
                        Role::Actor,
                        basket([(Asset::Rescued, 1), (Asset::SpentEnergy, 1)]),
                    )
                    .preserve(Role::Nature, basket([(Asset::Truth(destination), 1)]))
                    .distinct(Role::Actor, Role::Nature),
            );
        if extended {
            builder = builder.rate(
                RateId::Evacuate {
                    location: destination,
                },
                Rate::new()
                    .consume(
                        Role::Actor,
                        basket([
                            (Asset::Rescued, 1),
                            (Asset::At(destination), 1),
                            (Asset::Energy, 1),
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
    }

    let completion = if extended {
        Asset::Evacuated
    } else {
        Asset::Rescued
    };

    builder
        .rate(
            RateId::Finish,
            Rate::new()
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

/// The exchange of one unit of `rate`, each of its roles bound to the
/// account that plays it.
pub fn action(rate: RateId) -> Action {
    match rate {
        RateId::Instantiate { .. } => {
            Exchange::new(rate, Quantity::new(1)).bind(Role::Nature, AccountId::Nature)
        }
        RateId::ResolveObservation { .. } | RateId::Rescue { .. } => {
            Exchange::new(rate, Quantity::new(1))
                .bind(Role::Actor, AccountId::Agent)
                .bind(Role::Nature, AccountId::Nature)
        }
        RateId::Finish => Exchange::new(rate, Quantity::new(1)).bind(Role::Actor, AccountId::Agent),
        RateId::BeginObserve | RateId::Move { .. } | RateId::Evacuate { .. } => {
            Exchange::new(rate, Quantity::new(1)).bind(Role::Actor, AccountId::Agent)
        }
    }
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
            next_seed: 2,
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

    fn trace(rates: impl IntoIterator<Item = RateId>) -> Trace<Ontology> {
        rates.into_iter().map(action).collect()
    }

    #[test]
    fn nature_resolves_an_observation_only_after_its_public_intent_and_replays() {
        let world = scenario(Location::South, 1);
        assert!(!world.is_applicable(&action(SENSED_SOUTH[1])));

        let replay = world
            .replayed(&trace(SENSED_SOUTH))
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
            .apply(action(RateId::BeginObserve))
            .expect("public observation intent applies");
        let rebound = Exchange::new(
            RateId::ResolveObservation {
                truth: Location::South,
                seed: 1,
                report: Location::South,
                next_seed: 2,
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
        let intent = action(RateId::BeginObserve);
        assert!(north.is_applicable(&intent));
        assert!(south.is_applicable(&intent));
    }

    #[test]
    fn sampled_trace_replays_from_the_uncertain_model() {
        let model = uniform_uncertain();
        let sampled = trace(
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
                .filter_map(|rate| rate.as_literal())
                .all(|rate| !replay.is_applicable(&action(*rate)))
        );
    }

    #[test]
    fn stress_profile_encodes_more_hidden_scenarios() {
        let scenarios = |world: &World| {
            world
                .rate_ids()
                .filter_map(|rate| rate.as_literal())
                .filter(|rate| {
                    matches!(rate, RateId::Instantiate { .. })
                        && world.is_applicable(&action(**rate))
                })
                .count()
        };
        assert_eq!(scenarios(&uniform_uncertain()), 8);
        assert_eq!(scenarios(&uniform_uncertain_showcase()), 32);
        assert_eq!(scenarios(&uniform_uncertain_stress()), 72);
    }
}
