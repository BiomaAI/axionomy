//! Long-horizon stochastic logistics over core-validated rollouts.

use axionomy::path;
use axionomy::{
    Account, Apply, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    Trace, basket,
};
use axionomy_search::{
    action_source::eager_actions,
    mcts::{MctsConfig, MctsDecision, MctsProgress, MctsSession, search},
    monte_carlo::{
        MonteCarloConfig, MonteCarloProgress, MonteCarloSession, MonteCarloStatus, PolicyEstimate,
        ScalarStatistics, ScalarSummary, VectorStatistics, VectorSummary, evaluate,
    },
    pareto::{FrontierCompleteness, Objective, ObjectiveVector, ParetoFront},
    rollout::{Episodes, RolloutConfig, RolloutDecision, RolloutStop, TraceRetention, run_to_goal},
    sampling::{SeededSampler, TicketSource, WeightedExchange, sample},
    session::{Continue, WorkBudget},
    tree::{TreeSearchError, TreeStatus},
};
use std::ops::ControlFlow;
pub mod fleet;

const REFUEL_AMOUNT: u64 = 4;
const REPAIR_TIME: u64 = 2;
const ROLLOUT_HORIZON: usize = 120;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Policy {
    Direct,
    Reliable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskCriterion {
    Mean,
    LowerDecile,
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
pub type PlanningDecision = MctsDecision<Action>;
pub type PlanningError = TreeSearchError<Ontology>;
pub type PolicyFront = ParetoFront<PolicyEstimate<Policy, VectorSummary>, PolicyObjective, f64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyObjective {
    CompletionProbability,
    MeanDelivered,
    MeanElapsedTime,
}

#[derive(Debug, Clone)]
pub struct MissionRollout {
    trace: Trace<Ontology>,
    completed: bool,
    delivered: usize,
    elapsed_time: u64,
    steps: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningStop {
    DeliveredAllOrders,
    NoFeasibleAction,
    Horizon,
}

#[derive(Debug, Clone)]
pub struct ReplanningStep {
    pub trace_index: usize,
    pub decision: PlanningDecision,
}

#[derive(Debug, Clone)]
pub struct PlannedLogistics {
    pub rollout: MissionRollout,
    pub decisions: Vec<ReplanningStep>,
    pub stop: PlanningStop,
}

impl MissionRollout {
    pub const fn trace(&self) -> &Trace<Ontology> {
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
    build(&ORDERS)
}

/// One remaining delivery for fast transport and oracle checks; the other
/// orders are already represented as completed economic state.
pub fn initial_micro() -> World {
    build(&[OrderId::A])
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
        builder = builder
            .account(AccountId::Order(order), Account::from(order_state))
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

pub fn candidates(world: &World) -> Vec<Action> {
    let mut rates = world.rate_ids().copied().collect::<Vec<_>>();
    rates.sort();
    world.applicable(rates.into_iter().map(action))
}

/// Runs a policy on `world`, which is left where the rollout ended.
pub fn run_policy(
    world: &mut impl Apply<Ontology>,
    policy: Policy,
    exploration_seed: u64,
) -> MissionRollout {
    let mut sampler = SeededSampler::new(exploration_seed);
    let goal = goal();
    let result = run_to_goal(
        world,
        &goal,
        RolloutConfig::new(ROLLOUT_HORIZON).with_retention(TraceRetention::Trace),
        |world, _| match policy_action(world, policy, &mut sampler) {
            Some(exchange) => RolloutDecision::Propose(exchange),
            None => RolloutDecision::Stop(RolloutStop::NoProposal),
        },
    );
    let end = world.economy();
    MissionRollout {
        trace: result
            .trace()
            .cloned()
            .expect("mission rollouts retain traces"),
        completed: end.matches(&goal),
        delivered: delivered(end),
        elapsed_time: end
            .balance(&AccountId::Vehicle, &Asset::ElapsedTime)
            .to_u64(),
        steps: result.steps(),
    }
}

/// Sample `index` of `policy` from the model, stepped where `episodes` keeps
/// it; the sample index is its exploration seed.
fn episode(
    model: &World,
    policy: &Policy,
    index: usize,
    episodes: &mut impl Episodes<Ontology, Policy, MissionRollout>,
) -> MissionRollout {
    let mut world = episodes.begin(policy, index, model.clone());
    let rollout = run_policy(
        &mut world,
        *policy,
        u64::try_from(index).unwrap_or(u64::MAX),
    );
    episodes.end(policy, index, world, &rollout);
    rollout
}

pub fn monte_carlo(
    model: &World,
    samples: usize,
    episodes: &mut impl Episodes<Ontology, Policy, MissionRollout>,
) -> Option<MonteCarloEstimate> {
    monte_carlo_with_risk(model, samples, RiskCriterion::Mean, episodes)
}

pub fn monte_carlo_with_risk(
    model: &World,
    samples: usize,
    criterion: RiskCriterion,
    episodes: &mut impl Episodes<Ontology, Policy, MissionRollout>,
) -> Option<MonteCarloEstimate> {
    let estimates = evaluate(
        [Policy::Direct, Policy::Reliable],
        MonteCarloConfig::new(samples),
        |policy, sample_index| {
            let rollout = episode(model, policy, sample_index, episodes);
            Ok::<_, std::convert::Infallible>(mission_utility(&rollout))
        },
        ScalarStatistics::new,
    )
    .ok()?;
    monte_carlo_estimate(estimates, criterion)
}

pub fn monte_carlo_with_risk_progress(
    model: &World,
    samples: usize,
    criterion: RiskCriterion,
    episodes: &mut impl Episodes<Ontology, Policy, MissionRollout>,
    chunk_size: usize,
    mut observer: impl FnMut(MonteCarloProgress) -> ControlFlow<()>,
) -> Option<MonteCarloEstimate> {
    let mut session = MonteCarloSession::new(
        [Policy::Direct, Policy::Reliable],
        MonteCarloConfig::new(samples),
        |policy: &Policy, sample_index| {
            let rollout = episode(model, policy, sample_index, episodes);
            Ok::<_, std::convert::Infallible>(mission_utility(&rollout))
        },
        ScalarStatistics::new,
    );
    while session.status() == MonteCarloStatus::Running {
        let report = session
            .advance(WorkBudget::new(chunk_size.max(1)), &mut Continue)
            .ok()?;
        if observer(*report.progress()).is_break() {
            return None;
        }
    }
    monte_carlo_estimate(session.into_estimates()?, criterion)
}

fn monte_carlo_estimate(
    estimates: Vec<PolicyEstimate<Policy, ScalarSummary>>,
    criterion: RiskCriterion,
) -> Option<MonteCarloEstimate> {
    let chosen = estimates
        .iter()
        .max_by(|left, right| {
            risk_score(left.summary(), criterion)
                .total_cmp(&risk_score(right.summary(), criterion))
                .then_with(|| right.policy().cmp(left.policy()))
        })?
        .policy()
        .to_owned();
    Some(MonteCarloEstimate { estimates, chosen })
}

/// Estimates a non-dominated policy set from sampled, core-validated rollouts.
/// Statistical sampling means this front is intentionally never labeled exact.
pub fn policy_front(
    model: &World,
    samples: usize,
    episodes: &mut impl Episodes<Ontology, Policy, MissionRollout>,
) -> Option<PolicyFront> {
    if samples == 0 {
        return None;
    }
    let estimates = evaluate(
        [Policy::Direct, Policy::Reliable],
        MonteCarloConfig::new(samples),
        |policy, sample_index| {
            let rollout = episode(model, policy, sample_index, episodes);
            Ok::<_, std::convert::Infallible>(vec![
                f64::from(rollout.completed()),
                rollout.delivered() as f64,
                rollout.elapsed_time() as f64,
            ])
        },
        || VectorStatistics::new(3),
    )
    .ok()?;

    policy_front_from(estimates)
}

pub fn policy_front_with_progress(
    model: &World,
    samples: usize,
    episodes: &mut impl Episodes<Ontology, Policy, MissionRollout>,
    chunk_size: usize,
    mut observer: impl FnMut(MonteCarloProgress) -> ControlFlow<()>,
) -> Option<PolicyFront> {
    if samples == 0 {
        return None;
    }
    let mut session = MonteCarloSession::new(
        [Policy::Direct, Policy::Reliable],
        MonteCarloConfig::new(samples),
        |policy: &Policy, sample_index| {
            let rollout = episode(model, policy, sample_index, episodes);
            Ok::<_, std::convert::Infallible>(vec![
                f64::from(rollout.completed()),
                rollout.delivered() as f64,
                rollout.elapsed_time() as f64,
            ])
        },
        || VectorStatistics::new(3),
    );
    while session.status() == MonteCarloStatus::Running {
        let report = session
            .advance(WorkBudget::new(chunk_size.max(1)), &mut Continue)
            .ok()?;
        if observer(*report.progress()).is_break() {
            return None;
        }
    }
    policy_front_from(session.into_estimates()?)
}

fn policy_front_from(estimates: Vec<PolicyEstimate<Policy, VectorSummary>>) -> Option<PolicyFront> {
    let mut front = ParetoFront::approximate();
    for estimate in estimates {
        let objectives = policy_objectives(estimate.summary())?;
        front.insert(estimate, objectives).ok()?;
    }
    debug_assert_eq!(front.completeness(), FrontierCompleteness::Approximate);
    Some(front)
}

pub fn policy_objectives(summary: &VectorSummary) -> Option<ObjectiveVector<PolicyObjective, f64>> {
    let dimensions = summary.dimensions();
    (dimensions.len() == 3).then_some(())?;
    ObjectiveVector::try_new([
        Objective::maximize(PolicyObjective::CompletionProbability, dimensions[0].mean()),
        Objective::maximize(PolicyObjective::MeanDelivered, dimensions[1].mean()),
        Objective::minimize(PolicyObjective::MeanElapsedTime, dimensions[2].mean()),
    ])
    .ok()
}

/// Plans one live decision with MCTS over encoded routes and Nature outcomes.
///
/// The tree is disposable: its edges are ordinary exchanges, chance weights
/// come from Nature's assets, and the selected action is revalidated against
/// the supplied economy by `axionomy-search` before it is returned.
pub fn plan_action(
    world: &World,
    iterations: usize,
    seed: u64,
) -> Result<PlanningDecision, PlanningError> {
    search(
        world,
        MctsConfig::new(iterations, ROLLOUT_HORIZON).with_seed(seed),
        1,
        planning_candidates,
        planning_outcomes,
        |_| 0,
        |state| {
            state
                .matches(&goal())
                .then(|| vec![planning_utility(state)])
        },
        |state| vec![planning_utility(state)],
        |state, actions, random| {
            policy_action(state, Policy::Reliable, random)
                .filter(|preferred| actions.contains(preferred))
                .or_else(|| actions.first().cloned())
        },
    )
}

pub fn plan_action_with_progress(
    world: &World,
    iterations: usize,
    seed: u64,
    chunk_size: usize,
    mut observer: impl FnMut(MctsProgress) -> ControlFlow<()>,
) -> Result<Option<PlanningDecision>, PlanningError> {
    let config = MctsConfig::new(iterations, ROLLOUT_HORIZON).with_seed(seed);
    let mut session = MctsSession::new(
        world,
        config,
        1,
        eager_actions(planning_candidates),
        planning_outcomes,
        |_| 0,
        |state| {
            state
                .matches(&goal())
                .then(|| vec![planning_utility(state)])
        },
        |state| vec![planning_utility(state)],
        |state, actions, random| {
            policy_action(state, Policy::Reliable, random)
                .filter(|preferred| actions.contains(preferred))
                .or_else(|| actions.first().cloned())
        },
    )?;
    while session.status() == TreeStatus::Running {
        let report = session.advance(WorkBudget::new(chunk_size.max(1)), &mut Continue)?;
        if observer(*report.progress()).is_break() {
            return Ok(None);
        }
    }
    Ok(session.into_decision())
}

/// Executes a receding-horizon planner through delivery or an explicit failure.
/// Search consumes a separate random stream from actual Nature events, so a
/// larger planning budget cannot itself change the realized weather sequence.
pub fn run_planned_with_progress(
    initial: &World,
    iterations: usize,
    environment_seed: u64,
    planning_seed: u64,
    chunk_size: usize,
    mut observer: impl FnMut(usize, MctsProgress) -> ControlFlow<()>,
) -> Result<Option<PlannedLogistics>, String> {
    let mut world = initial.clone();
    let mut trace = Trace::new();
    let mut decisions = Vec::new();
    let mut nature = SeededSampler::new(environment_seed);
    let stop = loop {
        if world.matches(&goal()) {
            break PlanningStop::DeliveredAllOrders;
        }
        if trace.exchanges().len() >= ROLLOUT_HORIZON {
            break PlanningStop::Horizon;
        }
        let outcomes = planning_outcomes(&world);
        let action = if !outcomes.is_empty() {
            sample(&outcomes, &mut nature)
                .map_err(|error| format!("Nature sampling: {error:?}"))?
                .clone()
        } else {
            let actions = planning_candidates(&world);
            if actions.is_empty() {
                break PlanningStop::NoFeasibleAction;
            }
            if actions.len() == 1 {
                actions[0].clone()
            } else {
                let index = decisions.len();
                let decision = plan_action_with_progress(
                    &world,
                    iterations.max(1),
                    planning_seed.wrapping_add(index as u64),
                    chunk_size,
                    |state| observer(index, state),
                )
                .map_err(|error| format!("logistics planning: {error:?}"))?;
                let Some(decision) = decision else {
                    return Ok(None);
                };
                let action = decision.action().clone();
                decisions.push(ReplanningStep {
                    trace_index: trace.exchanges().len(),
                    decision,
                });
                action
            }
        };
        world
            .apply(action.clone())
            .map_err(|error| format!("logistics execution: {error:?}"))?;
        trace.push(action);
    };
    let replayed = initial
        .replayed(&trace)
        .map_err(|error| format!("logistics replay: {error:?}"))?;
    if replayed.state_key() != world.state_key() {
        return Err("logistics replay diverged".into());
    }
    let rollout = MissionRollout {
        completed: world.matches(&goal()),
        delivered: delivered(&world),
        elapsed_time: world
            .balance(&AccountId::Vehicle, &Asset::ElapsedTime)
            .to_u64(),
        steps: trace.exchanges().len(),
        trace,
    };
    Ok(Some(PlannedLogistics {
        rollout,
        decisions,
        stop,
    }))
}

fn planning_candidates(world: &World) -> Vec<Action> {
    // Derive a small proposal set from encoded state, then assess every binding.
    // Scanning every concrete rate at every rollout step dominated full journeys.
    let rates = if let Some(route) = active_route(world) {
        if !world
            .balance(&AccountId::Nature, &Asset::WeatherReady)
            .is_zero()
        {
            Vec::new()
        } else if !world
            .balance(
                &AccountId::Nature,
                &Asset::Outcome(route, TravelOutcome::Breakdown),
            )
            .is_zero()
        {
            vec![RateId::Repair(route)]
        } else {
            [TravelOutcome::Clear, TravelOutcome::Delayed]
                .into_iter()
                .filter(|outcome| {
                    !world
                        .balance(&AccountId::Nature, &Asset::Outcome(route, *outcome))
                        .is_zero()
                })
                .map(|outcome| RateId::Arrive(route, outcome))
                .collect()
        }
    } else {
        let carrying = carried_order(world);
        match current_location(world) {
            Some(Location::Depot) => {
                let mut rates = if carrying.is_some() {
                    vec![
                        RateId::Depart(Route::DirectOut),
                        RateId::Depart(Route::SafeOutFirst),
                    ]
                } else {
                    ORDERS
                        .into_iter()
                        .filter(|order| {
                            !world
                                .balance(&AccountId::Order(*order), &Asset::Waiting)
                                .is_zero()
                        })
                        .map(RateId::Load)
                        .collect()
                };
                if world.balance(&AccountId::Vehicle, &Asset::Fuel).to_u64() < REFUEL_AMOUNT {
                    rates.push(RateId::Refuel);
                }
                rates
            }
            Some(Location::Customer) => carrying.map_or_else(
                || {
                    vec![
                        RateId::Depart(Route::DirectBack),
                        RateId::Depart(Route::SafeBackFirst),
                    ]
                },
                |order| vec![RateId::Deliver(order)],
            ),
            Some(Location::Junction) => vec![RateId::Depart(if carrying.is_some() {
                Route::SafeOutSecond
            } else {
                Route::SafeBackSecond
            })],
            None => Vec::new(),
        }
    };
    world.applicable(rates.into_iter().map(action))
}

fn planning_outcomes(world: &World) -> Vec<WeightedExchange<Action>> {
    active_route(world).map_or_else(Vec::new, |route| nature_outcomes(world, route))
}

fn planning_utility(world: &World) -> f64 {
    let completion_bonus = if world.matches(&goal()) { 1_000.0 } else { 0.0 };
    completion_bonus + delivered(world) as f64 * 100.0
        - world
            .balance(&AccountId::Vehicle, &Asset::ElapsedTime)
            .to_u64() as f64
}

fn risk_score(summary: &ScalarSummary, criterion: RiskCriterion) -> f64 {
    match criterion {
        RiskCriterion::Mean => summary.mean(),
        RiskCriterion::LowerDecile => summary.lower_tail_mean(0.1).unwrap_or(f64::NEG_INFINITY),
    }
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
            } else if world.balance(&AccountId::Vehicle, &Asset::Fuel).to_u64() < REFUEL_AMOUNT {
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
    if world
        .balance(&AccountId::Nature, &Asset::WeatherReady)
        .is_zero()
    {
        return Vec::new();
    }
    OUTCOMES
        .into_iter()
        .filter_map(|outcome| {
            let exchange = action(RateId::Resolve(route, outcome));
            world.is_applicable(&exchange).then(|| {
                WeightedExchange::new(
                    exchange,
                    world
                        .balance(&AccountId::Nature, &Asset::OutcomeWeight(route, outcome))
                        .to_u64(),
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
    use axionomy_search::rollout::Unrecorded;

    #[test]
    fn long_horizon_rollout_replays_with_explicit_nature_outcomes() {
        let model = initial();
        let rollout = run_policy(&mut model.clone(), Policy::Reliable, 7);

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
        let estimate = monte_carlo(&initial(), 64, &mut Unrecorded).expect("mission has policies");
        let direct = estimate.estimate(Policy::Direct).expect("direct estimate");
        let reliable = estimate
            .estimate(Policy::Reliable)
            .expect("reliable estimate");

        assert_eq!(estimate.chosen(), Policy::Reliable);
        assert!(reliable.mean() > direct.mean());
        assert_eq!(direct.samples(), 64);

        let tail =
            monte_carlo_with_risk(&initial(), 64, RiskCriterion::LowerDecile, &mut Unrecorded)
                .expect("mission has policies");
        assert_eq!(tail.chosen(), Policy::Reliable);
    }

    #[test]
    fn sampled_policy_front_is_explicitly_approximate_and_self_consistent() {
        let front =
            policy_front(&initial(), 64, &mut Unrecorded).expect("policies have sampled outcomes");
        assert_eq!(front.completeness(), FrontierCompleteness::Approximate);
        assert!(!front.is_empty());
        for entry in front.entries() {
            assert_eq!(entry.payload().summary().dimensions()[0].samples(), 64);
            assert_eq!(
                &policy_objectives(entry.payload().summary()).unwrap(),
                entry.objectives()
            );
        }
    }

    #[test]
    fn mcts_plans_an_applicable_route_from_encoded_chance() {
        let mut world = initial();
        world
            .apply(action(RateId::Load(OrderId::A)))
            .expect("first package loads");

        let decision = plan_action(&world, 64, 31).expect("loaded vehicle has a plan");
        assert!(world.is_applicable(decision.action()));
        assert!(matches!(
            decision.action().rate(),
            RateId::Depart(Route::DirectOut | Route::SafeOutFirst)
        ));
    }

    #[test]
    fn repeated_planning_executes_through_delivery_and_replays_every_choice() {
        let initial = initial();
        let mut saw_delivery = false;
        let mut saw_replanning_after_breakdown = false;
        for seed in [0, 7, 17] {
            let planned = run_planned_with_progress(&initial, 32, seed, 31, 8, |_, _| {
                ControlFlow::Continue(())
            })
            .unwrap()
            .unwrap();
            let trace = planned.rollout.trace();
            let replayed = initial.replayed(trace).unwrap();
            assert!(trace.exchanges().len() > 2);
            assert!(planned.decisions.len() > 1);
            assert_eq!(delivered(&replayed), planned.rollout.delivered());
            assert_eq!(replayed.matches(&goal()), planned.rollout.completed());
            saw_delivery |= planned.rollout.delivered() > 0;
            for step in &planned.decisions {
                assert_eq!(&trace.exchanges()[step.trace_index], step.decision.action());
                saw_replanning_after_breakdown |= trace.exchanges()[..step.trace_index]
                    .iter()
                    .any(|action| matches!(action.rate(), RateId::Repair(_)));
            }
            match planned.stop {
                PlanningStop::DeliveredAllOrders => assert!(replayed.matches(&goal())),
                PlanningStop::NoFeasibleAction => assert!(
                    planning_candidates(&replayed).is_empty()
                        && planning_outcomes(&replayed).is_empty()
                ),
                PlanningStop::Horizon => assert_eq!(trace.exchanges().len(), ROLLOUT_HORIZON),
            }
        }
        assert!(saw_delivery && saw_replanning_after_breakdown);
    }

    #[test]
    fn repeated_planning_cancels_without_mutating_the_source() {
        let initial = initial();
        let before = initial.state_key();
        let result =
            run_planned_with_progress(&initial, 32, 17, 31, 8, |_, _| ControlFlow::Break(()))
                .unwrap();
        assert!(result.is_none());
        assert_eq!(initial.state_key(), before);
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

    #[test]
    fn package_identity_rejects_loading_from_the_wrong_order() {
        let world = initial();
        let rebound = Exchange::new(RateId::Load(OrderId::A), Quantity::new(1))
            .bind(Role::Vehicle, AccountId::Vehicle)
            .bind(Role::Order, AccountId::Order(OrderId::B));

        assert!(!world.is_applicable(&rebound));
    }
}
