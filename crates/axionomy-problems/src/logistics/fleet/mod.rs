//! Timed heterogeneous fleet with cargo, a shared road and finite services.
use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, OptionPattern, Param,
    Pattern, Quantity, QuantityComparison as C, QuantityExpression as Q, Rate, Relation, Rounding,
    Slot, Variable, basket,
};

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
    /// A road from one place to another, as the fuel a trip along it burns:
    /// a fact of the world's map.
    Road(Location, Location),
    /// The trunk road the vans share, as the one lane a van on it takes: a
    /// fact of the world's map.
    Trunk(Location, Location),
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
    /// When an undelivered order expires: a cold shipment the tick after
    /// its window closes, other freight at the season's horizon.
    Expiry,
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
/// One unit of each asset `assets` name.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}
/// One event of the fleet, its world role fixed to the world's account.
fn event() -> Rate<Ontology> {
    Rate::new()
        .account(Role::World, Pattern::literal(AccountId::World))
        .condition(
            path!("one-fleet-event"),
            Q::units(),
            C::Equal,
            Q::constant(1),
        )
}
fn law() -> Rate<Ontology> {
    event().preserve(Role::World, basket([(Asset::Active, 1)]))
}
/// `rate` for each van, `{van}`, its vehicle role fixed to that van.
fn each_van(rate: Rate<Ontology>, van: &Param<Van>) -> Rate<Ontology> {
    rate.variable(van.every())
        .account(Role::Vehicle, AccountIdPattern::Vehicle(van.var()).into())
}
/// `rate` for each order `orders` takes, `{order}`, its order role fixed to
/// that order.
fn each_order(rate: Rate<Ontology>, order: &Param<u8>, orders: Variable) -> Rate<Ontology> {
    rate.variable(orders)
        .account(Role::Order, AccountIdPattern::Order(order.var()).into())
}
/// `rate` with the station, its role fixed to the station's account.
fn station(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Station, Pattern::literal(AccountId::Station))
}
/// `rate` spending the dispatch permit of the van `van` names this tick.
fn permit(rate: Rate<Ontology>, van: &Param<Van>) -> Rate<Ontology> {
    rate.consume_at(Role::World, one([AssetPattern::Permit(van.var()).into()]))
        .produce_at(
            Role::World,
            one([AssetPattern::UsedPermit(van.var()).into()]),
        )
}
/// A decision of the van `{van}`, ready and with its permit.
fn ready(van: &Param<Van>) -> Rate<Ontology> {
    permit(each_van(law(), van), van).preserve(Role::Vehicle, basket([(Asset::Ready, 1)]))
}
fn deadline(rate: Rate<Ontology>, duration: u64) -> Rate<Ontology> {
    rate.condition(
        path!("activity-fits-remaining-horizon"),
        clock() + Q::constant(duration),
        C::LessThanOrEqual,
        q(Role::World, Asset::Horizon),
    )
}
/// The van `{van}` ending the activity `state` once its time is up.
fn finish(van: &Param<Van>, state: Pattern<Asset>) -> Rate<Ontology> {
    each_van(law(), van)
        .condition(
            path!("activity-duration-elapsed"),
            clock(),
            C::GreaterThanOrEqual,
            q(Role::Vehicle, Asset::ReadyAt),
        )
        .consume_at(Role::Vehicle, one([state]))
        .produce(Role::Vehicle, basket([(Asset::Ready, 1)]))
        .consume_computed(
            Role::Vehicle,
            Asset::ReadyAt,
            q(Role::Vehicle, Asset::ReadyAt),
        )
}
/// The order's weight, as its account records it.
fn order_weight() -> Q<Ontology> {
    q(Role::Order, Asset::Weight)
}
/// `rate` freeing, in the van `role` plays, the payload space the order's
/// cargo takes and the refrigerated slot a cold one takes.
fn release_capacity(rate: Rate<Ontology>, role: Role) -> Rate<Ontology> {
    rate.produce_computed(role, Asset::Space, order_weight())
        .produce_computed(role, Asset::ColdSpace, q(Role::Order, Asset::ColdCargo))
}
/// `rate` taking, in the van `role` plays, the payload space the order's
/// cargo takes and the refrigerated slot a cold one takes.
fn take_capacity(rate: Rate<Ontology>, role: Role) -> Rate<Ontology> {
    rate.consume_computed(role, Asset::Space, order_weight())
        .consume_computed(role, Asset::ColdSpace, q(Role::Order, Asset::ColdCargo))
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
    let count: u8 = if stress { 6 } else { 4 };
    let horizon = if stress { 52 } else { 40 };
    let mut world = basket([
        (Asset::Active, 1),
        (Asset::Horizon, horizon),
        (Asset::TimeRemaining, horizon),
        (Asset::Count, count.into()),
        (Asset::Money, 20),
        (Asset::RoadSpace, 1),
        (Asset::DockSpace, 2),
        (Asset::Permit(Van::Cold), 1),
        (Asset::Permit(Van::Freight), 1),
    ]);
    for (from, to) in directed_roads() {
        let fuel = if express(from, to) { 2 } else { 1 };
        world.insert(Asset::Road(from, to), Quantity::new(fuel));
        if main_road(from, to) {
            world.insert(Asset::Trunk(from, to), Quantity::new(1));
        }
    }
    let mut builder = EconomyBuilder::new()
        .account(AccountId::World, Account::from(world))
        .account(
            AccountId::Station,
            Account::from(basket([
                (Asset::Fuel, if stress { 18 } else { 15 }),
                (Asset::FuelPump, 1),
                (Asset::RepairBay, 1),
                (Asset::Part, if stress { 3 } else { 2 }),
                (Asset::TowTruck, 1),
            ])),
        );
    let mut nature = axionomy::Basket::new();
    for van in VANS {
        let capacity = if van == Van::Cold { 2 } else { 3 };
        let mut stock = basket([
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
            (Asset::Scheduled, 1),
            (Asset::At(Location::Depot), 1),
            (Asset::Arrival, arrival),
            (Asset::Deadline, due),
            (Asset::Expiry, if cold(id) { due + 1 } else { horizon }),
            (Asset::Destination(destination(id)), 1),
            (Asset::Weight, weight(id)),
            (Asset::Price, price),
            (Asset::Money, price),
        ]);
        if cold(id) {
            stock.insert(Asset::ColdCargo, Quantity::new(1));
        }
        builder = builder.account(AccountId::Order(id), Account::from(stock));
    }

    // The rates' variables: a van, an order and the two ends of a road.
    let van = Param::<Van>::new(path!("van"));
    let order = Param::<u8>::new(path!("order"));
    let (from, to) = (
        Param::<Location>::new(path!("from")),
        Param::new(path!("to")),
    );
    let every_order = || order.range(0, count - 1);
    let at = |place: &Param<Location>| AssetPattern::At(place.var()).into();
    let cargo = || AssetPattern::Cargo(order.var()).into();
    let travelling = || AssetPattern::Travelling(from.var(), to.var()).into();
    // A trip goes along a road, which the world's map says the fuel of and
    // whether it is the trunk road.
    let on_road = |rate: Rate<Ontology>| {
        rate.variable(from.every())
            .variable(to.every())
            .relation(Relation::pairs(&from, &to, directed_roads()))
    };
    let road = || Q::balance_at(Role::World, AssetPattern::Road(from.var(), to.var()).into());
    let trunk = || {
        Q::balance_at(
            Role::World,
            AssetPattern::Trunk(from.var(), to.var()).into(),
        )
    };

    builder = builder.rate_at(
        RateIdPattern::Depart {
            van: van.var(),
            from: from.var(),
            to: to.var(),
        }
        .into(),
        on_road(deadline(permit(each_van(law(), &van), &van), 6))
            .preserve(Role::Vehicle, basket([(Asset::Integrity, 1)]))
            .condition(
                path!("service-before-excessive-wear"),
                q(Role::Vehicle, Asset::Wear),
                C::LessThanOrEqual,
                Q::constant(5),
            )
            .condition(
                path!("finite-travel-tickets"),
                q(Role::Vehicle, Asset::Trip),
                C::LessThanOrEqual,
                Q::constant(23),
            )
            .consume_at(
                Role::Vehicle,
                one([Pattern::literal(Asset::Ready), at(&from)]),
            )
            .consume_computed(Role::Vehicle, Asset::Fuel, road())
            .produce_at(
                Role::Vehicle,
                one([travelling(), Pattern::literal(Asset::AwaitingOutcome)]),
            )
            .produce_computed(Role::Vehicle, Asset::SpentFuel, road())
            .produce_computed(Role::Vehicle, Asset::TankSpace, road())
            .produce_computed(Role::Vehicle, Asset::DepartedAt, clock())
            .consume_computed(Role::World, Asset::RoadSpace, trunk())
            .produce_computed(Role::Vehicle, Asset::OnMainRoad, trunk()),
    );
    for outcome in [Outcome::Clear, Outcome::Delayed, Outcome::Breakdown] {
        let duration = match outcome {
            Outcome::Clear => 2,
            Outcome::Delayed => 4,
            Outcome::Breakdown => 6,
        };
        // Bit extraction from the hidden ticket: floor is the operation
        // itself, not a settlement choice.
        let shifted = Q::divide(
            Q::balance_at(
                Role::Nature,
                AssetPattern::TicketWord(van.var(), from.var(), to.var()).into(),
            ),
            q(Role::Vehicle, Asset::TicketDivisor),
            Rounding::Floor,
        );
        let code = shifted.clone()
            - (Q::divide(shifted, Q::constant(4), Rounding::Floor) * Q::constant(4));
        let mut resolution = on_road(each_van(law(), &van))
            .account(Role::Nature, Pattern::literal(AccountId::Nature))
            .preserve_at(Role::Vehicle, one([travelling()]))
            .condition(
                path!("match-hidden-vehicle-trip-route-ticket"),
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
            );
        if outcome == Outcome::Breakdown {
            resolution = resolution
                .consume(Role::Vehicle, basket([(Asset::Integrity, 1)]))
                .produce(Role::Vehicle, basket([(Asset::Damage, 1)]))
                .produce(Role::World, basket([(Asset::DamageCount, 1)]));
        }
        builder = builder.rate_at(
            RateIdPattern::Resolve {
                van: van.var(),
                from: from.var(),
                to: to.var(),
                outcome: Slot::Is(outcome),
            }
            .into(),
            resolution,
        );
    }
    builder = builder.rate_at(
        RateIdPattern::Arrive {
            van: van.var(),
            from: from.var(),
            to: to.var(),
        }
        .into(),
        on_road(finish(&van, travelling()))
            .condition(
                path!("nature-has-resolved-the-trip"),
                q(Role::Vehicle, Asset::AwaitingOutcome),
                C::Equal,
                Q::constant(0),
            )
            .produce_at(Role::Vehicle, one([at(&to)]))
            .consume_computed(
                Role::Vehicle,
                Asset::DepartedAt,
                q(Role::Vehicle, Asset::DepartedAt),
            )
            .consume_computed(Role::Vehicle, Asset::OnMainRoad, trunk())
            .produce_computed(Role::World, Asset::RoadSpace, trunk()),
    );
    builder = builder.rate_at(
        RateIdPattern::Refuel(van.var()).into(),
        station(deadline(permit(each_van(law(), &van), &van), 2))
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
    builder = builder.rate_at(
        RateIdPattern::EndRefuel(van.var()).into(),
        station(finish(&van, Pattern::literal(Asset::Refueling)))
            .produce(Role::Station, basket([(Asset::FuelPump, 1)])),
    );
    // Repairing a broken-down van and servicing a worn one differ in what
    // they require and what they restore.
    for damaged in [false, true] {
        let condition = if damaged {
            Asset::Damage
        } else {
            Asset::Integrity
        };
        let mut repair = station(deadline(permit(each_van(law(), &van), &van), 3))
            .preserve(
                Role::Vehicle,
                basket([(Asset::At(Location::Depot), 1), (condition, 1)]),
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
                path!("preventive-service-after-accumulated-wear"),
                q(Role::Vehicle, Asset::Wear),
                C::GreaterThanOrEqual,
                Q::constant(3),
            );
        }
        builder = builder.rate_at(
            RateIdPattern::Repair {
                van: van.var(),
                damaged: Slot::Is(damaged),
            }
            .into(),
            repair,
        );
        let mut end = station(finish(&van, Pattern::literal(Asset::Repairing)))
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
        builder = builder.rate_at(
            RateIdPattern::EndRepair {
                van: van.var(),
                damaged: Slot::Is(damaged),
            }
            .into(),
            end,
        );
    }
    builder = builder.rate_at(
        RateIdPattern::Tow {
            van: van.var(),
            from: from.var(),
        }
        .into(),
        station(deadline(permit(each_van(law(), &van), &van), 3))
            .variable(
                from.values(
                    LOCATIONS
                        .into_iter()
                        .filter(|location| *location != Location::Depot),
                ),
            )
            .consume_at(
                Role::Vehicle,
                one([Pattern::literal(Asset::Ready), at(&from)]),
            )
            .produce(Role::Vehicle, basket([(Asset::Towing, 1)]))
            .produce_computed(Role::Vehicle, Asset::ReadyAt, clock() + Q::constant(3))
            .consume(Role::Station, basket([(Asset::TowTruck, 1)]))
            .produce(Role::Station, basket([(Asset::Money, 4)]))
            .consume(Role::World, basket([(Asset::Money, 4)])),
    );
    builder = builder.rate_at(
        RateIdPattern::EndTow(van.var()).into(),
        station(finish(&van, Pattern::literal(Asset::Towing)))
            .produce(Role::Station, basket([(Asset::TowTruck, 1)]))
            .produce(Role::Vehicle, basket([(Asset::At(Location::Depot), 1)])),
    );

    builder = builder.rate_at(
        RateIdPattern::Release(order.var()).into(),
        each_order(law(), &order, every_order())
            .condition(
                path!("customer-order-has-arrived"),
                clock(),
                C::GreaterThanOrEqual,
                q(Role::Order, Asset::Arrival),
            )
            .consume(Role::Order, basket([(Asset::Scheduled, 1)]))
            .produce(Role::Order, basket([(Asset::Waiting, 1)])),
    );
    // Loading at the hub also frees the staging space the order held there.
    for place in [Location::Depot, Location::Hub] {
        let mut load = take_capacity(
            each_order(ready(&van), &order, every_order()),
            Role::Vehicle,
        )
        .preserve(Role::Vehicle, basket([(Asset::At(place), 1)]))
        .consume(
            Role::Order,
            basket([(Asset::Waiting, 1), (Asset::At(place), 1)]),
        )
        .produce(Role::Order, basket([(Asset::InTransit, 1)]))
        .produce_at(Role::Vehicle, one([cargo()]));
        if place == Location::Hub {
            load = load
                .consume_computed(Role::Order, Asset::DockUsed, order_weight())
                .produce_computed(Role::World, Asset::DockSpace, order_weight());
        }
        builder = builder.rate_at(
            RateIdPattern::Load {
                van: van.var(),
                order: order.var(),
                at: Slot::Is(place),
            }
            .into(),
            load,
        );
    }
    builder = builder.rate_at(
        RateIdPattern::Stage {
            van: van.var(),
            order: order.var(),
        }
        .into(),
        release_capacity(
            each_order(ready(&van), &order, every_order()),
            Role::Vehicle,
        )
        .preserve(Role::Vehicle, basket([(Asset::At(Location::Hub), 1)]))
        .consume_at(Role::Vehicle, one([cargo()]))
        .consume(Role::Order, basket([(Asset::InTransit, 1)]))
        .produce(
            Role::Order,
            basket([(Asset::Waiting, 1), (Asset::At(Location::Hub), 1)]),
        )
        .produce_computed(Role::Order, Asset::DockUsed, order_weight())
        .consume_computed(Role::World, Asset::DockSpace, order_weight())
        .produce(Role::World, basket([(Asset::StagedCount, 1)])),
    );
    // A delivery is made where the van stands at the order's destination.
    let at_destination = LOCATIONS
        .into_iter()
        .map(|place| q(Role::Order, Asset::Destination(place)) * q(Role::Vehicle, Asset::At(place)))
        .reduce(|a, b| a + b)
        .expect("the fleet has places");
    // A late delivery earns half the price, which is exact since every
    // price is even; a cold load is never delivered late, it expires.
    for late in [false, true] {
        let price = q(Role::Order, Asset::Price);
        let payment = if late {
            Q::divide(price, Q::constant(2), Rounding::Floor)
        } else {
            price
        };
        let orders = if late {
            order.values((0..count).filter(|id| !cold(*id)))
        } else {
            every_order()
        };
        let mut deliver = release_capacity(each_order(ready(&van), &order, orders), Role::Vehicle)
            .condition(
                path!("van-at-order-destination"),
                at_destination.clone(),
                C::Equal,
                Q::constant(1),
            )
            .consume_at(Role::Vehicle, one([cargo()]))
            .consume(Role::Order, basket([(Asset::InTransit, 1)]))
            .consume_computed(Role::Order, Asset::Money, payment.clone())
            .produce(Role::Order, basket([(Asset::Delivered, 1)]))
            .produce(
                Role::World,
                basket([(Asset::Resolved, 1), (Asset::DeliveredCount, 1)]),
            )
            .produce_computed(Role::World, Asset::Money, payment.clone())
            .produce_computed(Role::World, Asset::Revenue, payment);
        if late {
            deliver = deliver.condition(
                path!("late-delivery-earns-half-payment"),
                clock(),
                C::GreaterThanOrEqual,
                q(Role::Order, Asset::Deadline) + Q::constant(1),
            );
        } else {
            deliver = deliver
                .condition(
                    path!("within-customer-window"),
                    clock(),
                    C::LessThanOrEqual,
                    q(Role::Order, Asset::Deadline),
                )
                .produce(Role::World, basket([(Asset::OnTimeCount, 1)]));
        }
        builder = builder.rate_at(
            RateIdPattern::Deliver {
                van: van.var(),
                order: order.var(),
                late: Slot::Is(late),
            }
            .into(),
            deliver,
        );
    }
    // A van hands an order's cargo to the other van where both stand, the
    // other van spending its own permit.
    let (giver, taker) = (Param::<Van>::new(path!("from")), Param::new(path!("to")));
    let meeting = Param::<Location>::new(path!("at"));
    builder = builder.rate_at(
        RateIdPattern::Transfer {
            from: giver.var(),
            to: taker.var(),
            order: order.var(),
            at: meeting.var(),
        }
        .into(),
        permit(
            take_capacity(
                release_capacity(
                    each_order(ready(&giver), &order, every_order()),
                    Role::Vehicle,
                ),
                Role::Other,
            ),
            &taker,
        )
        .variable(taker.every())
        .distinct_variables(giver.name().clone(), taker.name().clone())
        .account(Role::Other, AccountIdPattern::Vehicle(taker.var()).into())
        .variable(meeting.every())
        .preserve_at(Role::Vehicle, one([at(&meeting)]))
        .preserve_at(
            Role::Other,
            one([at(&meeting), Pattern::literal(Asset::Ready)]),
        )
        .preserve(Role::Order, basket([(Asset::InTransit, 1)]))
        .consume_at(Role::Vehicle, one([cargo()]))
        .produce_at(Role::Other, one([cargo()]))
        .produce(Role::World, basket([(Asset::TransferCount, 1)])),
    );
    // An order expires waiting at the depot or the hub, or aboard a van.
    let expiring = || {
        each_order(law(), &order, every_order())
            .condition(
                path!("order-has-expired"),
                clock(),
                C::GreaterThanOrEqual,
                q(Role::Order, Asset::Expiry),
            )
            .produce(Role::Order, basket([(Asset::Expired, 1)]))
            .produce(
                Role::World,
                basket([(Asset::Resolved, 1), (Asset::ExpiredCount, 1)]),
            )
    };
    for place in [Location::Depot, Location::Hub] {
        let mut expire = expiring().consume(
            Role::Order,
            basket([(Asset::Waiting, 1), (Asset::At(place), 1)]),
        );
        if place == Location::Hub {
            expire = expire
                .consume_computed(Role::Order, Asset::DockUsed, order_weight())
                .produce_computed(Role::World, Asset::DockSpace, order_weight());
        }
        builder = builder.rate_at(
            RateIdPattern::Expire {
                order: order.var(),
                carrier: Slot::Is(None),
                at: Slot::Is(place),
            }
            .into(),
            expire,
        );
    }
    builder = builder.rate_at(
        RateIdPattern::Expire {
            order: order.var(),
            carrier: Slot::Of(OptionPattern::Some(van.var())),
            at: Slot::Is(Location::Depot),
        }
        .into(),
        release_capacity(each_van(expiring(), &van), Role::Vehicle)
            .consume(Role::Order, basket([(Asset::InTransit, 1)]))
            .consume_at(Role::Vehicle, one([cargo()])),
    );
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
        event()
            .account(
                Role::Vehicle,
                Pattern::literal(AccountId::Vehicle(Van::Cold)),
            )
            .account(
                Role::Other,
                Pattern::literal(AccountId::Vehicle(Van::Freight)),
            )
            .condition(
                path!("all-customer-orders-resolved"),
                q(Role::World, Asset::Resolved),
                C::Equal,
                q(Role::World, Asset::Count),
            )
            .preserve(Role::Vehicle, basket([(Asset::Ready, 1)]))
            .preserve(Role::Other, basket([(Asset::Ready, 1)]))
            .consume(Role::World, basket([(Asset::Active, 1)]))
            .produce(Role::World, basket([(Asset::Closed, 1)])),
    );
    let mut space =
        LinearInvariant::new(path!("physical-vehicle-cargo-capacity")).weight(Asset::Space, 1);
    let mut cold_space =
        LinearInvariant::new(path!("one-refrigerated-cargo-slot")).weight(Asset::ColdSpace, 1);
    let mut activity = LinearInvariant::new(path!("two-vehicle-activities"))
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
    for (from, to) in directed_roads() {
        activity = activity.weight(Asset::Travelling(from, to), 1);
    }
    for van in VANS {
        builder = builder.invariant(
            LinearInvariant::new(path!("dispatch-permit").join(&path::write(&van)))
                .weight(Asset::Permit(van), 1)
                .weight(Asset::UsedPermit(van), 1),
        );
    }
    builder
        .invariant(space)
        .invariant(cold_space)
        .invariant(activity)
        .invariant(
            LinearInvariant::new(path!("cargo-lifecycle"))
                .weight(Asset::Scheduled, 1)
                .weight(Asset::Waiting, 1)
                .weight(Asset::InTransit, 1)
                .weight(Asset::Delivered, 1)
                .weight(Asset::Expired, 1),
        )
        .invariant(
            LinearInvariant::new(path!("one-occupied-trunk-road"))
                .weight(Asset::RoadSpace, 1)
                .weight(Asset::OnMainRoad, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-hub-staging-capacity"))
                .weight(Asset::DockSpace, 1)
                .weight(Asset::DockUsed, 1),
        )
        .invariant(
            LinearInvariant::new(path!("fuel-stock"))
                .weight(Asset::Fuel, 1)
                .weight(Asset::SpentFuel, 1),
        )
        .invariant(
            LinearInvariant::new(path!("funded-delivery-and-service-payments"))
                .weight(Asset::Money, 1),
        )
        .invariant(
            LinearInvariant::new(path!("vehicle-condition"))
                .weight(Asset::Integrity, 1)
                .weight(Asset::Damage, 1),
        )
        .invariant(
            LinearInvariant::new(path!("fuel-pump-occupancy"))
                .weight(Asset::FuelPump, 1)
                .weight(Asset::Refueling, 1),
        )
        .invariant(
            LinearInvariant::new(path!("repair-bay-occupancy"))
                .weight(Asset::RepairBay, 1)
                .weight(Asset::Repairing, 1),
        )
        .invariant(
            LinearInvariant::new(path!("tow-truck-occupancy"))
                .weight(Asset::TowTruck, 1)
                .weight(Asset::Towing, 1),
        )
        .invariant(
            LinearInvariant::new(path!("finite-repair-parts"))
                .weight(Asset::Part, 1)
                .weight(Asset::UsedPart, 1),
        )
        .invariant(
            LinearInvariant::new(path!("bounded-fleet-clock"))
                .weight(Asset::Time, 1)
                .weight(Asset::TimeRemaining, 1),
        )
        .invariant(
            LinearInvariant::new(path!("fleet-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Closed, 1),
        )
        .build()
        .expect("fleet mechanics conserve physical and financial resources")
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
