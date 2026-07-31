//! Long-horizon stochastic logistics over core-validated rollouts.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate, Trace,
    basket,
};
use axionomy_search::{
    monte_carlo::{MonteCarloConfig, PolicyEstimate, ScalarStatistics, ScalarSummary, evaluate},
    rollout::{RolloutConfig, RolloutDecision, RolloutStop, TraceRetention, run_to_goal},
    sampling::{SeededSampler, TicketSource, WeightedExchange, sample},
};

const REFUEL_AMOUNT: u64 = 4;
const REPAIR_TIME: u64 = 2;
const ROLLOUT_HORIZON: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrderId {
    A,
    B,
    C,
    D,
}

pub const ORDERS: [OrderId; 4] = [OrderId::A, OrderId::B, OrderId::C, OrderId::D];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Location {
    Depot,
    Junction,
    Customer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Vehicle,
    Order(OrderId),
    Nature,
    FuelStation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Vehicle,
    Order,
    Nature,
    FuelStation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RateId {
    Load(OrderId),
    Depart(Route),
    Resolve(Route, TravelOutcome),
    Arrive(Route, TravelOutcome),
    Repair(Route),
    Deliver(OrderId),
    Refuel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Policy {
    Direct,
    Reliable,
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;

#[derive(Debug, Clone)]
pub struct MissionRollout {
    trace: Trace<RateId, Role, AccountId>,
    completed: bool,
    delivered: usize,
    elapsed_time: u64,
    steps: usize,
}

impl MissionRollout {
    pub const fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }

    pub const fn completed(&self) -> bool {
        self.completed
    }

    pub const fn delivered(&self) -> usize {
        self.delivered
    }

    pub const fn elapsed_time(&self) -> u64 {
        self.elapsed_time
    }

    pub const fn steps(&self) -> usize {
        self.steps
    }
}

#[derive(Debug, Clone)]
pub struct MonteCarloEstimate {
    estimates: Vec<PolicyEstimate<Policy, ScalarSummary>>,
    chosen: Policy,
}

impl MonteCarloEstimate {
    pub const fn chosen(&self) -> Policy {
        self.chosen
    }

    pub fn estimate(&self, policy: Policy) -> Option<&ScalarSummary> {
        self.estimates
            .iter()
            .find(|estimate| estimate.policy() == &policy)
            .map(PolicyEstimate::summary)
    }
}

pub fn initial() -> World {
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
        builder = builder
            .account(
                AccountId::Order(order),
                Account::from(basket([(Asset::Waiting, 1), (Asset::Package(order), 1)])),
            )
            .rate(
                RateId::Load(order),
                Rate::new()
                    .preserve(Role::Vehicle, basket([(Asset::At(Location::Depot), 1)]))
                    .consume(Role::Vehicle, basket([(Asset::CargoSpace, 1)]))
                    .produce(
                        Role::Vehicle,
                        basket([(Asset::CargoOccupied, 1), (Asset::Package(order), 1)]),
                    )
                    .consume(
                        Role::Order,
                        basket([(Asset::Waiting, 1), (Asset::Package(order), 1)]),
                    )
                    .produce(Role::Order, basket([(Asset::InTransit, 1)]))
                    .distinct(Role::Vehicle, Role::Order),
            )
            .rate(
                RateId::Deliver(order),
                Rate::new()
                    .preserve(Role::Vehicle, basket([(Asset::At(Location::Customer), 1)]))
                    .consume(
                        Role::Vehicle,
                        basket([(Asset::CargoOccupied, 1), (Asset::Package(order), 1)]),
                    )
                    .produce(Role::Vehicle, basket([(Asset::CargoSpace, 1)]))
                    .consume(Role::Order, basket([(Asset::InTransit, 1)]))
                    .produce(
                        Role::Order,
                        basket([(Asset::Delivered, 1), (Asset::Package(order), 1)]),
                    )
                    .distinct(Role::Vehicle, Role::Order),
            );
    }

    for route in ROUTES {
        builder = builder.rate(
            RateId::Depart(route),
            Rate::new()
                .consume(Role::Vehicle, basket([(Asset::At(route_source(route)), 1)]))
                .produce(Role::Vehicle, basket([(Asset::Traveling(route), 1)])),
        );

        for outcome in OUTCOMES {
            builder = builder.rate(
                RateId::Resolve(route, outcome),
                Rate::new()
                    .preserve(Role::Vehicle, basket([(Asset::Traveling(route), 1)]))
                    .consume(Role::Nature, basket([(Asset::WeatherReady, 1)]))
                    .preserve(
                        Role::Nature,
                        basket([(Asset::OutcomeWeight(route, outcome), 1)]),
                    )
                    .produce(Role::Nature, basket([(Asset::Outcome(route, outcome), 1)]))
                    .distinct(Role::Vehicle, Role::Nature),
            );
        }

        for outcome in [TravelOutcome::Clear, TravelOutcome::Delayed] {
            let (fuel, time) = travel_cost(route, outcome);
            builder = builder.rate(
                RateId::Arrive(route, outcome),
                Rate::new()
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
                    .produce(Role::Nature, basket([(Asset::WeatherReady, 1)]))
                    .distinct(Role::Vehicle, Role::Nature),
            );
        }

        builder = builder.rate(
            RateId::Repair(route),
            Rate::new()
                .preserve(
                    Role::Vehicle,
                    basket([(Asset::Traveling(route), 1), (Asset::RepairTool, 1)]),
                )
                .consume(Role::Vehicle, basket([(Asset::TimeRemaining, REPAIR_TIME)]))
                .produce(Role::Vehicle, basket([(Asset::ElapsedTime, REPAIR_TIME)]))
                .consume(
                    Role::Nature,
                    basket([(Asset::Outcome(route, TravelOutcome::Breakdown), 1)]),
                )
                .produce(Role::Nature, basket([(Asset::WeatherReady, 1)]))
                .distinct(Role::Vehicle, Role::Nature),
        );
    }

    builder
        .rate(
            RateId::Refuel,
            Rate::new()
                .preserve(Role::Vehicle, basket([(Asset::At(Location::Depot), 1)]))
                .consume(Role::Vehicle, basket([(Asset::Money, REFUEL_AMOUNT)]))
                .produce(Role::Vehicle, basket([(Asset::Fuel, REFUEL_AMOUNT)]))
                .consume(Role::FuelStation, basket([(Asset::Fuel, REFUEL_AMOUNT)]))
                .produce(Role::FuelStation, basket([(Asset::Money, REFUEL_AMOUNT)]))
                .distinct(Role::Vehicle, Role::FuelStation),
        )
        .invariant(
            [Location::Depot, Location::Junction, Location::Customer]
                .into_iter()
                .fold(
                    ROUTES.into_iter().fold(
                        LinearInvariant::new("vehicle position"),
                        |invariant, route| invariant.weight(Asset::Traveling(route), 1),
                    ),
                    |invariant, location| invariant.weight(Asset::At(location), 1),
                ),
        )
        .invariant(
            LinearInvariant::new("fuel accounting")
                .weight(Asset::Fuel, 1)
                .weight(Asset::SpentFuel, 1),
        )
        .invariant(LinearInvariant::new("money accounting").weight(Asset::Money, 1))
        .invariant(
            LinearInvariant::new("time accounting")
                .weight(Asset::TimeRemaining, 1)
                .weight(Asset::ElapsedTime, 1),
        )
        .invariant(
            LinearInvariant::new("cargo capacity")
                .weight(Asset::CargoSpace, 1)
                .weight(Asset::CargoOccupied, 1),
        )
        .invariant(
            LinearInvariant::new("order lifecycle")
                .weight(Asset::Waiting, 1)
                .weight(Asset::InTransit, 1)
                .weight(Asset::Delivered, 1),
        )
        .build()
}

pub fn goal() -> Goal<AccountId, Asset> {
    ORDERS.into_iter().fold(Goal::new(), |goal, order| {
        goal.require(AccountId::Order(order), basket([(Asset::Delivered, 1)]))
    })
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut rates = world.rate_ids().copied().collect::<Vec<_>>();
    rates.sort();
    world.applicable(rates.into_iter().map(action))
}

pub fn run_policy(model: &World, policy: Policy, exploration_seed: u64) -> MissionRollout {
    let mut sampler = SeededSampler::new(exploration_seed);
    let goal = goal();
    let result = run_to_goal(
        model,
        &goal,
        RolloutConfig::new(ROLLOUT_HORIZON).with_retention(TraceRetention::Trace),
        |world, _| match policy_action(world, policy, &mut sampler) {
            Some(exchange) => RolloutDecision::Propose(exchange),
            None => RolloutDecision::Stop(RolloutStop::NoProposal),
        },
    );
    MissionRollout {
        trace: result
            .trace()
            .cloned()
            .expect("mission rollouts retain traces"),
        completed: result.world().matches(&goal),
        delivered: delivered(result.world()),
        elapsed_time: result
            .world()
            .balance(&AccountId::Vehicle, &Asset::ElapsedTime)
            .get(),
        steps: result.steps(),
    }
}

pub fn monte_carlo(model: &World, samples: usize) -> Option<MonteCarloEstimate> {
    let estimates = evaluate(
        [Policy::Direct, Policy::Reliable],
        MonteCarloConfig::new(samples),
        |policy, sample_index| {
            let rollout = run_policy(
                model,
                *policy,
                u64::try_from(sample_index).unwrap_or(u64::MAX),
            );
            Ok::<_, std::convert::Infallible>(mission_utility(&rollout))
        },
        ScalarStatistics::new,
    )
    .ok()?;
    let chosen = estimates
        .iter()
        .max_by(|left, right| {
            left.summary()
                .mean()
                .total_cmp(&right.summary().mean())
                .then_with(|| right.policy().cmp(left.policy()))
        })?
        .policy()
        .to_owned();
    Some(MonteCarloEstimate { estimates, chosen })
}

fn policy_action(world: &World, policy: Policy, sampler: &mut impl TicketSource) -> Option<Action> {
    if let Some(route) = active_route(world) {
        if !world
            .balance(&AccountId::Nature, &Asset::WeatherReady)
            .is_zero()
        {
            let outcomes = nature_outcomes(world, route);
            return sample(&outcomes, sampler).ok().cloned();
        }
        if !world
            .balance(
                &AccountId::Nature,
                &Asset::Outcome(route, TravelOutcome::Breakdown),
            )
            .is_zero()
        {
            return Some(action(RateId::Repair(route)));
        }
        for outcome in [TravelOutcome::Clear, TravelOutcome::Delayed] {
            if !world
                .balance(&AccountId::Nature, &Asset::Outcome(route, outcome))
                .is_zero()
            {
                return Some(action(RateId::Arrive(route, outcome)));
            }
        }
        return None;
    }

    let carrying = carried_order(world);
    let location = current_location(world)?;
    match location {
        Location::Depot => {
            if carrying.is_some() {
                Some(action(RateId::Depart(match policy {
                    Policy::Direct => Route::DirectOut,
                    Policy::Reliable => Route::SafeOutFirst,
                })))
            } else if world.balance(&AccountId::Vehicle, &Asset::Fuel).get() < REFUEL_AMOUNT {
                Some(action(RateId::Refuel))
            } else {
                ORDERS.into_iter().find_map(|order| {
                    (!world
                        .balance(&AccountId::Order(order), &Asset::Waiting)
                        .is_zero())
                    .then(|| action(RateId::Load(order)))
                })
            }
        }
        Location::Customer => {
            if let Some(order) = carrying {
                Some(action(RateId::Deliver(order)))
            } else {
                Some(action(RateId::Depart(match policy {
                    Policy::Direct => Route::DirectBack,
                    Policy::Reliable => Route::SafeBackFirst,
                })))
            }
        }
        Location::Junction => Some(action(RateId::Depart(if carrying.is_some() {
            Route::SafeOutSecond
        } else {
            Route::SafeBackSecond
        }))),
    }
}

fn nature_outcomes(world: &World, route: Route) -> Vec<WeightedExchange<Action>> {
    OUTCOMES
        .into_iter()
        .filter_map(|outcome| {
            let exchange = action(RateId::Resolve(route, outcome));
            world.is_applicable(&exchange).then(|| {
                WeightedExchange::new(
                    exchange,
                    world
                        .balance(&AccountId::Nature, &Asset::OutcomeWeight(route, outcome))
                        .get(),
                )
            })
        })
        .collect()
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

fn action(rate: RateId) -> Action {
    let exchange = Exchange::new(rate, Quantity::new(1));
    match rate {
        RateId::Load(order) | RateId::Deliver(order) => exchange
            .bind(Role::Vehicle, AccountId::Vehicle)
            .bind(Role::Order, AccountId::Order(order)),
        RateId::Depart(_) => exchange.bind(Role::Vehicle, AccountId::Vehicle),
        RateId::Resolve(_, _) | RateId::Arrive(_, _) | RateId::Repair(_) => exchange
            .bind(Role::Vehicle, AccountId::Vehicle)
            .bind(Role::Nature, AccountId::Nature),
        RateId::Refuel => exchange
            .bind(Role::Vehicle, AccountId::Vehicle)
            .bind(Role::FuelStation, AccountId::FuelStation),
    }
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

fn active_route(world: &World) -> Option<Route> {
    ROUTES.into_iter().find(|route| {
        !world
            .balance(&AccountId::Vehicle, &Asset::Traveling(*route))
            .is_zero()
    })
}

fn current_location(world: &World) -> Option<Location> {
    [Location::Depot, Location::Junction, Location::Customer]
        .into_iter()
        .find(|location| {
            !world
                .balance(&AccountId::Vehicle, &Asset::At(*location))
                .is_zero()
        })
}

fn carried_order(world: &World) -> Option<OrderId> {
    ORDERS.into_iter().find(|order| {
        !world
            .balance(&AccountId::Vehicle, &Asset::Package(*order))
            .is_zero()
    })
}

fn delivered(world: &World) -> usize {
    ORDERS
        .into_iter()
        .filter(|order| {
            !world
                .balance(&AccountId::Order(*order), &Asset::Delivered)
                .is_zero()
        })
        .count()
}

fn mission_utility(rollout: &MissionRollout) -> f64 {
    if rollout.completed() {
        -(rollout.elapsed_time() as f64)
    } else {
        -1_000.0 + rollout.delivered() as f64 * 100.0 - rollout.elapsed_time() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_horizon_rollout_replays_with_explicit_nature_outcomes() {
        let model = initial();
        let rollout = run_policy(&model, Policy::Reliable, 7);

        assert!(rollout.completed());
        assert_eq!(rollout.delivered(), ORDERS.len());
        assert!(rollout.steps() >= 40);
        assert!(
            rollout
                .trace()
                .exchanges()
                .iter()
                .any(|exchange| matches!(exchange.rate(), RateId::Resolve(_, _)))
        );
        let replayed = model
            .replayed(rollout.trace())
            .expect("long mission trace must replay");
        assert!(replayed.matches(&goal()));
    }

    #[test]
    fn monte_carlo_prefers_reliable_routes_for_encoded_risk() {
        let estimate = monte_carlo(&initial(), 64).expect("mission has policies");
        let direct = estimate.estimate(Policy::Direct).expect("direct estimate");
        let reliable = estimate
            .estimate(Policy::Reliable)
            .expect("reliable estimate");

        assert_eq!(estimate.chosen(), Policy::Reliable);
        assert!(reliable.mean() > direct.mean());
        assert_eq!(direct.samples(), 64);
    }

    #[test]
    fn failed_travel_requirement_is_atomic() {
        let mut world = initial();
        let before = world.state_key();
        let error = world
            .apply(action(RateId::Arrive(
                Route::DirectOut,
                TravelOutcome::Clear,
            )))
            .expect_err("vehicle has not departed or resolved Nature");
        assert!(matches!(error, axionomy::ApplyError::Infeasible { .. }));
        assert_eq!(world.state_key(), before);
    }
}
