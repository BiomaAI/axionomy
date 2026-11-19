#![doc = include_str!("README.md")]

use axionomy::{Economy, Exchange, Goal, Quantity, basket};
pub mod fleet;
#[cfg(feature = "packages")]
pub mod package;

#[cfg(test)]
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
    crate::fixtures::variant(axionomy::path!("logistics/micro"), |instance| {
        for order in ORDERS {
            crate::fixtures::set(
                instance,
                AccountId::Order(order),
                [(Asset::Waiting, 1), (Asset::Delivered, 0)],
            );
        }
    })
}

/// One remaining delivery for fast transport and oracle checks; the other
/// orders are already represented as completed economic state.
pub fn initial_micro() -> World {
    crate::fixtures::initial(axionomy::path!("logistics/micro"))
}

pub fn goal() -> Goal<Ontology> {
    ORDERS.into_iter().fold(Goal::new(), |goal, order| {
        goal.require(AccountId::Order(order), basket([(Asset::Delivered, 1)]))
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
