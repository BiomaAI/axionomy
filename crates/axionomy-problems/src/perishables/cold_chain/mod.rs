//! A cold chain with physical storage, thermal history and competing fuel uses.
use axionomy::path;
use axionomy::{Economy, Exchange, Goal, Param, Quantity, Relation, basket};

pub const BATCHES: [u8; 6] = [0, 1, 2, 3, 4, 5];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Place {
    Warehouse,
    Fridge,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    World,
    Batch(u8),
    Buyer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Active,
    Dormant,
    At(Place),
    Fresh,
    Preserved,
    Spoiled,
    Disposed,
    Delivered,
    Heat,
    Origin(u8),
    ColdSpace,
    ColdOccupancy,
    Energy,
    SpentEnergy,
    GridPotential,
    SpilledEnergy,
    Fuel,
    GeneratorFuel,
    TransportFuel,
    CoolingOn,
    CoolingOff,
    Labor,
    SpentLabor,
    FutureLabor,
    UnusedLabor,
    Freight,
    UsedFreight,
    FutureFreight,
    UnusedFreight,
    Money,
    Demand,
    FutureDemand,
    SatisfiedDemand,
    MissedDemand,
    Day,
    Decisions,
    Aging,
    PendingAge(u8),
    Solved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    World,
    Batch,
    Source,
    Destination,
    Buyer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Split {
        source: u8,
        destination: u8,
        at: Place,
    },
    Move {
        batch: u8,
        from: Place,
        to: Place,
    },
    Process(u8),
    Deliver {
        batch: u8,
        preserved: bool,
        at: Place,
    },
    Discard {
        batch: u8,
        at: Place,
    },
    Cool,
    Generate,
    EndDay,
    AgeEmpty(u8),
    AgeCold(u8),
    AgeWarm {
        batch: u8,
        at: Place,
        spoil: bool,
    },
    OpenDay(u8),
    Close,
}
pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `perishables/cold-chain` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "perishables/cold-chain";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn initial(stress: bool, seed: u64) -> World {
    let reference = if stress {
        axionomy::path!("perishables/stress")
    } else {
        axionomy::path!("perishables/showcase")
    };
    crate::fixtures::variant(reference, |instance| {
        let days = if stress { 8 } else { 6 };
        let start = 2 + (seed % 2) as u8;
        let outage = start..start + if stress { 3 } else { 2 };
        let day = Param::<u8>::new(path!("day"));
        let calendar = Relation::each_giving(
            &day,
            [path!("grid-supply")],
            (1..days).map(|d| (d, [if outage.contains(&d) { 0 } else { 3u64 }])),
        );
        let open = RateIdPattern::OpenDay(day.var()).into();
        instance
            .tables
            .get_mut(&open)
            .expect("declared grid calendar")
            .relations = vec![calendar];
    })
}

/// The exchange of `units` of `rate`, each of its roles bound to the
/// account `world`'s rate fixes it to.
///
/// # Panics
///
/// If `rate` names no instance of `world`'s rates.
pub fn action(world: &World, rate: RateId, units: u64) -> Action {
    let instance = world
        .instance(&rate)
        .unwrap_or_else(|_| panic!("the cold chain has no rate {rate:?}"));
    instance.fixed_accounts().fold(
        Exchange::new(rate, Quantity::new(units)),
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
    Goal::new().require(AccountId::World, basket([(Asset::Solved, 1)]))
}
pub fn value(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
}
pub fn total(world: &World, asset: Asset) -> u64 {
    world
        .accounts()
        .map(|(_, account)| account.balance(&asset).to_u64())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Ends the day, each batch aging by `ages`, and opens day `next`.
    fn end_day(world: &mut World, next: u8, ages: [RateId; 6]) {
        world.apply(action(world, RateId::EndDay, 1)).unwrap();
        assert!(
            !world.is_applicable(&action(world, RateId::OpenDay(next), 1)),
            "all cohort exposure must settle before the next day"
        );
        for age in ages {
            world
                .apply(action(world, age, 1))
                .unwrap_or_else(|error| panic!("{age:?}: {error}"));
        }
        world
            .apply(action(world, RateId::OpenDay(next), 1))
            .unwrap();
    }
    #[test]
    fn split_cohorts_inherit_heat_and_diverge_only_after_different_storage_exposure() {
        let mut world = initial(false, 17);
        world.apply(action(&world, RateId::Cool, 1)).unwrap();
        end_day(
            &mut world,
            1,
            [
                RateId::AgeWarm {
                    batch: 0,
                    at: Place::Warehouse,
                    spoil: false,
                },
                RateId::AgeCold(1),
                RateId::AgeEmpty(2),
                RateId::AgeEmpty(3),
                RateId::AgeEmpty(4),
                RateId::AgeEmpty(5),
            ],
        );
        assert_eq!(value(&world, AccountId::Batch(0), Asset::Heat), 2);
        let before = world.state_key();
        assert!(
            world
                .apply(action(
                    &world,
                    RateId::Move {
                        batch: 0,
                        from: Place::Warehouse,
                        to: Place::Fridge
                    },
                    1
                ))
                .is_err()
        );
        assert_eq!(before, world.state_key());
        world
            .apply(action(
                &world,
                RateId::Deliver {
                    batch: 1,
                    preserved: false,
                    at: Place::Fridge,
                },
                2,
            ))
            .unwrap();
        world
            .apply(action(
                &world,
                RateId::Split {
                    source: 0,
                    destination: 2,
                    at: Place::Warehouse,
                },
                2,
            ))
            .unwrap();
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Heat), 2);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Origin(0)), 1);
        world
            .apply(action(
                &world,
                RateId::Move {
                    batch: 2,
                    from: Place::Warehouse,
                    to: Place::Fridge,
                },
                1,
            ))
            .unwrap();
        world.apply(action(&world, RateId::Cool, 1)).unwrap();
        // The split-off child ages cold; its warm parent spoils.
        end_day(
            &mut world,
            2,
            [
                RateId::AgeWarm {
                    batch: 0,
                    at: Place::Warehouse,
                    spoil: true,
                },
                RateId::AgeCold(1),
                RateId::AgeCold(2),
                RateId::AgeEmpty(3),
                RateId::AgeEmpty(4),
                RateId::AgeEmpty(5),
            ],
        );
        assert_eq!(value(&world, AccountId::Batch(0), Asset::Spoiled), 4);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Fresh), 2);
        assert_eq!(value(&world, AccountId::Batch(2), Asset::Heat), 2);
        assert_eq!(total(&world, Asset::ColdOccupancy), 4);
    }
    #[test]
    fn backup_and_transport_draw_from_the_same_finite_fuel() {
        let mut world = initial(false, 17);
        world.apply(action(&world, RateId::Generate, 1)).unwrap();
        assert_eq!(value(&world, AccountId::World, Asset::Energy), 6);
        assert!(!world.is_applicable(&action(&world, RateId::Generate, 1)));
        world
            .apply(action(
                &world,
                RateId::Deliver {
                    batch: 0,
                    preserved: false,
                    at: Place::Warehouse,
                },
                2,
            ))
            .unwrap();
        assert_eq!(value(&world, AccountId::World, Asset::Fuel), 4);
        assert_eq!(value(&world, AccountId::World, Asset::Money), 6);
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Deliver {
                batch: 0,
                preserved: false,
                at: Place::Warehouse
            },
            1
        )));
        let bad = action(&world, RateId::Process(0), 1).bind(Role::Batch, AccountId::Batch(1));
        assert!(!world.is_applicable(&bad));
    }
}
