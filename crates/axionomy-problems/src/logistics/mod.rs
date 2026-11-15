#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Param, Pattern, Quantity,
    Rate, Slot, basket,
};
pub mod fleet;
#[cfg(feature = "packages")]
pub mod package;

const REFUEL_AMOUNT: u64 = 4;
const REPAIR_TIME: u64 = 2;
/// The most exchanges a run takes before it is cut short.
pub const ROLLOUT_HORIZON: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum OrderId {
    A,
    B,
    C,
    D,
}

pub const ORDERS: [OrderId; 4] = [OrderId::A, OrderId::B, OrderId::C, OrderId::D];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Location {
    Depot,
    Junction,
    Customer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Route {
    DirectOut,
    DirectBack,
    SafeOutFirst,
    SafeOutSecond,
    SafeBackFirst,
    SafeBackSecond,
}

pub const ROUTES: [Route; 6] = [
    Route::DirectOut,
    Route::DirectBack,
    Route::SafeOutFirst,
    Route::SafeOutSecond,
    Route::SafeBackFirst,
    Route::SafeBackSecond,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum TravelOutcome {
    Clear,
    Delayed,
    Breakdown,
}

pub const OUTCOMES: [TravelOutcome; 3] = [
    TravelOutcome::Clear,
    TravelOutcome::Delayed,
    TravelOutcome::Breakdown,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Vehicle,
    Order(OrderId),
    Nature,
    FuelStation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    At(Location),
    Traveling(Route),
    Fuel,
    SpentFuel,
    Money,
    TimeRemaining,
    ElapsedTime,
    CargoSpace,
    CargoOccupied,
    RepairTool,
    Waiting,
    InTransit,
    Delivered,
    Package(OrderId),
    WeatherReady,
    OutcomeWeight(Route, TravelOutcome),
    Outcome(Route, TravelOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Vehicle,
    Order,
    Nature,
    FuelStation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Load(OrderId),
    Depart(Route),
    Resolve(Route, TravelOutcome),
    Arrive(Route, TravelOutcome),
    Repair(Route),
    Deliver(OrderId),
    Refuel,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `logistics` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "logistics";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;

pub fn initial() -> World {
    build(&ORDERS)
}

/// One remaining delivery for fast transport and oracle checks; the other
/// orders are already represented as completed economic state.
pub fn initial_micro() -> World {
    build(&[OrderId::A])
}

/// One unit of each asset `assets` name.
fn one(assets: impl IntoIterator<Item = Pattern<Asset>>) -> Vec<(Pattern<Asset>, Quantity)> {
    assets
        .into_iter()
        .map(|asset| (asset, Quantity::new(1)))
        .collect()
}

/// `rate` with the vehicle, its role fixed to the vehicle's account.
fn with_vehicle(rate: Rate<Ontology>) -> Rate<Ontology> {
    rate.account(Role::Vehicle, Pattern::literal(AccountId::Vehicle))
}

/// `rate` with the vehicle and Nature, each role fixed to its account.
fn with_nature(rate: Rate<Ontology>) -> Rate<Ontology> {
    with_vehicle(rate).account(Role::Nature, Pattern::literal(AccountId::Nature))
}

fn build(pending: &[OrderId]) -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Vehicle,
            Account::from(basket([
                (Asset::At(Location::Depot), 1),
                (Asset::Fuel, REFUEL_AMOUNT),
                (Asset::Money, 12),
                (Asset::TimeRemaining, 80),
                (Asset::CargoSpace, 1),
                (Asset::RepairTool, 1),
            ])),
        )
        .account(AccountId::Nature, Account::from(nature_assets()))
        .account(
            AccountId::FuelStation,
            Account::from(basket([(Asset::Fuel, 12)])),
        );
    for order in ORDERS {
        let order_state = if pending.contains(&order) {
            basket([(Asset::Waiting, 1), (Asset::Package(order), 1)])
        } else {
            basket([(Asset::Delivered, 1), (Asset::Package(order), 1)])
        };
        builder = builder.account(AccountId::Order(order), Account::from(order_state));
    }

    // The rates' variables: an order, a route and the weather on it.
    let order = Param::<OrderId>::new(path!("order"));
    let route = Param::<Route>::new(path!("route"));
    let outcome = Param::<TravelOutcome>::new(path!("outcome"));
    let package = || AssetPattern::Package(order.var()).into();
    let traveling = || AssetPattern::Traveling(route.var()).into();
    // `rate` for each order, `{order}`, its role fixed to that order.
    let each_order = |rate: Rate<Ontology>| {
        with_vehicle(rate)
            .variable(order.every())
            .account(Role::Order, AccountIdPattern::Order(order.var()).into())
    };
    builder = builder
        .rate_at(
            RateIdPattern::Load(order.var()).into(),
            each_order(Rate::new())
                .preserve(Role::Vehicle, basket([(Asset::At(Location::Depot), 1)]))
                .consume(Role::Vehicle, basket([(Asset::CargoSpace, 1)]))
                .produce_at(
                    Role::Vehicle,
                    one([Pattern::literal(Asset::CargoOccupied), package()]),
                )
                .consume_at(
                    Role::Order,
                    one([Pattern::literal(Asset::Waiting), package()]),
                )
                .produce(Role::Order, basket([(Asset::InTransit, 1)])),
        )
        .rate_at(
            RateIdPattern::Deliver(order.var()).into(),
            each_order(Rate::new())
                .preserve(Role::Vehicle, basket([(Asset::At(Location::Customer), 1)]))
                .consume_at(
                    Role::Vehicle,
                    one([Pattern::literal(Asset::CargoOccupied), package()]),
                )
                .produce(Role::Vehicle, basket([(Asset::CargoSpace, 1)]))
                .consume(Role::Order, basket([(Asset::InTransit, 1)]))
                .produce_at(
                    Role::Order,
                    one([Pattern::literal(Asset::Delivered), package()]),
                ),
        )
        // Nature draws a leg's weather from the weights it holds.
        .rate_at(
            RateIdPattern::Resolve(route.var(), outcome.var()).into(),
            with_nature(Rate::new())
                .variable(route.every())
                .variable(outcome.every())
                .preserve_at(Role::Vehicle, one([traveling()]))
                .consume(Role::Nature, basket([(Asset::WeatherReady, 1)]))
                .preserve_at(
                    Role::Nature,
                    one([AssetPattern::OutcomeWeight(route.var(), outcome.var()).into()]),
                )
                .produce_at(
                    Role::Nature,
                    one([AssetPattern::Outcome(route.var(), outcome.var()).into()]),
                ),
        )
        // A breakdown is repaired where it happened, and the weather drawn
        // again.
        .rate_at(
            RateIdPattern::Repair(route.var()).into(),
            with_nature(Rate::new())
                .variable(route.every())
                .preserve_at(
                    Role::Vehicle,
                    one([traveling(), Pattern::literal(Asset::RepairTool)]),
                )
                .consume(Role::Vehicle, basket([(Asset::TimeRemaining, REPAIR_TIME)]))
                .produce(Role::Vehicle, basket([(Asset::ElapsedTime, REPAIR_TIME)]))
                .consume_at(
                    Role::Nature,
                    one([
                        AssetPattern::Outcome(route.var(), Slot::Is(TravelOutcome::Breakdown))
                            .into(),
                    ]),
                )
                .produce(Role::Nature, basket([(Asset::WeatherReady, 1)])),
        );

    // A route's departure and arrivals are its own: where it starts and
    // ends, and the fuel and time a leg takes, are the route's, not values
    // an identifier gives.
    for route in ROUTES {
        builder = builder.rate(
            RateId::Depart(route),
            with_vehicle(Rate::new())
                .consume(Role::Vehicle, basket([(Asset::At(route_source(route)), 1)]))
                .produce(Role::Vehicle, basket([(Asset::Traveling(route), 1)])),
        );
        for outcome in [TravelOutcome::Clear, TravelOutcome::Delayed] {
            let (fuel, time) = travel_cost(route, outcome);
            builder = builder.rate(
                RateId::Arrive(route, outcome),
                with_nature(Rate::new())
                    .consume(
                        Role::Vehicle,
                        basket([
                            (Asset::Traveling(route), 1),
                            (Asset::Fuel, fuel),
                            (Asset::TimeRemaining, time),
                        ]),
                    )
                    .produce(
                        Role::Vehicle,
                        basket([
                            (Asset::At(route_destination(route)), 1),
                            (Asset::SpentFuel, fuel),
                            (Asset::ElapsedTime, time),
                        ]),
                    )
                    .consume(Role::Nature, basket([(Asset::Outcome(route, outcome), 1)]))
                    .produce(Role::Nature, basket([(Asset::WeatherReady, 1)])),
            );
        }
    }

    builder
        .rate(
            RateId::Refuel,
            with_vehicle(Rate::new())
                .account(Role::FuelStation, Pattern::literal(AccountId::FuelStation))
                .preserve(Role::Vehicle, basket([(Asset::At(Location::Depot), 1)]))
                .consume(Role::Vehicle, basket([(Asset::Money, REFUEL_AMOUNT)]))
                .produce(Role::Vehicle, basket([(Asset::Fuel, REFUEL_AMOUNT)]))
                .consume(Role::FuelStation, basket([(Asset::Fuel, REFUEL_AMOUNT)]))
                .produce(Role::FuelStation, basket([(Asset::Money, REFUEL_AMOUNT)])),
        )
        .invariant(
            [Location::Depot, Location::Junction, Location::Customer]
                .into_iter()
                .fold(
                    ROUTES.into_iter().fold(
                        LinearInvariant::new(path!("vehicle-position")),
                        |invariant, route| invariant.weight(Asset::Traveling(route), 1),
                    ),
                    |invariant, location| invariant.weight(Asset::At(location), 1),
                ),
        )
        .invariant(
            LinearInvariant::new(path!("fuel-accounting"))
                .weight(Asset::Fuel, 1)
                .weight(Asset::SpentFuel, 1),
        )
        .invariant(LinearInvariant::new(path!("money-accounting")).weight(Asset::Money, 1))
        .invariant(
            LinearInvariant::new(path!("time-accounting"))
                .weight(Asset::TimeRemaining, 1)
                .weight(Asset::ElapsedTime, 1),
        )
        .invariant(
            LinearInvariant::new(path!("cargo-capacity"))
                .weight(Asset::CargoSpace, 1)
                .weight(Asset::CargoOccupied, 1),
        )
        .invariant(
            LinearInvariant::new(path!("order-lifecycle"))
                .weight(Asset::Waiting, 1)
                .weight(Asset::InTransit, 1)
                .weight(Asset::Delivered, 1),
        )
        .build()
        .expect("logistics model is valid")
}

pub fn goal() -> Goal<Ontology> {
    ORDERS.into_iter().fold(Goal::new(), |goal, order| {
        goal.require(AccountId::Order(order), basket([(Asset::Delivered, 1)]))
    })
}

fn nature_assets() -> axionomy::Basket<Asset> {
    let mut assets = basket([(Asset::WeatherReady, 1)]);
    for route in ROUTES {
        for outcome in OUTCOMES {
            let weight = match (is_direct(route), outcome) {
                (true, TravelOutcome::Clear) => 2,
                (true, TravelOutcome::Delayed | TravelOutcome::Breakdown) => 1,
                (false, TravelOutcome::Clear) => 8,
                (false, TravelOutcome::Delayed | TravelOutcome::Breakdown) => 1,
            };
            assets.insert(Asset::OutcomeWeight(route, outcome), Quantity::new(weight));
        }
    }
    assets
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
        .unwrap_or_else(|_| panic!("logistics has no rate {rate:?}"));
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

const fn route_source(route: Route) -> Location {
    match route {
        Route::DirectOut | Route::SafeOutFirst => Location::Depot,
        Route::DirectBack | Route::SafeBackFirst => Location::Customer,
        Route::SafeOutSecond | Route::SafeBackSecond => Location::Junction,
    }
}

const fn route_destination(route: Route) -> Location {
    match route {
        Route::DirectOut | Route::SafeOutSecond => Location::Customer,
        Route::DirectBack | Route::SafeBackSecond => Location::Depot,
        Route::SafeOutFirst | Route::SafeBackFirst => Location::Junction,
    }
}

const fn is_direct(route: Route) -> bool {
    matches!(route, Route::DirectOut | Route::DirectBack)
}

const fn travel_cost(route: Route, outcome: TravelOutcome) -> (u64, u64) {
    let fuel = if is_direct(route) { 2 } else { 1 };
    let time = match (is_direct(route), outcome) {
        (true, TravelOutcome::Clear) => 2,
        (true, TravelOutcome::Delayed) => 5,
        (false, TravelOutcome::Clear) => 1,
        (false, TravelOutcome::Delayed) => 2,
        (_, TravelOutcome::Breakdown) => unreachable!(),
    };
    (fuel, time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::Trace;

    #[test]
    fn a_delivery_through_a_breakdown_replays_with_explicit_nature_outcomes() {
        let first = Route::SafeOutFirst;
        let second = Route::SafeOutSecond;
        let rates = [
            RateId::Load(OrderId::A),
            RateId::Depart(first),
            RateId::Resolve(first, TravelOutcome::Breakdown),
            RateId::Repair(first),
            RateId::Resolve(first, TravelOutcome::Clear),
            RateId::Arrive(first, TravelOutcome::Clear),
            RateId::Depart(second),
            RateId::Resolve(second, TravelOutcome::Clear),
            RateId::Arrive(second, TravelOutcome::Clear),
            RateId::Deliver(OrderId::A),
        ];
        let model = initial_micro();

        // A broken-down vehicle arrives only after its repair and the
        // weather Nature draws again.
        let broken: Trace<Ontology> = rates[..3]
            .iter()
            .map(|rate| action(&model, *rate))
            .collect();
        let broken = model.replayed(&broken).expect("the breakdown replays");
        assert!(!broken.is_applicable(&action(
            &broken,
            RateId::Arrive(first, TravelOutcome::Clear)
        )));
        assert!(!broken.is_applicable(&action(
            &broken,
            RateId::Resolve(first, TravelOutcome::Clear)
        )));

        let trace: Trace<Ontology> = rates.into_iter().map(|rate| action(&model, rate)).collect();
        let replayed = model.replayed(&trace).expect("the delivery replays");
        assert!(replayed.matches(&goal()));
        assert_eq!(
            replayed.balance(&AccountId::Vehicle, &Asset::ElapsedTime),
            Quantity::new(REPAIR_TIME + 2)
        );
    }

    #[test]
    fn failed_travel_requirement_is_atomic() {
        let mut world = initial();
        let before = world.state_key();
        let error = world
            .apply(action(
                &world,
                RateId::Arrive(Route::DirectOut, TravelOutcome::Clear),
            ))
            .expect_err("vehicle has not departed or resolved Nature");
        assert!(matches!(error, axionomy::ApplyError::Infeasible { .. }));
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn package_identity_rejects_loading_from_the_wrong_order() {
        let world = initial();
        let rebound = Exchange::new(RateId::Load(OrderId::A), Quantity::new(1))
            .bind(Role::Vehicle, AccountId::Vehicle)
            .bind(Role::Order, AccountId::Order(OrderId::B));

        assert!(!world.is_applicable(&rebound));
    }
}
