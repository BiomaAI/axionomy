//! Timed heterogeneous fleet with cargo, a shared road and finite services.
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity,
    QuantityComparison as C, QuantityExpression as Q, Rate, Rounding, Trace, basket,
};
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Van {
    Cold,
    Freight,
}
pub const VANS: [Van; 2] = [Van::Cold, Van::Freight];
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Outcome {
    Clear,
    Delayed,
    Breakdown,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum AccountId {
    World,
    Station,
    Nature,
    Vehicle(Van),
    Order(u8),
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Asset {
    WorldIdentity,
    StationIdentity,
    NatureIdentity,
    VehicleIdentity(Van),
    OrderIdentity(u8),
    Active,
    Closed,
    Time,
    TimeRemaining,
    Horizon,
    Count,
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
    Arrival,
    Deadline,
    Destination(Location),
    Weight,
    Price,
    ColdCargo,
    TicketWord(Van, Location, Location),
    TicketDivisor,
    TransferCount,
    StagedCount,
    DamageCount,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Role {
    World,
    Vehicle,
    Other,
    Order,
    Station,
    Nature,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Fastest,
    Reliable,
    Relay,
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
pub type Proposal = Trace<Ontology>;
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
fn q(role: Role, asset: Asset) -> Q<Ontology> {
    Q::balance(role, asset)
}
fn clock() -> Q<Ontology> {
    q(Role::World, Asset::Time)
}
fn identity() -> Rate<Ontology> {
    Rate::new()
        .preserve(Role::World, basket([(Asset::WorldIdentity, 1)]))
        .condition("one fleet event", Q::units(), C::Equal, Q::constant(1))
}
fn law() -> Rate<Ontology> {
    identity().preserve(Role::World, basket([(Asset::Active, 1)]))
}
fn vehicle(rate: Rate<Ontology>, van: Van) -> Rate<Ontology> {
    rate.preserve(Role::Vehicle, basket([(Asset::VehicleIdentity(van), 1)]))
        .distinct(Role::World, Role::Vehicle)
}
fn order(rate: Rate<Ontology>, id: u8) -> Rate<Ontology> {
    rate.preserve(Role::Order, basket([(Asset::OrderIdentity(id), 1)]))
        .distinct(Role::Order, Role::World)
}
fn station(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.preserve(Role::Station, basket([(Asset::StationIdentity, 1)]))
        .distinct(Role::Station, Role::World)
        .distinct(Role::Station, Role::Vehicle)
}
fn permit(rate: Rate<Ontology>, van: Van) -> Rate<Ontology> {
    rate.consume(Role::World, basket([(Asset::Permit(van), 1)]))
        .produce(Role::World, basket([(Asset::UsedPermit(van), 1)]))
}
fn ready(van: Van) -> Rate<Ontology> {
    permit(vehicle(law(), van), van).preserve(Role::Vehicle, basket([(Asset::Ready, 1)]))
}
fn deadline(rate: Rate<Ontology>, duration: u64) -> Rate<Ontology> {
    rate.condition(
        "activity fits remaining horizon",
        clock() + Q::constant(duration),
        C::LessThanOrEqual,
        q(Role::World, Asset::Horizon),
    )
}
fn finish(van: Van, state: Asset) -> Rate<Ontology> {
    vehicle(law(), van)
        .condition(
            "activity duration elapsed",
            clock(),
            C::GreaterThanOrEqual,
            q(Role::Vehicle, Asset::ReadyAt),
        )
        .consume(Role::Vehicle, basket([(state, 1)]))
        .produce(Role::Vehicle, basket([(Asset::Ready, 1)]))
        .consume_computed(
            Role::Vehicle,
            Asset::ReadyAt,
            q(Role::Vehicle, Asset::ReadyAt),
        )
}
fn release_capacity(rate: Rate<Ontology>, id: u8, role: Role) -> Rate<Ontology> {
    let rate = rate.produce(role, basket([(Asset::Space, weight(id))]));
    if cold(id) {
        rate.produce(role, basket([(Asset::ColdSpace, 1)]))
    } else {
        rate
    }
}
fn take_capacity(rate: Rate<Ontology>, id: u8, role: Role) -> Rate<Ontology> {
    let rate = rate.consume(role, basket([(Asset::Space, weight(id))]));
    if cold(id) {
        rate.consume(role, basket([(Asset::ColdSpace, 1)]))
    } else {
        rate
    }
}
fn random_ticket(seed: u64, van: Van, attempt: u8, from: Location, to: Location) -> u64 {
    let mut n = seed.wrapping_add(0x9e3779b97f4a7c15u64.wrapping_mul(
        1 + van as u64 * 389 + u64::from(attempt) * 37 + from as u64 * 7 + to as u64,
    ));
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
pub fn initial(stress: bool, seed: u64) -> World {
    let count: u8 = if stress { 6 } else { 4 };
    let horizon = if stress { 52 } else { 40 };
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::World,
            Account::from(basket([
                (Asset::WorldIdentity, 1),
                (Asset::Active, 1),
                (Asset::Horizon, horizon),
                (Asset::TimeRemaining, horizon),
                (Asset::Count, count.into()),
                (Asset::Money, 20),
                (Asset::RoadSpace, 1),
                (Asset::DockSpace, 2),
                (Asset::Permit(Van::Cold), 1),
                (Asset::Permit(Van::Freight), 1),
            ])),
        )
        .account(
            AccountId::Station,
            Account::from(basket([
                (Asset::StationIdentity, 1),
                (Asset::Fuel, if stress { 18 } else { 15 }),
                (Asset::FuelPump, 1),
                (Asset::RepairBay, 1),
                (Asset::Part, if stress { 3 } else { 2 }),
                (Asset::TowTruck, 1),
            ])),
        );
    let mut nature = basket([(Asset::NatureIdentity, 1)]);
    for van in VANS {
        let capacity = if van == Van::Cold { 2 } else { 3 };
        let mut stock = basket([
            (Asset::VehicleIdentity(van), 1),
            (Asset::At(Location::Depot), 1),
            (Asset::Ready, 1),
            (Asset::Integrity, 1),
            (Asset::TicketDivisor, 1),
            (Asset::Fuel, 5),
            (Asset::TankSpace, 3),
            (Asset::Space, capacity),
        ]);
        if van == Van::Cold {
            stock.insert(Asset::ColdSpace, Quantity::new(1));
        }
        builder = builder.account(AccountId::Vehicle(van), Account::from(stock));
        for (a, b) in ROADS {
            for (from, to) in [(a, b), (b, a)] {
                let fuel = if express(from, to) { 2 } else { 1 };
                let mut depart = deadline(permit(vehicle(law(), van), van), 6)
                    .preserve(Role::Vehicle, basket([(Asset::Integrity, 1)]))
                    .condition(
                        "service before excessive wear",
                        q(Role::Vehicle, Asset::Wear),
                        C::LessThanOrEqual,
                        Q::constant(5),
                    )
                    .condition(
                        "finite travel tickets",
                        q(Role::Vehicle, Asset::Trip),
                        C::LessThanOrEqual,
                        Q::constant(23),
                    )
                    .consume(
                        Role::Vehicle,
                        basket([(Asset::Ready, 1), (Asset::At(from), 1), (Asset::Fuel, fuel)]),
                    )
                    .produce(
                        Role::Vehicle,
                        basket([
                            (Asset::Travelling(from, to), 1),
                            (Asset::AwaitingOutcome, 1),
                            (Asset::SpentFuel, fuel),
                            (Asset::TankSpace, fuel),
                        ]),
                    )
                    .produce_computed(Role::Vehicle, Asset::DepartedAt, clock());
                if main_road(from, to) {
                    depart = depart
                        .consume(Role::World, basket([(Asset::RoadSpace, 1)]))
                        .produce(Role::Vehicle, basket([(Asset::OnMainRoad, 1)]));
                }
                builder = builder.rate(RateId::Depart { van, from, to }, depart);
                // Pack 24 two-bit outcomes into one exact integer per directed
                // road. All seeds expose the same rate catalog; only Nature's
                // private words differ. An authoritative trip cursor selects the bit
                // pair through core arithmetic, without policy-side randomness.
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
                for outcome in [Outcome::Clear, Outcome::Delayed, Outcome::Breakdown] {
                    let duration = match outcome {
                        Outcome::Clear => 2,
                        Outcome::Delayed => 4,
                        Outcome::Breakdown => 6,
                    };
                    // Bit extraction from the hidden ticket: floor is the
                    // operation itself, not a settlement choice.
                    let shifted = Q::divide(
                        q(Role::Nature, Asset::TicketWord(van, from, to)),
                        q(Role::Vehicle, Asset::TicketDivisor),
                        Rounding::Floor,
                    );
                    let code = shifted.clone()
                        - (Q::divide(shifted, Q::constant(4), Rounding::Floor) * Q::constant(4));
                    let mut resolution = vehicle(law(), van)
                        .preserve(Role::Nature, basket([(Asset::NatureIdentity, 1)]))
                        .preserve(Role::Vehicle, basket([(Asset::Travelling(from, to), 1)]))
                        .condition(
                            "match hidden vehicle-trip route ticket",
                            code,
                            C::Equal,
                            Q::constant(outcome as u64),
                        )
                        .consume(Role::Vehicle, basket([(Asset::AwaitingOutcome, 1)]))
                        .produce(Role::Vehicle, basket([(Asset::Trip, 1), (Asset::Wear, 1)]))
                        .produce_computed(
                            Role::Vehicle,
                            Asset::TicketDivisor,
                            q(Role::Vehicle, Asset::TicketDivisor) * Q::constant(3),
                        )
                        .produce_computed(
                            Role::Vehicle,
                            Asset::ReadyAt,
                            q(Role::Vehicle, Asset::DepartedAt) + Q::constant(duration),
                        )
                        .distinct(Role::Nature, Role::World)
                        .distinct(Role::Nature, Role::Vehicle);
                    if outcome == Outcome::Breakdown {
                        resolution = resolution
                            .consume(Role::Vehicle, basket([(Asset::Integrity, 1)]))
                            .produce(Role::Vehicle, basket([(Asset::Damage, 1)]))
                            .produce(Role::World, basket([(Asset::DamageCount, 1)]));
                    }
                    builder = builder.rate(
                        RateId::Resolve {
                            van,
                            from,
                            to,
                            outcome,
                        },
                        resolution,
                    );
                }
                let mut arrive = finish(van, Asset::Travelling(from, to))
                    .condition(
                        "Nature has resolved the trip",
                        q(Role::Vehicle, Asset::AwaitingOutcome),
                        C::Equal,
                        Q::constant(0),
                    )
                    .produce(Role::Vehicle, basket([(Asset::At(to), 1)]))
                    .consume_computed(
                        Role::Vehicle,
                        Asset::DepartedAt,
                        q(Role::Vehicle, Asset::DepartedAt),
                    );
                if main_road(from, to) {
                    arrive = arrive
                        .consume(Role::Vehicle, basket([(Asset::OnMainRoad, 1)]))
                        .produce(Role::World, basket([(Asset::RoadSpace, 1)]));
                }
                builder = builder.rate(RateId::Arrive { van, from, to }, arrive);
            }
        }
        builder = builder.rate(
            RateId::Refuel(van),
            station(deadline(permit(vehicle(law(), van), van), 2))
                .preserve(Role::Vehicle, basket([(Asset::At(Location::Depot), 1)]))
                .consume(
                    Role::Vehicle,
                    basket([(Asset::Ready, 1), (Asset::TankSpace, 3)]),
                )
                .produce(
                    Role::Vehicle,
                    basket([(Asset::Refueling, 1), (Asset::Fuel, 3)]),
                )
                .produce_computed(Role::Vehicle, Asset::ReadyAt, clock() + Q::constant(2))
                .consume(
                    Role::Station,
                    basket([(Asset::FuelPump, 1), (Asset::Fuel, 3)]),
                )
                .produce(Role::Station, basket([(Asset::Money, 3)]))
                .consume(Role::World, basket([(Asset::Money, 3)])),
        );
        builder = builder.rate(
            RateId::EndRefuel(van),
            station(finish(van, Asset::Refueling))
                .produce(Role::Station, basket([(Asset::FuelPump, 1)])),
        );
        for damaged in [false, true] {
            let mut repair = station(deadline(permit(vehicle(law(), van), van), 3))
                .preserve(
                    Role::Vehicle,
                    basket([
                        (Asset::At(Location::Depot), 1),
                        (
                            if damaged {
                                Asset::Damage
                            } else {
                                Asset::Integrity
                            },
                            1,
                        ),
                    ]),
                )
                .consume(Role::Vehicle, basket([(Asset::Ready, 1)]))
                .produce(Role::Vehicle, basket([(Asset::Repairing, 1)]))
                .produce_computed(Role::Vehicle, Asset::ReadyAt, clock() + Q::constant(3))
                .consume(
                    Role::Station,
                    basket([(Asset::RepairBay, 1), (Asset::Part, 1)]),
                )
                .produce(
                    Role::Station,
                    basket([(Asset::Money, 2), (Asset::UsedPart, 1)]),
                )
                .consume(Role::World, basket([(Asset::Money, 2)]));
            if !damaged {
                repair = repair.condition(
                    "preventive service after accumulated wear",
                    q(Role::Vehicle, Asset::Wear),
                    C::GreaterThanOrEqual,
                    Q::constant(3),
                );
            }
            builder = builder.rate(RateId::Repair { van, damaged }, repair);
            let mut end = station(finish(van, Asset::Repairing))
                .produce(Role::Station, basket([(Asset::RepairBay, 1)]))
                .consume_computed(Role::Vehicle, Asset::Wear, q(Role::Vehicle, Asset::Wear))
                .produce_computed(
                    Role::Vehicle,
                    Asset::MaintainedWear,
                    q(Role::Vehicle, Asset::Wear),
                );
            if damaged {
                end = end
                    .consume(Role::Vehicle, basket([(Asset::Damage, 1)]))
                    .produce(Role::Vehicle, basket([(Asset::Integrity, 1)]));
            } else {
                end = end.preserve(Role::Vehicle, basket([(Asset::Integrity, 1)]));
            }
            builder = builder.rate(RateId::EndRepair { van, damaged }, end);
        }
        for from in LOCATIONS
            .into_iter()
            .filter(|location| *location != Location::Depot)
        {
            builder = builder.rate(
                RateId::Tow { van, from },
                station(deadline(permit(vehicle(law(), van), van), 3))
                    .consume(
                        Role::Vehicle,
                        basket([(Asset::Ready, 1), (Asset::At(from), 1)]),
                    )
                    .produce(Role::Vehicle, basket([(Asset::Towing, 1)]))
                    .produce_computed(Role::Vehicle, Asset::ReadyAt, clock() + Q::constant(3))
                    .consume(Role::Station, basket([(Asset::TowTruck, 1)]))
                    .produce(Role::Station, basket([(Asset::Money, 4)]))
                    .consume(Role::World, basket([(Asset::Money, 4)])),
            );
        }
        builder = builder.rate(
            RateId::EndTow(van),
            station(finish(van, Asset::Towing))
                .produce(Role::Station, basket([(Asset::TowTruck, 1)]))
                .produce(Role::Vehicle, basket([(Asset::At(Location::Depot), 1)])),
        );
    }
    builder = builder.account(AccountId::Nature, Account::from(nature));
    for id in 0..count {
        let arrival = [0, 0, 0, 7, 10, 13][id as usize];
        let due = [12, 18, 17, 30, 40, 44][id as usize];
        let price = if cold(id) {
            24
        } else if weight(id) == 3 {
            28
        } else {
            16
        };
        let mut stock = basket([
            (Asset::OrderIdentity(id), 1),
            (Asset::Scheduled, 1),
            (Asset::At(Location::Depot), 1),
            (Asset::Arrival, arrival),
            (Asset::Deadline, due),
            (Asset::Destination(destination(id)), 1),
            (Asset::Weight, weight(id)),
            (Asset::Price, price),
            (Asset::Money, price),
        ]);
        if cold(id) {
            stock.insert(Asset::ColdCargo, Quantity::new(1));
        }
        builder = builder.account(AccountId::Order(id), Account::from(stock));
        builder = builder.rate(
            RateId::Release(id),
            order(law(), id)
                .condition(
                    "customer order has arrived",
                    clock(),
                    C::GreaterThanOrEqual,
                    q(Role::Order, Asset::Arrival),
                )
                .consume(Role::Order, basket([(Asset::Scheduled, 1)]))
                .produce(Role::Order, basket([(Asset::Waiting, 1)])),
        );
        for van in VANS {
            for at in [Location::Depot, Location::Hub] {
                let mut load = take_capacity(order(ready(van), id), id, Role::Vehicle)
                    .preserve(Role::Vehicle, basket([(Asset::At(at), 1)]))
                    .consume(
                        Role::Order,
                        basket([(Asset::Waiting, 1), (Asset::At(at), 1)]),
                    )
                    .produce(Role::Order, basket([(Asset::InTransit, 1)]))
                    .produce(Role::Vehicle, basket([(Asset::Cargo(id), 1)]))
                    .distinct(Role::Vehicle, Role::Order);
                if at == Location::Hub {
                    load = load
                        .consume(Role::Order, basket([(Asset::DockUsed, weight(id))]))
                        .produce(Role::World, basket([(Asset::DockSpace, weight(id))]));
                }
                builder = builder.rate(RateId::Load { van, order: id, at }, load);
            }
            builder = builder.rate(
                RateId::Stage { van, order: id },
                release_capacity(order(ready(van), id), id, Role::Vehicle)
                    .preserve(Role::Vehicle, basket([(Asset::At(Location::Hub), 1)]))
                    .consume(Role::Vehicle, basket([(Asset::Cargo(id), 1)]))
                    .consume(Role::Order, basket([(Asset::InTransit, 1)]))
                    .produce(
                        Role::Order,
                        basket([
                            (Asset::Waiting, 1),
                            (Asset::At(Location::Hub), 1),
                            (Asset::DockUsed, weight(id)),
                        ]),
                    )
                    .consume(Role::World, basket([(Asset::DockSpace, weight(id))]))
                    .produce(Role::World, basket([(Asset::StagedCount, 1)]))
                    .distinct(Role::Order, Role::Vehicle),
            );
            for late in [false, true] {
                if late && cold(id) {
                    continue;
                }
                let payment = if late { price / 2 } else { price };
                let mut deliver = release_capacity(order(ready(van), id), id, Role::Vehicle)
                    .preserve(Role::Vehicle, basket([(Asset::At(destination(id)), 1)]))
                    .consume(Role::Vehicle, basket([(Asset::Cargo(id), 1)]))
                    .consume(
                        Role::Order,
                        basket([(Asset::InTransit, 1), (Asset::Money, payment)]),
                    )
                    .produce(Role::Order, basket([(Asset::Delivered, 1)]))
                    .produce(
                        Role::World,
                        basket([
                            (Asset::Resolved, 1),
                            (Asset::DeliveredCount, 1),
                            (Asset::Money, payment),
                            (Asset::Revenue, payment),
                        ]),
                    )
                    .distinct(Role::Vehicle, Role::Order);
                if late {
                    deliver = deliver.condition(
                        "late delivery earns half payment",
                        clock(),
                        C::GreaterThanOrEqual,
                        q(Role::Order, Asset::Deadline) + Q::constant(1),
                    );
                } else {
                    deliver = deliver
                        .condition(
                            "within customer window",
                            clock(),
                            C::LessThanOrEqual,
                            q(Role::Order, Asset::Deadline),
                        )
                        .produce(Role::World, basket([(Asset::OnTimeCount, 1)]));
                }
                builder = builder.rate(
                    RateId::Deliver {
                        van,
                        order: id,
                        late,
                    },
                    deliver,
                );
            }
            let other = if van == Van::Cold {
                Van::Freight
            } else {
                Van::Cold
            };
            for at in LOCATIONS {
                let transfer = permit(
                    take_capacity(
                        release_capacity(order(ready(van), id), id, Role::Vehicle),
                        id,
                        Role::Other,
                    ),
                    other,
                )
                .preserve(Role::Vehicle, basket([(Asset::At(at), 1)]))
                .preserve(
                    Role::Other,
                    basket([
                        (Asset::VehicleIdentity(other), 1),
                        (Asset::At(at), 1),
                        (Asset::Ready, 1),
                    ]),
                )
                .preserve(Role::Order, basket([(Asset::InTransit, 1)]))
                .consume(Role::Vehicle, basket([(Asset::Cargo(id), 1)]))
                .produce(Role::Other, basket([(Asset::Cargo(id), 1)]))
                .produce(Role::World, basket([(Asset::TransferCount, 1)]))
                .distinct(Role::Vehicle, Role::Other)
                .distinct(Role::Order, Role::Vehicle)
                .distinct(Role::Order, Role::Other)
                .distinct(Role::World, Role::Other);
                builder = builder.rate(
                    RateId::Transfer {
                        from: van,
                        to: other,
                        order: id,
                        at,
                    },
                    transfer,
                );
            }
        }
        for carrier in [None, Some(Van::Cold), Some(Van::Freight)] {
            for at in if carrier.is_none() {
                &[Location::Depot, Location::Hub][..]
            } else {
                &[Location::Depot][..]
            } {
                let mut expire = order(law(), id)
                    .produce(Role::Order, basket([(Asset::Expired, 1)]))
                    .produce(
                        Role::World,
                        basket([(Asset::Resolved, 1), (Asset::ExpiredCount, 1)]),
                    );
                expire = if cold(id) {
                    expire.condition(
                        "cold shipment has expired",
                        clock(),
                        C::GreaterThanOrEqual,
                        q(Role::Order, Asset::Deadline) + Q::constant(1),
                    )
                } else {
                    expire.condition(
                        "fleet horizon reached",
                        clock(),
                        C::GreaterThanOrEqual,
                        q(Role::World, Asset::Horizon),
                    )
                };
                if let Some(van) = carrier {
                    expire = release_capacity(vehicle(expire, van), id, Role::Vehicle)
                        .consume(Role::Order, basket([(Asset::InTransit, 1)]))
                        .consume(Role::Vehicle, basket([(Asset::Cargo(id), 1)]))
                        .distinct(Role::Order, Role::Vehicle);
                } else {
                    expire = expire.consume(
                        Role::Order,
                        basket([(Asset::Waiting, 1), (Asset::At(*at), 1)]),
                    );
                    if *at == Location::Hub {
                        expire = expire
                            .consume(Role::Order, basket([(Asset::DockUsed, weight(id))]))
                            .produce(Role::World, basket([(Asset::DockSpace, weight(id))]));
                    }
                }
                builder = builder.rate(
                    RateId::Expire {
                        order: id,
                        carrier,
                        at: *at,
                    },
                    expire,
                );
            }
        }
    }
    let mut tick = law()
        .consume(Role::World, basket([(Asset::TimeRemaining, 1)]))
        .produce(Role::World, basket([(Asset::Time, 1)]));
    for van in VANS {
        tick = tick
            .consume_computed(
                Role::World,
                Asset::UsedPermit(van),
                q(Role::World, Asset::UsedPermit(van)),
            )
            .produce_computed(
                Role::World,
                Asset::Permit(van),
                q(Role::World, Asset::UsedPermit(van)),
            );
    }
    builder = builder.rate(RateId::Tick, tick);
    builder = builder.rate(
        RateId::Close,
        identity()
            .condition(
                "all customer orders resolved",
                q(Role::World, Asset::Resolved),
                C::Equal,
                q(Role::World, Asset::Count),
            )
            .preserve(
                Role::Vehicle,
                basket([(Asset::VehicleIdentity(Van::Cold), 1), (Asset::Ready, 1)]),
            )
            .preserve(
                Role::Other,
                basket([(Asset::VehicleIdentity(Van::Freight), 1), (Asset::Ready, 1)]),
            )
            .distinct(Role::World, Role::Vehicle)
            .distinct(Role::World, Role::Other)
            .distinct(Role::Vehicle, Role::Other)
            .consume(Role::World, basket([(Asset::Active, 1)]))
            .produce(Role::World, basket([(Asset::Closed, 1)])),
    );
    let mut space = LinearInvariant::new("physical vehicle cargo capacity").weight(Asset::Space, 1);
    let mut cold_space =
        LinearInvariant::new("one refrigerated cargo slot").weight(Asset::ColdSpace, 1);
    let mut activity = LinearInvariant::new("two vehicle activities")
        .weight(Asset::Ready, 1)
        .weight(Asset::Refueling, 1)
        .weight(Asset::Repairing, 1)
        .weight(Asset::Towing, 1);
    for id in 0..count {
        space = space.weight(Asset::Cargo(id), weight(id) as i64);
        if cold(id) {
            cold_space = cold_space.weight(Asset::Cargo(id), 1);
        }
    }
    for (a, b) in ROADS {
        activity = activity
            .weight(Asset::Travelling(a, b), 1)
            .weight(Asset::Travelling(b, a), 1);
    }
    for van in VANS {
        builder = builder.invariant(
            LinearInvariant::new(format!("{van:?} dispatch permit"))
                .weight(Asset::Permit(van), 1)
                .weight(Asset::UsedPermit(van), 1),
        );
    }
    builder
        .invariant(space)
        .invariant(cold_space)
        .invariant(activity)
        .invariant(
            LinearInvariant::new("cargo lifecycle")
                .weight(Asset::Scheduled, 1)
                .weight(Asset::Waiting, 1)
                .weight(Asset::InTransit, 1)
                .weight(Asset::Delivered, 1)
                .weight(Asset::Expired, 1),
        )
        .invariant(
            LinearInvariant::new("one occupied trunk road")
                .weight(Asset::RoadSpace, 1)
                .weight(Asset::OnMainRoad, 1),
        )
        .invariant(
            LinearInvariant::new("finite hub staging capacity")
                .weight(Asset::DockSpace, 1)
                .weight(Asset::DockUsed, 1),
        )
        .invariant(
            LinearInvariant::new("fuel stock")
                .weight(Asset::Fuel, 1)
                .weight(Asset::SpentFuel, 1),
        )
        .invariant(
            LinearInvariant::new("funded delivery and service payments").weight(Asset::Money, 1),
        )
        .invariant(
            LinearInvariant::new("vehicle condition")
                .weight(Asset::Integrity, 1)
                .weight(Asset::Damage, 1),
        )
        .invariant(
            LinearInvariant::new("fuel pump occupancy")
                .weight(Asset::FuelPump, 1)
                .weight(Asset::Refueling, 1),
        )
        .invariant(
            LinearInvariant::new("repair bay occupancy")
                .weight(Asset::RepairBay, 1)
                .weight(Asset::Repairing, 1),
        )
        .invariant(
            LinearInvariant::new("tow truck occupancy")
                .weight(Asset::TowTruck, 1)
                .weight(Asset::Towing, 1),
        )
        .invariant(
            LinearInvariant::new("finite repair parts")
                .weight(Asset::Part, 1)
                .weight(Asset::UsedPart, 1),
        )
        .invariant(
            LinearInvariant::new("bounded fleet clock")
                .weight(Asset::Time, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .invariant(
            LinearInvariant::new("fleet lifecycle")
                .weight(Asset::Active, 1)
                .weight(Asset::Closed, 1),
        )
        .build()
        .expect("fleet mechanics conserve physical and financial resources")
}
pub fn action(rate: RateId) -> Action {
    let mut exchange = Exchange::new(rate, Quantity::new(1)).bind(Role::World, AccountId::World);
    match rate {
        RateId::Close => {
            exchange = exchange
                .bind(Role::Vehicle, AccountId::Vehicle(Van::Cold))
                .bind(Role::Other, AccountId::Vehicle(Van::Freight))
        }
        RateId::Release(id) => exchange = exchange.bind(Role::Order, AccountId::Order(id)),
        RateId::Load { van, order, .. }
        | RateId::Stage { van, order }
        | RateId::Deliver { van, order, .. } => {
            exchange = exchange
                .bind(Role::Vehicle, AccountId::Vehicle(van))
                .bind(Role::Order, AccountId::Order(order))
        }
        RateId::Transfer {
            from, to, order, ..
        } => {
            exchange = exchange
                .bind(Role::Vehicle, AccountId::Vehicle(from))
                .bind(Role::Other, AccountId::Vehicle(to))
                .bind(Role::Order, AccountId::Order(order))
        }
        RateId::Expire { order, carrier, .. } => {
            exchange = exchange.bind(Role::Order, AccountId::Order(order));
            if let Some(van) = carrier {
                exchange = exchange.bind(Role::Vehicle, AccountId::Vehicle(van));
            }
        }
        RateId::Depart { van, .. } | RateId::Arrive { van, .. } => {
            exchange = exchange.bind(Role::Vehicle, AccountId::Vehicle(van))
        }
        RateId::Resolve { van, .. } => {
            exchange = exchange
                .bind(Role::Vehicle, AccountId::Vehicle(van))
                .bind(Role::Nature, AccountId::Nature)
        }
        RateId::Refuel(van)
        | RateId::EndRefuel(van)
        | RateId::Repair { van, .. }
        | RateId::EndRepair { van, .. }
        | RateId::Tow { van, .. }
        | RateId::EndTow(van) => {
            exchange = exchange
                .bind(Role::Vehicle, AccountId::Vehicle(van))
                .bind(Role::Station, AccountId::Station)
        }
        _ => {}
    }
    exchange
}
pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::World, basket([(Asset::Closed, 1)]))
}
pub fn location(world: &World, van: Van) -> Option<Location> {
    LOCATIONS
        .into_iter()
        .find(|at| value(world, AccountId::Vehicle(van), Asset::At(*at)) > 0)
}
/// Static route planning uses public road costs; the realized ticket account is
/// deliberately absent. Dispatch also checks actual trunk-road occupancy.
pub fn route(from: Location, to: Location, reliable: bool) -> Option<(Location, u64)> {
    if from == to {
        return None;
    }
    let mut queue = std::collections::VecDeque::from([(from, None, 0)]);
    let mut seen = std::collections::BTreeSet::from([from]);
    while let Some((at, first, cost)) = queue.pop_front() {
        for (a, b) in ROADS {
            let next = if at == a {
                b
            } else if at == b {
                a
            } else {
                continue;
            };
            if reliable && express(at, next) {
                continue;
            }
            let first = first.unwrap_or(next);
            if next == to {
                return Some((first, cost + 1));
            }
            if seen.insert(next) {
                queue.push_back((next, Some(first), cost + 1));
            }
        }
    }
    None
}
pub fn response(world: &World) -> Option<Action> {
    let count = value(world, AccountId::World, Asset::Count) as u8;
    for id in 0..count {
        let release = action(RateId::Release(id));
        if world.is_applicable(&release) {
            return Some(release);
        }
        for carrier in [None, Some(Van::Cold), Some(Van::Freight)] {
            for at in [Location::Depot, Location::Hub] {
                let exchange = action(RateId::Expire {
                    order: id,
                    carrier,
                    at,
                });
                if world.is_applicable(&exchange) {
                    return Some(exchange);
                }
            }
        }
    }
    for van in VANS {
        let own = AccountId::Vehicle(van);
        for (a, b) in ROADS {
            for (from, to) in [(a, b), (b, a)] {
                if value(world, own, Asset::Travelling(from, to)) == 0 {
                    continue;
                }
                if value(world, own, Asset::AwaitingOutcome) > 0 {
                    for outcome in [Outcome::Clear, Outcome::Delayed, Outcome::Breakdown] {
                        let exchange = action(RateId::Resolve {
                            van,
                            from,
                            to,
                            outcome,
                        });
                        if world.is_applicable(&exchange) {
                            return Some(exchange);
                        }
                    }
                }
                let arrive = action(RateId::Arrive { van, from, to });
                if world.is_applicable(&arrive) {
                    return Some(arrive);
                }
            }
        }
        for rate in [
            RateId::EndRefuel(van),
            RateId::EndTow(van),
            RateId::EndRepair { van, damaged: true },
            RateId::EndRepair {
                van,
                damaged: false,
            },
        ] {
            let exchange = action(rate);
            if world.is_applicable(&exchange) {
                return Some(exchange);
            }
        }
    }
    None
}
pub fn decision(world: &World, van: Van, policy: Policy) -> Option<Action> {
    if value(world, AccountId::World, Asset::Resolved)
        == value(world, AccountId::World, Asset::Count)
    {
        return None;
    }
    let own = AccountId::Vehicle(van);
    let val = |asset| value(world, own, asset);
    if val(Asset::Ready) == 0 || value(world, AccountId::World, Asset::Permit(van)) == 0 {
        return None;
    }
    let from = location(world, van)?;
    let count = value(world, AccountId::World, Asset::Count) as u8;
    let propose = |rate| {
        let exchange = action(rate);
        world.is_applicable(&exchange).then_some(exchange)
    };
    let drive = |to| {
        route(from, to, policy != Policy::Fastest).and_then(|(next, _)| {
            propose(RateId::Depart {
                van,
                from,
                to: next,
            })
        })
    };
    let mut cargo: Vec<_> = (0..count).filter(|id| val(Asset::Cargo(*id)) > 0).collect();
    cargo.sort_by_key(|id| {
        (
            !cold(*id),
            value(world, AccountId::Order(*id), Asset::Deadline),
        )
    });
    for id in &cargo {
        for late in [false, true] {
            if let Some(deliver) = propose(RateId::Deliver {
                van,
                order: *id,
                late,
            }) {
                return Some(deliver);
            }
        }
    }
    if policy == Policy::Relay
        && from == Location::Hub
        && van == Van::Cold
        && cargo.iter().any(|id| cold(*id))
    {
        for id in cargo.iter().filter(|id| !cold(**id)) {
            if let Some(stage) = propose(RateId::Stage { van, order: *id }) {
                return Some(stage);
            }
        }
    }
    // A working teammate can recover a compatible load from a stranded van.
    if val(Asset::Damage) > 0 || val(Asset::Fuel) == 0 {
        let other = if van == Van::Cold {
            Van::Freight
        } else {
            Van::Cold
        };
        for id in &cargo {
            if let Some(transfer) = propose(RateId::Transfer {
                from: van,
                to: other,
                order: *id,
                at: from,
            }) {
                return Some(transfer);
            }
        }
        if from != Location::Depot {
            return propose(RateId::Tow { van, from });
        }
    }
    if from == Location::Depot {
        if (val(Asset::Damage) > 0 || val(Asset::Wear) >= 4)
            && let Some(repair) = propose(RateId::Repair {
                van,
                damaged: val(Asset::Damage) > 0,
            })
        {
            return Some(repair);
        }
        if val(Asset::Fuel) < 4
            && let Some(refuel) = propose(RateId::Refuel(van))
        {
            return Some(refuel);
        }
    }
    // Relay packs the cold van's spare slot for hub staging. Other policies
    // keep one destination per trip. Heavy freight never fits the cold van.
    let may_load = cargo.is_empty()
        || (policy == Policy::Relay
            && van == Van::Cold
            && from == Location::Depot
            && cargo.iter().any(|id| cold(*id)));
    if may_load {
        let mut waiting: Vec<_> = (0..count)
            .filter(|id| {
                value(world, AccountId::Order(*id), Asset::Waiting) > 0
                    && value(world, AccountId::Order(*id), Asset::At(from)) > 0
            })
            .collect();
        waiting.sort_by_key(|id| {
            (
                if van == Van::Cold {
                    !cold(*id)
                } else {
                    weight(*id) != 3
                },
                value(world, AccountId::Order(*id), Asset::Deadline),
            )
        });
        for id in waiting {
            if policy == Policy::Relay && from == Location::Hub && van == Van::Cold && !cold(id) {
                continue;
            }
            if let Some(load) = propose(RateId::Load {
                van,
                order: id,
                at: from,
            }) {
                return Some(load);
            }
        }
    }
    if let Some(id) = cargo.first() {
        let target = if policy == Policy::Relay
            && van == Van::Cold
            && from == Location::Depot
            && cargo.len() > 1
        {
            Location::Hub
        } else {
            destination(*id)
        };
        if let Some(next) = drive(target) {
            return Some(next);
        }
    } else {
        if policy == Policy::Relay
            && van == Van::Freight
            && (0..count).any(|id| {
                value(world, AccountId::Order(id), Asset::Waiting) > 0
                    && value(world, AccountId::Order(id), Asset::At(Location::Hub)) > 0
                    && !cold(id)
            })
            && let Some(next) = drive(Location::Hub)
        {
            return Some(next);
        }
        if from != Location::Depot
            && let Some(next) = drive(Location::Depot)
        {
            return Some(next);
        }
    }
    // Recovery from exhausted fuel or wear can use the paid shared tow truck.
    if from != Location::Depot && (val(Asset::Wear) >= 6 || val(Asset::Fuel) == 0) {
        return propose(RateId::Tow { van, from });
    }
    None
}
pub fn run(source: &World, policy: Policy) -> Proposal {
    let mut world = source.clone();
    let mut trace = Trace::new();
    for _ in 0..700 {
        if world.matches(&goal()) {
            return trace;
        }
        let exchange = response(&world)
            .or_else(|| {
                let close = action(RateId::Close);
                world.is_applicable(&close).then_some(close)
            })
            .or_else(|| {
                VANS.into_iter()
                    .find_map(|van| decision(&world, van, policy))
            })
            .unwrap_or_else(|| action(RateId::Tick));
        world.apply(exchange.clone()).unwrap_or_else(|error| {
            panic!(
                "{policy:?} t={}: {:?}: {error:?}",
                now(&world),
                exchange.rate()
            )
        });
        trace.push(exchange);
    }
    panic!("bounded fleet run did not close")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settle(world: &mut World) {
        while let Some(exchange) = response(world) {
            world.apply(exchange).unwrap();
        }
    }
    #[test]
    fn fleet_policies_replay_common_tickets_and_distinct_physical_outcomes() {
        let mut outcomes = std::collections::BTreeSet::new();
        let mut stages = false;
        let mut recovered = false;
        let mut overlap = false;
        for stress in [false, true] {
            for seed in [0, 3, 17, 29] {
                for policy in [Policy::Fastest, Policy::Reliable, Policy::Relay] {
                    let source = initial(stress, seed);
                    let before = source.state_key();
                    let trace = run(&source, policy);
                    assert_eq!(trace, run(&source, policy));
                    let mut replay = source.clone();
                    for exchange in trace.exchanges() {
                        replay.apply(exchange.clone()).unwrap();
                        let busy: u64 = VANS
                            .into_iter()
                            .map(|van| value(&replay, AccountId::Vehicle(van), Asset::OnMainRoad))
                            .sum();
                        assert_eq!(busy + value(&replay, AccountId::World, Asset::RoadSpace), 1);
                        overlap |= VANS
                            .into_iter()
                            .all(|van| value(&replay, AccountId::Vehicle(van), Asset::Ready) == 0);
                        for van in VANS {
                            let own = AccountId::Vehicle(van);
                            assert_eq!(
                                value(&replay, own, Asset::Fuel)
                                    + value(&replay, own, Asset::TankSpace),
                                8
                            );
                            let cargo: u64 = (0..value(&source, AccountId::World, Asset::Count)
                                as u8)
                                .map(|id| value(&replay, own, Asset::Cargo(id)) * weight(id))
                                .sum();
                            assert_eq!(
                                cargo + value(&replay, own, Asset::Space),
                                if van == Van::Cold { 2 } else { 3 }
                            );
                        }
                    }
                    assert_eq!(before, source.state_key());
                    assert!(replay.matches(&goal()));
                    assert_eq!(
                        value(&replay, AccountId::World, Asset::Resolved),
                        value(&source, AccountId::World, Asset::Count)
                    );
                    assert_eq!(value(&replay, AccountId::Station, Asset::FuelPump), 1);
                    assert_eq!(value(&replay, AccountId::Station, Asset::RepairBay), 1);
                    assert_eq!(value(&replay, AccountId::Station, Asset::TowTruck), 1);
                    stages |= trace
                        .exchanges()
                        .iter()
                        .any(|exchange| matches!(exchange.rate(), RateId::Stage { .. }));
                    recovered |= trace.exchanges().iter().any(|exchange| {
                        matches!(exchange.rate(), RateId::Tow { .. } | RateId::Repair { .. })
                    });
                    let delivered = value(&replay, AccountId::World, Asset::DeliveredCount);
                    let expired = value(&replay, AccountId::World, Asset::ExpiredCount);
                    outcomes.insert((delivered, expired, now(&replay)));
                    if seed == 17 {
                        eprintln!(
                            "{stress:?} {policy:?}: delivered {delivered} expired {expired} time {} stage {} revenue {}",
                            now(&replay),
                            value(&replay, AccountId::World, Asset::StagedCount),
                            value(&replay, AccountId::World, Asset::Revenue)
                        );
                    }
                }
            }
        }
        assert!(stages && recovered && overlap);
        assert!(outcomes.len() >= 4, "{outcomes:?}");
    }
    #[test]
    fn cargo_compatibility_and_trunk_occupancy_are_core_constraints() {
        let mut world = initial(false, 17);
        settle(&mut world);
        assert!(!world.is_applicable(&action(RateId::Load {
            van: Van::Freight,
            order: 0,
            at: Location::Depot
        })));
        assert!(!world.is_applicable(&action(RateId::Load {
            van: Van::Cold,
            order: 1,
            at: Location::Depot
        })));
        world
            .apply(action(RateId::Load {
                van: Van::Cold,
                order: 0,
                at: Location::Depot,
            }))
            .unwrap();
        assert!(!world.is_applicable(&action(RateId::Deliver {
            van: Van::Cold,
            order: 0,
            late: false
        })));
        world.apply(action(RateId::Tick)).unwrap();
        world
            .apply(action(RateId::Depart {
                van: Van::Cold,
                from: Location::Depot,
                to: Location::Hub,
            }))
            .unwrap();
        assert!(!world.is_applicable(&action(RateId::Depart {
            van: Van::Freight,
            from: Location::Depot,
            to: Location::Hub
        })));
        assert!(!world.is_applicable(&action(RateId::Arrive {
            van: Van::Cold,
            from: Location::Depot,
            to: Location::Hub
        })));
        settle(&mut world);
        assert!(!world.is_applicable(&action(RateId::Arrive {
            van: Van::Cold,
            from: Location::Depot,
            to: Location::Hub
        })));
        while value(&world, AccountId::World, Asset::RoadSpace) == 0 {
            world.apply(action(RateId::Tick)).unwrap();
            settle(&mut world);
        }
        assert!(world.is_applicable(&action(RateId::Depart {
            van: Van::Freight,
            from: Location::Depot,
            to: Location::Hub
        })));
    }
    #[test]
    fn handoff_and_service_use_real_cargo_funds_and_capacity() {
        let mut world = initial(false, 3);
        settle(&mut world);
        world
            .apply(action(RateId::Load {
                van: Van::Cold,
                order: 2,
                at: Location::Depot,
            }))
            .unwrap();
        world.apply(action(RateId::Tick)).unwrap();
        let transfer = action(RateId::Transfer {
            from: Van::Cold,
            to: Van::Freight,
            order: 2,
            at: Location::Depot,
        });
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
        world.apply(action(RateId::Tick)).unwrap();
        world.apply(action(RateId::Refuel(Van::Cold))).unwrap();
        assert!(!world.is_applicable(&action(RateId::Refuel(Van::Freight))));
        assert!(!world.is_applicable(&action(RateId::EndRefuel(Van::Cold))));
        assert_eq!(value(&world, AccountId::World, Asset::Money), 17);
        assert_eq!(value(&world, AccountId::Station, Asset::Fuel), 12);
    }
    #[test]
    fn dispatch_does_not_use_unrealized_weather_tickets() {
        let mut a = initial(false, 0);
        let mut b = initial(false, 17);
        settle(&mut a);
        settle(&mut b);
        assert_ne!(a.state_key(), b.state_key());
        let catalog = |world: &World| {
            let mut rates: Vec<_> = world.rate_ids().copied().collect();
            rates.sort();
            rates
        };
        assert_eq!(
            catalog(&a),
            catalog(&b),
            "rate availability must not disclose future weather"
        );
        for policy in [Policy::Fastest, Policy::Reliable, Policy::Relay] {
            for van in VANS {
                assert_eq!(decision(&a, van, policy), decision(&b, van, policy));
            }
        }
    }
}
