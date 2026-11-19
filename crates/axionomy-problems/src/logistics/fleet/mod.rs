//! Timed heterogeneous fleet with cargo, a shared road and finite services.
use axionomy::{Account, Economy, Exchange, Goal, Quantity, basket};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Van {
    Cold,
    Freight,
}
pub const VANS: [Van; 2] = [Van::Cold, Van::Freight];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Depot,
    Hub,
    North,
    South,
    East,
}
pub const LOCATIONS: [Location; 5] = [
    Location::Depot,
    Location::Hub,
    Location::North,
    Location::South,
    Location::East,
];
pub const ROADS: [(Location, Location); 6] = [
    (Location::Depot, Location::Hub),
    (Location::Hub, Location::North),
    (Location::Hub, Location::South),
    (Location::North, Location::East),
    (Location::South, Location::East),
    (Location::Depot, Location::North),
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Outcome {
    Clear,
    Delayed,
    Breakdown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    World,
    Station,
    Nature,
    Vehicle(Van),
    Order(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Active,
    Closed,
    Time,
    TimeRemaining,
    Resolved,
    DeliveredCount,
    OnTimeCount,
    ExpiredCount,
    Permit(Van),
    UsedPermit(Van),
    RoadSpace,
    OnMainRoad,
    DockSpace,
    DockUsed,
    At(Location),
    Travelling(Location, Location),
    AwaitingOutcome,
    Ready,
    DepartedAt,
    ReadyAt,
    Trip,
    Integrity,
    Damage,
    Wear,
    MaintainedWear,
    Fuel,
    TankSpace,
    SpentFuel,
    Space,
    ColdSpace,
    Cargo(u8),
    Money,
    Revenue,
    FuelPump,
    Refueling,
    RepairBay,
    Repairing,
    Part,
    UsedPart,
    TowTruck,
    Towing,
    Scheduled,
    Waiting,
    InTransit,
    Delivered,
    Expired,
    TicketWord(Van, Location, Location),
    TicketDivisor,
    TransferCount,
    StagedCount,
    DamageCount,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    World,
    Vehicle,
    Other,
    Order,
    Station,
    Nature,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Release(u8),
    Load {
        van: Van,
        order: u8,
        at: Location,
    },
    Stage {
        van: Van,
        order: u8,
    },
    Transfer {
        from: Van,
        to: Van,
        order: u8,
        at: Location,
    },
    Depart {
        van: Van,
        from: Location,
        to: Location,
    },
    Resolve {
        van: Van,
        from: Location,
        to: Location,
        outcome: Outcome,
    },
    Arrive {
        van: Van,
        from: Location,
        to: Location,
    },
    Deliver {
        van: Van,
        order: u8,
        late: bool,
    },
    Expire {
        order: u8,
        carrier: Option<Van>,
        at: Location,
    },
    Refuel(Van),
    EndRefuel(Van),
    Repair {
        van: Van,
        damaged: bool,
    },
    EndRepair {
        van: Van,
        damaged: bool,
    },
    Tow {
        van: Van,
        from: Location,
    },
    EndTow(Van),
    Tick,
    Close,
}
pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `logistics/fleet` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "logistics/fleet";
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
pub fn cold(id: u8) -> bool {
    id.is_multiple_of(3)
}
pub fn weight(id: u8) -> u64 {
    if id % 3 == 1 { 3 } else { 1 }
}
pub fn destination(id: u8) -> Location {
    match id {
        0 => Location::North,
        1 | 2 => Location::South,
        3 | 4 => Location::East,
        _ => Location::North,
    }
}
/// What an order pays when delivered within its window: a cold load 24,
/// heavy freight 28, other freight 16.
pub fn price(id: u8) -> u64 {
    if cold(id) {
        24
    } else if weight(id) == 3 {
        28
    } else {
        16
    }
}
pub fn main_road(from: Location, to: Location) -> bool {
    matches!(
        (from, to),
        (Location::Depot, Location::Hub) | (Location::Hub, Location::Depot)
    )
}
pub fn express(from: Location, to: Location) -> bool {
    matches!(
        (from, to),
        (Location::Depot, Location::North) | (Location::North, Location::Depot)
    )
}
fn random_ticket(seed: u64, van: Van, attempt: u8, from: Location, to: Location) -> u64 {
    axionomy_search::sampling::splitmix(
        seed,
        1 + van as u64 * 389 + u64::from(attempt) * 37 + from as u64 * 7 + to as u64,
    )
}
/// Every road, each way.
fn directed_roads() -> impl Iterator<Item = (Location, Location)> {
    ROADS.into_iter().flat_map(|(a, b)| [(a, b), (b, a)])
}
pub fn initial(stress: bool, seed: u64) -> World {
    let mut nature = axionomy::Basket::new();
    for van in VANS {
        for (from, to) in directed_roads() {
            // Pack 24 two-bit outcomes into one exact integer per directed
            // road. All seeds expose the same rate catalog; only Nature's
            // private words differ. An authoritative trip cursor selects the
            // bit pair through core arithmetic, without policy-side
            // randomness.
            let mut word = 0u64;
            for attempt in 0..24 {
                let ticket = random_ticket(seed, van, attempt, from, to) % 12;
                let outcome = if express(from, to) && ticket < 3 {
                    Outcome::Breakdown
                } else if ticket < if stress { 4 } else { 2 } {
                    Outcome::Delayed
                } else {
                    Outcome::Clear
                };
                word |= (outcome as u64) << (u32::from(attempt) * 2);
            }
            nature.insert(Asset::TicketWord(van, from, to), Quantity::new(word));
        }
    }
    let reference = if stress {
        axionomy::path!("logistics/stress")
    } else {
        axionomy::path!("logistics/showcase")
    };
    crate::fixtures::variant(reference, |instance| {
        *crate::fixtures::account(instance, AccountId::Nature) = Account::from(nature);
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
        .unwrap_or_else(|_| panic!("the fleet has no rate {rate:?}"));
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
/// The most steps a fleet season takes before it is cut short.
pub const HORIZON: usize = 700;

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
    /// Nature's opening: the season's first orders released.
    const RELEASES: [RateId; 3] = [RateId::Release(0), RateId::Release(1), RateId::Release(2)];
    #[test]
    fn cargo_compatibility_and_trunk_occupancy_are_core_constraints() {
        let mut world = initial(false, 17);
        apply_all(&mut world, &RELEASES);
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Load {
                van: Van::Freight,
                order: 0,
                at: Location::Depot
            }
        )));
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Load {
                van: Van::Cold,
                order: 1,
                at: Location::Depot
            }
        )));
        world
            .apply(action(
                &world,
                RateId::Load {
                    van: Van::Cold,
                    order: 0,
                    at: Location::Depot,
                },
            ))
            .unwrap();
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Deliver {
                van: Van::Cold,
                order: 0,
                late: false
            }
        )));
        world.apply(action(&world, RateId::Tick)).unwrap();
        world
            .apply(action(
                &world,
                RateId::Depart {
                    van: Van::Cold,
                    from: Location::Depot,
                    to: Location::Hub,
                },
            ))
            .unwrap();
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Depart {
                van: Van::Freight,
                from: Location::Depot,
                to: Location::Hub
            }
        )));
        assert!(!world.is_applicable(&action(
            &world,
            RateId::Arrive {
                van: Van::Cold,
                from: Location::Depot,
                to: Location::Hub
            }
        )));
        // Nature's ticket delays the trip: the van arrives only once its
        // time has passed.
        apply_all(
            &mut world,
            &[RateId::Resolve {
                van: Van::Cold,
                from: Location::Depot,
                to: Location::Hub,
                outcome: Outcome::Delayed,
            }],
        );
        let arrive = RateId::Arrive {
            van: Van::Cold,
            from: Location::Depot,
            to: Location::Hub,
        };
        assert!(!world.is_applicable(&action(&world, arrive)));
        apply_all(&mut world, &[RateId::Tick, RateId::Tick, RateId::Tick]);
        assert!(!world.is_applicable(&action(&world, arrive)));
        apply_all(&mut world, &[RateId::Tick, arrive]);
        assert_eq!(value(&world, AccountId::World, Asset::RoadSpace), 1);
        assert!(world.is_applicable(&action(
            &world,
            RateId::Depart {
                van: Van::Freight,
                from: Location::Depot,
                to: Location::Hub
            }
        )));
    }
    #[test]
    fn handoff_and_service_use_real_cargo_funds_and_capacity() {
        let mut world = initial(false, 3);
        apply_all(&mut world, &RELEASES);
        world
            .apply(action(
                &world,
                RateId::Load {
                    van: Van::Cold,
                    order: 2,
                    at: Location::Depot,
                },
            ))
            .unwrap();
        world.apply(action(&world, RateId::Tick)).unwrap();
        let transfer = action(
            &world,
            RateId::Transfer {
                from: Van::Cold,
                to: Van::Freight,
                order: 2,
                at: Location::Depot,
            },
        );
        world.apply(transfer.clone()).unwrap();
        assert!(!world.is_applicable(&transfer));
        assert_eq!(
            value(&world, AccountId::Vehicle(Van::Cold), Asset::Space),
            2
        );
        assert_eq!(
            value(&world, AccountId::Vehicle(Van::Freight), Asset::Space),
            2
        );
        world.apply(action(&world, RateId::Tick)).unwrap();
        world
            .apply(action(&world, RateId::Refuel(Van::Cold)))
            .unwrap();
        assert!(!world.is_applicable(&action(&world, RateId::Refuel(Van::Freight))));
        assert!(!world.is_applicable(&action(&world, RateId::EndRefuel(Van::Cold))));
        assert_eq!(value(&world, AccountId::World, Asset::Money), 17);
        assert_eq!(value(&world, AccountId::Station, Asset::Fuel), 12);
    }
    #[test]
    fn rates_do_not_disclose_unrealized_weather_tickets() {
        let mut a = initial(false, 0);
        let mut b = initial(false, 17);
        apply_all(&mut a, &RELEASES);
        apply_all(&mut b, &RELEASES);
        assert_ne!(a.state_key(), b.state_key());
        let catalog = |world: &World| {
            let mut rates: Vec<_> = world
                .rate_ids()
                .flat_map(|key| world.instances_of(key))
                .collect();
            rates.sort();
            rates
        };
        assert_eq!(
            catalog(&a),
            catalog(&b),
            "rate availability must not disclose future weather"
        );
    }
}
