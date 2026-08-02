//! A partially observed rescue decision with an explicitly encoded Nature.

use axionomy::{
    Account, Basket, EconomicView, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant,
    Quantity, Rate, Trace, basket,
};
use axionomy_search::rollout::{
    RolloutConfig, RolloutDecision, RolloutStop, TraceRetention, run_to_goal,
};
use axionomy_search::{
    monte_carlo::{BernoulliStatistics, MonteCarloConfig, evaluate},
    sampling::{
        SeededSampler, WeightedExchange, choose_by_ticket, sample, systematic_ticket, total_weight,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Location {
    Base,
    North,
    South,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountId {
    Agent,
    Nature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Actor,
    Nature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    ObserveThenFollow,
    NorthWithoutObserving,
}

#[derive(Debug, Clone)]
pub struct Rollout {
    trace: Trace<RateId, Role, AccountId>,
    succeeded: bool,
    spent_energy: u64,
}

impl Rollout {
    pub fn trace(&self) -> &Trace<RateId, Role, AccountId> {
        &self.trace
    }

    pub fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub fn spent_energy(&self) -> u64 {
        self.spent_energy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyComparison {
    chosen: Policy,
    observe_successes: usize,
    direct_successes: usize,
    samples: usize,
}

impl PolicyComparison {
    pub fn chosen(&self) -> Policy {
        self.chosen
    }

    pub fn observe_successes(&self) -> usize {
        self.observe_successes
    }

    pub fn direct_successes(&self) -> usize {
        self.direct_successes
    }

    pub fn samples(&self) -> usize {
        self.samples
    }
}

pub type World = Economy<AccountId, Asset, RateId, Role>;
pub type Action = Exchange<RateId, Role, AccountId>;
pub type AgentView<'a> = EconomicView<'a, AccountId, Asset, RateId, Role>;

/// Builds one already-resolved deterministic world.
pub fn scenario(truth: Location, seed: u8) -> World {
    assert_rescue_location(truth);
    assert!(seed < 4, "the fixture has four sensor seeds");
    build(Account::from(basket([
        (Asset::Truth(truth), 1),
        (Asset::Seed(seed), 1),
    ])))
}

/// Builds an unresolved world from user-provided, core-encoded prior weights.
pub fn uncertain(prior: impl IntoIterator<Item = (Location, u8, u64)>) -> World {
    let mut nature = Basket::from([(Asset::Unresolved, Quantity::new(1))]);
    for (truth, seed, weight) in prior {
        assert_rescue_location(truth);
        assert!(seed < 4, "the fixture has four sensor seeds");
        nature.insert(Asset::ScenarioWeight(truth, seed), Quantity::new(weight));
    }
    build(Account::from(nature))
}

pub fn uniform_uncertain() -> World {
    uncertain(
        [Location::North, Location::South]
            .into_iter()
            .flat_map(|truth| (0..4).map(move |seed| (truth, seed, 1))),
    )
}

fn build(nature: Account<Asset>) -> World {
    let mut builder = EconomyBuilder::new()
        .account(
            AccountId::Agent,
            Account::from(basket([
                (Asset::At(Location::Base), 1),
                (Asset::Energy, 2),
                (Asset::Sensor, 1),
                (Asset::Planning, 1),
            ])),
        )
        .account(AccountId::Nature, nature);

    for encoded_truth in [Location::North, Location::South] {
        for encoded_seed in 0..4 {
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
            let next_seed = (encoded_seed + 1) % 4;
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

    for destination in [Location::North, Location::South] {
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
    }

    builder
        .rate(
            RateId::Finish,
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Rescued, 1)]))
                .produce(Role::Actor, basket([(Asset::Solved, 1)])),
        )
        .invariant(
            [Location::Base, Location::North, Location::South]
                .into_iter()
                .fold(
                    LinearInvariant::new("one agent position"),
                    |invariant, location| invariant.weight(Asset::At(location), 1),
                ),
        )
        .invariant(
            LinearInvariant::new("energy accounting")
                .weight(Asset::Energy, 1)
                .weight(Asset::SpentEnergy, 1),
        )
        .invariant(
            LinearInvariant::new("sensor accounting")
                .weight(Asset::Sensor, 1)
                .weight(Asset::UsedSensor, 1),
        )
        .invariant((0..4).fold(
            LinearInvariant::new("one nature seed state").weight(Asset::Unresolved, 1),
            |invariant, seed| invariant.weight(Asset::Seed(seed), 1),
        ))
        .invariant([Location::North, Location::South].into_iter().fold(
            LinearInvariant::new("one hidden truth state").weight(Asset::Unresolved, 1),
            |invariant, location| invariant.weight(Asset::Truth(location), 1),
        ))
        .invariant(
            LinearInvariant::new("rescue lifecycle")
                .weight(Asset::Planning, 1)
                .weight(Asset::AwaitingObservation, 1)
                .weight(Asset::Committed, 1)
                .weight(Asset::Rescued, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("rescue model is valid")
}

pub fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Agent, basket([(Asset::Solved, 1)]))
}

pub fn agent_view(world: &World) -> AgentView<'_> {
    world.view([AccountId::Agent])
}

pub fn candidates(world: &World) -> Vec<Action> {
    let mut ids: Vec<_> = world.rate_ids().copied().collect();
    ids.sort();
    world.applicable(ids.into_iter().map(action))
}

/// Nature resolves an observation request by selecting the only observation
/// whose encoded preconditions match its hidden truth and seed.
pub fn nature_observation(world: &World) -> Option<Action> {
    candidates(world)
        .into_iter()
        .find(|exchange| matches!(exchange.rate(), RateId::ResolveObservation { .. }))
}

pub fn instantiate(world: &World, truth: Location, seed: u8) -> Option<Action> {
    candidates(world)
        .into_iter()
        .find(|exchange| exchange.rate() == &RateId::Instantiate { truth, seed })
}

pub fn run_policy(world: World, policy: Policy) -> Rollout {
    let goal = goal();
    let result = run_to_goal(
        &world,
        &goal,
        RolloutConfig::new(5).with_retention(TraceRetention::Trace),
        |state, _| match policy_action(state, policy) {
            Some(exchange) => RolloutDecision::Propose(exchange),
            None => RolloutDecision::Stop(RolloutStop::NoProposal),
        },
    );
    let succeeded = result.world().matches(&goal);
    Rollout {
        spent_energy: result
            .world()
            .balance(&AccountId::Agent, &Asset::SpentEnergy)
            .get(),
        trace: result
            .trace()
            .cloned()
            .expect("rescue rollouts retain their trace"),
        succeeded,
    }
}

/// Runs a policy after one explicit Nature instantiation and returns a trace
/// that begins at the unresolved model.
pub fn run_sampled_policy(model: &World, sample: &Action, policy: Policy) -> Option<Rollout> {
    let mut branch = model.fork();
    branch.apply(sample.clone()).ok()?;
    let rollout = run_policy(branch, policy);
    let mut trace = Trace::new();
    trace.push(sample.clone());
    trace.extend(rollout.trace.into_exchanges());
    Some(Rollout {
        trace,
        succeeded: rollout.succeeded,
        spent_energy: rollout.spent_energy,
    })
}

/// Exhaustively evaluates every integer-weighted scenario encoded by Nature.
pub fn evaluate_scenarios(model: &World) -> Option<PolicyComparison> {
    let weighted_scenarios = encoded_scenarios(model);
    let total_weight = total_weight(&weighted_scenarios).ok()?;
    let samples = usize::try_from(total_weight).ok()?;
    let estimates = evaluate(
        [Policy::ObserveThenFollow, Policy::NorthWithoutObserving],
        MonteCarloConfig::new(samples),
        |policy, sample_index| {
            let ticket = systematic_ticket(sample_index, total_weight);
            let sample = choose_by_ticket(&weighted_scenarios, ticket).map_err(|_| ())?;
            run_sampled_policy(model, sample, *policy)
                .map(|rollout| rollout.succeeded())
                .ok_or(())
        },
        BernoulliStatistics::new,
    )
    .ok()?;
    comparison(estimates, samples)
}

/// Estimates both policies from reproducible random draws over Nature's prior.
pub fn monte_carlo(model: &World, samples: usize, seed: u64) -> Option<PolicyComparison> {
    let weighted_scenarios = encoded_scenarios(model);
    let estimates = evaluate(
        [Policy::ObserveThenFollow, Policy::NorthWithoutObserving],
        MonteCarloConfig::new(samples),
        |policy, sample_index| {
            let offset = u64::try_from(sample_index)
                .unwrap_or(u64::MAX)
                .wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let mut sampler = SeededSampler::new(seed.wrapping_add(offset));
            let sampled = sample(&weighted_scenarios, &mut sampler).map_err(|_| ())?;
            run_sampled_policy(model, sampled, *policy)
                .map(|rollout| rollout.succeeded())
                .ok_or(())
        },
        BernoulliStatistics::new,
    )
    .ok()?;
    comparison(estimates, samples)
}

fn comparison(
    estimates: Vec<
        axionomy_search::monte_carlo::PolicyEstimate<
            Policy,
            axionomy_search::monte_carlo::BernoulliSummary,
        >,
    >,
    samples: usize,
) -> Option<PolicyComparison> {
    let observe_successes = estimates
        .iter()
        .find(|estimate| estimate.policy() == &Policy::ObserveThenFollow)?
        .summary()
        .successes();
    let direct_successes = estimates
        .iter()
        .find(|estimate| estimate.policy() == &Policy::NorthWithoutObserving)?
        .summary()
        .successes();
    let chosen = if observe_successes >= direct_successes {
        Policy::ObserveThenFollow
    } else {
        Policy::NorthWithoutObserving
    };
    Some(PolicyComparison {
        chosen,
        observe_successes,
        direct_successes,
        samples,
    })
}

fn encoded_scenarios(model: &World) -> Vec<WeightedExchange<Action>> {
    let mut scenarios: Vec<_> = candidates(model)
        .into_iter()
        .filter_map(|exchange| match *exchange.rate() {
            RateId::Instantiate { truth, seed } => Some((
                exchange,
                model
                    .balance(&AccountId::Nature, &Asset::ScenarioWeight(truth, seed))
                    .get(),
            )),
            _ => None,
        })
        .filter(|(_, weight)| *weight > 0)
        .collect();
    scenarios.sort_by_key(|(exchange, _)| *exchange.rate());
    scenarios
        .into_iter()
        .map(|(exchange, weight)| WeightedExchange::new(exchange, weight))
        .collect()
}

fn policy_action(world: &World, policy: Policy) -> Option<Action> {
    if !world
        .balance(&AccountId::Agent, &Asset::AwaitingObservation)
        .is_zero()
    {
        return nature_observation(world);
    }
    public_policy_action(&agent_view(world), policy)
}

fn public_policy_action(view: &AgentView<'_>, policy: Policy) -> Option<Action> {
    if view_has(view, Asset::Rescued) {
        return Some(action(RateId::Finish));
    }

    for location in [Location::North, Location::South] {
        if view_has(view, Asset::At(location)) {
            return Some(action(RateId::Rescue { location }));
        }
    }

    if !view_has(view, Asset::At(Location::Base)) {
        return None;
    }

    if policy == Policy::ObserveThenFollow && view_has(view, Asset::Sensor) {
        return Some(action(RateId::BeginObserve));
    }

    let destination = match policy {
        Policy::NorthWithoutObserving => Location::North,
        Policy::ObserveThenFollow => [Location::North, Location::South]
            .into_iter()
            .find(|location| view_has(view, Asset::Belief(*location)))?,
    };
    Some(action(RateId::Move {
        from: Location::Base,
        to: destination,
    }))
}

fn view_has(view: &AgentView<'_>, asset: Asset) -> bool {
    view.balance(&AccountId::Agent, &asset)
        .is_some_and(|quantity| !quantity.is_zero())
}

fn signal(truth: Location, seed: u8) -> Location {
    if seed == 0 {
        match truth {
            Location::North => Location::South,
            Location::South => Location::North,
            Location::Base => unreachable!("base is not a rescue truth"),
        }
    } else {
        truth
    }
}

fn assert_rescue_location(location: Location) {
    assert!(
        matches!(location, Location::North | Location::South),
        "base is not a possible rescue truth"
    );
}

fn action(rate: RateId) -> Action {
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
        RateId::BeginObserve | RateId::Move { .. } => {
            Exchange::new(rate, Quantity::new(1)).bind(Role::Actor, AccountId::Agent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nature_is_hidden_but_its_observation_is_replayable() {
        let world = scenario(Location::South, 1);
        let view = agent_view(&world);
        assert!(view.account(&AccountId::Nature).is_none());
        assert_eq!(
            view.balance(&AccountId::Agent, &Asset::Energy),
            Some(Quantity::new(2))
        );

        let rollout = run_policy(world, Policy::ObserveThenFollow);
        assert!(rollout.succeeded());
        assert_eq!(rollout.spent_energy(), 2);
        assert_eq!(rollout.trace().exchanges()[0].rate(), &RateId::BeginObserve);
        assert!(matches!(
            rollout.trace().exchanges()[1].rate(),
            RateId::ResolveObservation { .. }
        ));

        let replay = scenario(Location::South, 1)
            .replayed(rollout.trace())
            .expect("sampled trace must replay deterministically");
        assert!(replay.matches(&goal()));
    }

    #[test]
    fn exact_scenario_evaluation_prefers_the_encoded_prior() {
        let model = uniform_uncertain();
        let estimate = evaluate_scenarios(&model).expect("prior has positive weight");
        assert_eq!(estimate.observe_successes(), 6);
        assert_eq!(estimate.direct_successes(), 4);
        assert_eq!(estimate.chosen(), Policy::ObserveThenFollow);
    }

    #[test]
    fn seeded_monte_carlo_is_reproducible() {
        let model = uniform_uncertain();
        assert_eq!(monte_carlo(&model, 64, 19), monte_carlo(&model, 64, 19));
    }

    #[test]
    fn hidden_nature_and_actor_roles_cannot_be_swapped() {
        let world = scenario(Location::South, 1);
        let mut awaiting = world.fork();
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
    fn public_observation_intent_does_not_depend_on_hidden_truth() {
        let north = scenario(Location::North, 1);
        let south = scenario(Location::South, 0);
        let north_view = agent_view(&north);
        let south_view = agent_view(&south);

        assert_eq!(north_view.observation_key(), south_view.observation_key());
        assert_eq!(
            public_policy_action(&north_view, Policy::ObserveThenFollow),
            Some(action(RateId::BeginObserve))
        );
        assert_eq!(
            public_policy_action(&south_view, Policy::ObserveThenFollow),
            Some(action(RateId::BeginObserve))
        );
    }

    #[test]
    fn sampled_trace_replays_from_the_uncertain_model() {
        let model = uniform_uncertain();
        let sample = instantiate(&model, Location::South, 1).expect("scenario is encoded");
        let rollout = run_sampled_policy(&model, &sample, Policy::ObserveThenFollow)
            .expect("scenario can be instantiated");
        assert!(rollout.succeeded());

        let replay = model
            .replayed(rollout.trace())
            .expect("sample and policy must replay together");
        assert!(replay.matches(&goal()));
        assert!(candidates(&replay).is_empty());
    }
}
