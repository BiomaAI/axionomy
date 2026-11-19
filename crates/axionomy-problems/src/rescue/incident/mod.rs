//! Repeated search, clinical decline and capacity-one evacuation.
use axionomy::{Account, Economy, Exchange, Goal, Quantity, basket};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Base,
    North,
    South,
    East,
    Harbor,
    West,
}
pub const LOCATIONS: [Location; 6] = [
    Location::Base,
    Location::North,
    Location::South,
    Location::East,
    Location::Harbor,
    Location::West,
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    World,
    Responder,
    Nature,
    Patient(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Active,
    Closed,
    Time,
    TimeRemaining,
    Resolved,
    SavedCount,
    DeathCount,
    LeftCount,
    SavedVitality,
    At(Location),
    Surveyed(Location),
    Unsearched(Location),
    Known(u8, Location),
    Truth(u8, Location),
    Ready,
    AwaitingScan,
    Sensor,
    UsedSensor,
    ScanRound(u8),
    Signal(u8, Location),
    Hint(Location),
    Fuel,
    SpentFuel,
    Energy,
    SpentEnergy,
    MedicalKit,
    UsedMedicalKit,
    SupplyAvailable,
    SupplyUsed,
    Seat,
    Carrying(u8),
    Hidden,
    Found,
    Onboard,
    Alive,
    Saved,
    Dead,
    LeftBehind,
    Vitality,
    Harm,
    NextDecline,
    Deadline,
    Treatable,
    Stabilized,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    World,
    Responder,
    Nature,
    Patient,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Phase {
    Hidden,
    Found,
    Onboard,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Loss {
    Deterioration,
    Deadline,
    Horizon,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Drive {
        from: Location,
        to: Location,
        fast: bool,
    },
    Search(Location),
    Scan,
    Report {
        round: u8,
        location: Location,
    },
    Discover {
        patient: u8,
        location: Location,
    },
    Deteriorate(u8),
    Stabilize {
        patient: u8,
        location: Location,
        restore: bool,
    },
    Load {
        patient: u8,
        location: Location,
    },
    Unload(u8),
    Lose {
        patient: u8,
        phase: Phase,
        cause: Loss,
    },
    Resupply,
    Wait,
    Close,
}
pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `rescue/incident` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "rescue/incident";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn value(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
}
pub fn now(world: &World) -> u64 {
    value(world, AccountId::World, Asset::Time)
}
/// The incident's roads, each joining two places both ways.
const ROADS: [(Location, Location); 7] = [
    (Location::Base, Location::North),
    (Location::Base, Location::South),
    (Location::Base, Location::Harbor),
    (Location::North, Location::East),
    (Location::East, Location::Harbor),
    (Location::South, Location::West),
    (Location::West, Location::Harbor),
];
/// How many roads the shortest way from `from` to `to` takes.
pub fn distance(from: Location, to: Location) -> u64 {
    if from == to {
        return 0;
    }
    let mut pending = std::collections::VecDeque::from([(from, 0)]);
    let mut seen = std::collections::BTreeSet::from([from]);
    while let Some((at, distance)) = pending.pop_front() {
        for (a, b) in ROADS {
            let next = if a == at {
                b
            } else if b == at {
                a
            } else {
                continue;
            };
            if next == to {
                return distance + 1;
            }
            if seen.insert(next) {
                pending.push_back((next, distance + 1));
            }
        }
    }
    unreachable!("connected emergency road network")
}
pub fn initial(stress: bool, seed: u64) -> World {
    let count = if stress { 4 } else { 3 };
    let sites = if stress {
        &LOCATIONS[1..]
    } else {
        &LOCATIONS[1..5]
    };
    let offset = (seed % sites.len() as u64) as usize;
    let stride = if sites.len() == 4 {
        if seed & 4 == 0 { 1 } else { 3 }
    } else {
        1 + ((seed / 5) % 4) as usize
    };
    let locations: Vec<_> = (0..count)
        .map(|id| sites[(offset + id * stride) % sites.len()])
        .collect();
    let mut nature = axionomy::Basket::new();
    for (id, location) in locations.iter().enumerate() {
        nature.insert(Asset::Truth(id as u8, *location), Quantity::new(1));
    }
    for (round, location) in locations.iter().enumerate().take(2) {
        let report = if (seed % 4 + round as u64).is_multiple_of(4) {
            sites[(offset + count) % sites.len()]
        } else {
            *location
        };
        nature.insert(Asset::Signal(round as u8, report), Quantity::new(1));
    }
    let reference = if stress {
        axionomy::path!("rescue/stress")
    } else {
        axionomy::path!("rescue/showcase")
    };
    crate::fixtures::variant(reference, |instance| {
        *crate::fixtures::account(instance, AccountId::Nature) = Account::from(nature);
        for id in 0..count as u8 {
            let first_decline = [4, 6, 8, 5][id as usize] + (seed.rotate_right(id as u32 * 3) % 3);
            crate::fixtures::set(
                instance,
                AccountId::Patient(id),
                [
                    (Asset::NextDecline, first_decline),
                    (Asset::Deadline, first_decline + 10),
                ],
            );
        }
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
        .unwrap_or_else(|_| panic!("the incident has no rate {rate:?}"));
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
/// The most steps an incident takes before it is cut short.
pub const HORIZON: usize = 250;

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
    fn hidden_scenarios_differ_only_in_what_the_responder_cannot_see() {
        let left = initial(false, 0);
        let right = initial(false, 17);
        let seen = [AccountId::Responder, AccountId::World];
        assert_ne!(left.state_key(), right.state_key());
        assert_eq!(
            left.view(seen).observation_key(),
            right.view(seen).observation_key()
        );
    }

    #[test]
    fn clinical_updates_and_treatment_are_authoritative_and_finite() {
        let mut world = initial(false, 0);
        apply_all(
            &mut world,
            &[
                RateId::Drive {
                    from: Location::Base,
                    to: Location::North,
                    fast: true,
                },
                RateId::Search(Location::North),
                RateId::Discover {
                    patient: 0,
                    location: Location::North,
                },
            ],
        );
        assert_eq!(
            value(&world, AccountId::World, Asset::Known(0, Location::North)),
            1
        );
        apply_all(&mut world, &[RateId::Wait, RateId::Wait]);
        assert_eq!(now(&world), 4);
        let load = action(
            &world,
            RateId::Load {
                patient: 0,
                location: Location::North,
            },
        );
        assert!(!world.is_applicable(&load));
        apply_all(&mut world, &[RateId::Deteriorate(0)]);
        assert_eq!(value(&world, AccountId::Patient(0), Asset::Vitality), 2);
        let treatment = action(
            &world,
            RateId::Stabilize {
                patient: 0,
                location: Location::North,
                restore: true,
            },
        );
        world.apply(treatment.clone()).unwrap();
        assert_eq!(value(&world, AccountId::Patient(0), Asset::Vitality), 3);
        assert_eq!(value(&world, AccountId::Patient(0), Asset::NextDecline), 11);
        assert!(!world.is_applicable(&treatment));
        world.apply(load.clone()).unwrap();
        assert!(!world.is_applicable(&load));
        assert_eq!(value(&world, AccountId::Responder, Asset::Seat), 0);
        // The clock runs on to 21, each tick followed by what Nature then
        // owes: every patient's decline, and the deaths it brings.
        let lose = |patient, phase| RateId::Lose {
            patient,
            phase,
            cause: Loss::Deterioration,
        };
        apply_all(
            &mut world,
            &[
                RateId::Wait,
                RateId::Deteriorate(1),
                RateId::Wait,
                RateId::Deteriorate(2),
                RateId::Wait,
                RateId::Deteriorate(1),
                RateId::Wait,
                RateId::Wait,
                RateId::Deteriorate(0),
                RateId::Deteriorate(2),
                RateId::Wait,
                RateId::Deteriorate(1),
                lose(1, Phase::Hidden),
                RateId::Wait,
                RateId::Wait,
                RateId::Deteriorate(0),
                RateId::Deteriorate(2),
                lose(2, Phase::Hidden),
                RateId::Wait,
                RateId::Wait,
                RateId::Wait,
                RateId::Deteriorate(0),
                lose(0, Phase::Onboard),
                RateId::Wait,
                RateId::Wait,
                RateId::Wait,
                RateId::Wait,
            ],
        );
        assert_eq!(now(&world), 21);
        assert_eq!(value(&world, AccountId::Patient(0), Asset::Dead), 1);
        assert_eq!(value(&world, AccountId::Responder, Asset::Seat), 1);
        assert!(!world.is_applicable(&action(&world, RateId::Unload(0))));
    }
}
