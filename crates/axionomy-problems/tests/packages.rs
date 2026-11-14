//! The samples as packages (P9): each package, loaded alone into a fresh
//! store, runs experiments that reproduce what its sample's Rust code finds,
//! the traces of its solvers, policies and plans and their objective values,
//! exactly, at every instance.
#![cfg(feature = "packages")]

use axionomy::{Economy, Ontology, Quantity, Trace, path, paths};
use axionomy_engine::{ArmResult, Effort, Ended, Engine, ObjectivePath, Results, Version};
use axionomy_problems::{
    amm, bridge, connect_four, exact_cover, lending, logistics, marketplace, maze, mission,
    perishables, rescue, scheduling, sokoban, work_league, workshop,
};
use axionomy_search::{
    mcts::MctsConfig,
    pareto::{ObjectiveDirection, ParetoSearchResult},
    rollout::{RolloutResult, Unrecorded},
};
use axionomy_store::{Checkpoints, Recordable, Store};
use serde::{Serialize, de::DeserializeOwned};
use std::ops::ControlFlow;

/// The seed the samples' outcomes are found with, and the effort they are
/// given where their Rust code takes one: a planner's iterations, a Monte
/// Carlo's samples. The packages ship the same as their experiments'.
const SEED: u64 = 17;
const EFFORT: usize = 128;

/// The instances every sample comes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Size {
    Micro,
    Showcase,
    Stress,
}

impl Size {
    fn of(instance: &str) -> Self {
        match instance {
            "micro" => Self::Micro,
            "showcase" => Self::Showcase,
            "stress" => Self::Stress,
            other => panic!("no instance {other}"),
        }
    }
}

/// One outcome of a sample's Rust code: the run a strategy of the sample
/// makes, or another trace it keeps, such as a legal but losing plan,
/// written as paths, and its value on each objective it is measured by,
/// keyed as the package's objective path ends.
struct Outcome {
    name: &'static str,
    trace: Vec<paths::Exchange>,
    values: Vec<(&'static str, Quantity)>,
}

/// The outcome `name` of `trace`, valued as `values`.
fn outcome<O: Ontology>(
    name: &'static str,
    trace: &Trace<O>,
    values: Vec<(&'static str, Quantity)>,
) -> Outcome
where
    axionomy::Exchange<O>: Serialize,
{
    Outcome {
        name,
        trace: trace.exchanges().iter().map(written).collect(),
        values,
    }
}

/// `value`, a typed identifier, exchange or economy, as paths.
fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

/// Each of `values` with its key, as quantities.
fn valued<const N: usize>(values: [(&'static str, u64); N]) -> Vec<(&'static str, Quantity)> {
    values
        .into_iter()
        .map(|(key, value)| (key, Quantity::from(value)))
        .collect()
}

/// The trace an exact front keeps for the best value of `first`, ties broken
/// by the best value of `second`: how the samples take a front's end.
fn best_on<O: Ontology, K: Eq>(
    result: &ParetoSearchResult<O, K, u64>,
    first: (K, ObjectiveDirection),
    second: (K, ObjectiveDirection),
) -> Trace<O> {
    // Smaller ranks first: a maximized value counts down from the largest.
    let rank = |entry: &axionomy_search::pareto::ParetoEntry<Trace<O>, K, u64>,
                (key, direction): &(K, ObjectiveDirection)| {
        let value = front_value(entry, key);
        match direction {
            ObjectiveDirection::Minimize => value,
            ObjectiveDirection::Maximize => u64::MAX - value,
        }
    };
    result
        .front()
        .entries()
        .iter()
        .min_by_key(|entry| (rank(entry, &first), rank(entry, &second)))
        .expect("a front with a point")
        .payload()
        .clone()
}

/// A front entry's value on `key`.
fn front_value<T, K: Eq>(entry: &axionomy_search::pareto::ParetoEntry<T, K, u64>, key: &K) -> u64 {
    entry
        .objectives()
        .objectives()
        .iter()
        .find(|objective| objective.key() == key)
        .map_or(0, |objective| *objective.value())
}

/// A rollout from `initial`, which must reach its goal: its trace and where
/// it ended.
fn simulated<O: Ontology>(
    initial: &Economy<O>,
    run: impl FnOnce(&mut Economy<O>) -> RolloutResult<O>,
) -> (Trace<O>, Economy<O>) {
    let mut world = initial.clone();
    let result = run(&mut world);
    assert!(
        result.termination().is_terminal(),
        "a rollout ended {:?}",
        result.termination()
    );
    (
        result.trace().cloned().expect("a rollout keeps its trace"),
        world,
    )
}

/// `write`'s package, exported from the store it was written in and
/// loaded alone into a fresh one.
fn loaded(name: &str, write: axionomy_problems::WritePackage) -> Engine {
    let source = Engine::new(Store::memory().unwrap()).unwrap();
    write(&source).unwrap();
    let name = name.parse().unwrap();
    let file = Store::memory().unwrap();
    source
        .export(
            &name,
            "0.1.0".parse::<Version>().unwrap(),
            source.named_under(&name).unwrap(),
            &file,
        )
        .unwrap();
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine.load(&file).unwrap();
    engine
}

/// Where the objective `key` an outcome is valued by is among
/// `objectives`: the one whose path ends in the key, such as `maze/energy`
/// for `energy` or `bridge/priority/a` for `priority/a`.
fn objective_named(objectives: &[ObjectivePath], key: &str) -> Option<usize> {
    objectives
        .iter()
        .position(|objective| objective.as_str().ends_with(&format!("/{key}")))
}

/// The arm of `results` running the strategy named `strategy`: its last
/// segment, since an ontology of a problem's own names its strategies
/// under its own path, as `rescue/incident/direct-north`.
fn arm<'a>(results: &'a Results, strategy: &str) -> &'a ArmResult {
    results
        .arms
        .iter()
        .find(|arm| last(arm.arm.strategy.as_str()) == strategy)
        .unwrap_or_else(|| panic!("no arm runs {strategy}"))
}

/// The last segment of a path.
fn last(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The outcome named `name`, if the sample has one at this instance.
fn find<'a>(outcomes: &'a [Outcome], name: &str) -> Option<&'a Outcome> {
    outcomes.iter().find(|outcome| outcome.name == name)
}

/// The package's experiment at `problem/instance` reproduces each of
/// `strategies` the sample's Rust code has an outcome for: the arm running
/// the strategy of that name reaches the outcome's exchanges, and its value
/// on each objective the outcome is valued by, the experiment's objective
/// of that name. Returns how many outcomes it compared.
fn reproduces(
    engine: &Engine,
    problem: &str,
    instance: &str,
    outcomes: &[Outcome],
    strategies: &[&str],
) -> usize {
    reproduced_by(
        engine,
        &format!("{problem}/{instance}"),
        problem,
        instance,
        outcomes,
        strategies,
    )
}

/// What [`reproduces`] checks, of the experiment at `experiment`, which
/// starts at the instance.
fn reproduced_by(
    engine: &Engine,
    experiment: &str,
    problem: &str,
    instance: &str,
    outcomes: &[Outcome],
    strategies: &[&str],
) -> usize {
    let mut compared = 0;
    let experiment = experiment.parse().unwrap();
    let objectives = engine.experiment(&experiment).unwrap().objectives;
    let results = engine
        .run_experiment(&experiment, &mut |_, _| ControlFlow::Continue(()))
        .unwrap();
    for strategy in strategies {
        // An outcome some instances have, such as revenue where there are
        // customers.
        let Some(outcome) = find(outcomes, strategy) else {
            continue;
        };
        let trial = &arm(&results, strategy).trials[0];
        let (_, exchanges) = engine.history(trial.end).unwrap();
        assert_eq!(
            exchanges, outcome.trace,
            "{problem} {instance} {strategy}: the trace"
        );
        for (key, value) in &outcome.values {
            let index = objective_named(&objectives, key)
                .unwrap_or_else(|| panic!("{problem} measures no `{key}`"));
            assert_eq!(
                &trial.values[index], value,
                "{problem} {instance} {strategy}: {key}"
            );
        }
        compared += 1;
    }
    compared
}

/// `economy` as the engine's store reads it back, as paths.
fn kept<O: Recordable>(engine: &Engine, economy: Economy<O>) -> paths::Economy {
    let point = engine
        .store()
        .record::<O>(economy, Checkpoints::Ends)
        .finish()
        .unwrap();
    engine.history(point).unwrap().0
}

/// The outcome `name` of the sample's Rust code is the history of the
/// package's ref `reference`, from `start`, the economy the Rust code
/// starts at.
fn recorded<O: Recordable>(
    engine: &Engine,
    outcomes: &[Outcome],
    name: &str,
    start: Economy<O>,
    reference: &str,
) {
    let outcome = find(outcomes, name).unwrap_or_else(|| panic!("no outcome {name}"));
    let point = engine.referred(&reference.parse().unwrap()).unwrap();
    let (initial, exchanges) = engine.history(point).unwrap();
    assert_eq!(
        postcard::to_allocvec(&initial).unwrap(),
        postcard::to_allocvec(&kept(engine, start)).unwrap(),
        "{reference}: the economy it starts at"
    );
    assert_eq!(exchanges, outcome.trace, "{reference}");
}

// What each sample's Rust code gives at each instance: the outcomes of its
// strategies, by the names its package gives them, and of the plans it
// keeps beside them.

fn maze_initial(size: Size) -> maze::World {
    match size {
        Size::Micro => maze::initial(),
        Size::Showcase => maze::initial_showcase(),
        Size::Stress => maze::initial_stress(),
    }
}

fn maze_outcomes(size: Size) -> Vec<Outcome> {
    use ObjectiveDirection::Minimize;
    use maze::ObjectiveKey::{Energy, Time};
    let initial = maze_initial(size);
    let pareto = maze::pareto_front(&initial).unwrap();
    [
        (
            "breadth-first",
            maze::solve_bfs(&initial).unwrap().trace().clone(),
        ),
        (
            "dijkstra",
            maze::solve_dijkstra(&initial).unwrap().trace().clone(),
        ),
        (
            "a-star",
            maze::solve_astar(&initial).unwrap().trace().clone(),
        ),
        (
            "pareto-energy",
            best_on(&pareto, (Energy, Minimize), (Time, Minimize)),
        ),
        (
            "pareto-time",
            best_on(&pareto, (Time, Minimize), (Energy, Minimize)),
        ),
    ]
    .into_iter()
    .map(|(name, trace)| {
        let after = initial.replayed(&trace).unwrap();
        let values = valued([
            ("energy", maze::spent_energy(&after)),
            ("time", maze::spent_time(&after)),
        ]);
        outcome(name, &trace, values)
    })
    .collect()
}

fn sokoban_initial(size: Size) -> sokoban::World {
    match size {
        Size::Micro => sokoban::initial(),
        Size::Showcase => sokoban::initial_showcase(),
        Size::Stress => sokoban::initial_stress(),
    }
}

/// Sokoban's searches, A* over its crate-to-goal lower bound and
/// breadth-first search, as resumable sessions advanced by the effort's
/// expansions at a time. Of the equally short plans in the larger
/// warehouses, the A* session finds another than `sokoban::solve_astar`,
/// whose one-call search orders its frontier otherwise.
fn sokoban_searched(initial: &sokoban::World, a_star: bool) -> Trace<sokoban::Ontology> {
    use axionomy_search::{
        AStarSession, BfsSession,
        session::{Continue, WorkBudget},
    };
    let budget = || WorkBudget::new(EFFORT);
    let solution = if a_star {
        let mut session = AStarSession::new(
            initial,
            sokoban::goal(),
            sokoban::search_candidates,
            |_, _, _| 1,
            sokoban::crate_goal_distance,
        );
        while !session.status().is_terminal() {
            session.advance(budget(), &mut Continue);
        }
        session.into_solution()
    } else {
        let mut session = BfsSession::new(initial, sokoban::goal(), sokoban::search_candidates);
        while !session.status().is_terminal() {
            session.advance(budget(), &mut Continue);
        }
        session.into_solution()
    };
    solution.expect("every warehouse is solved").trace().clone()
}

fn sokoban_outcomes(size: Size) -> Vec<Outcome> {
    let initial = sokoban_initial(size);
    let mut outcomes = vec![
        outcome("a-star", &sokoban_searched(&initial, true), Vec::new()),
        outcome(
            "breadth-first",
            &sokoban_searched(&initial, false),
            Vec::new(),
        ),
    ];
    if let Some(deadlock) = sokoban::losing_trace(&initial) {
        outcomes.push(outcome("deadlock", &deadlock, Vec::new()));
    }
    outcomes
}

fn exact_cover_initial(size: Size) -> exact_cover::World {
    match size {
        Size::Micro => exact_cover::initial(),
        Size::Showcase => exact_cover::initial_showcase(),
        Size::Stress => exact_cover::initial_stress(),
    }
}

fn exact_cover_unsatisfiable(size: Size) -> exact_cover::World {
    match size {
        Size::Micro => exact_cover::unsatisfiable(),
        Size::Showcase => exact_cover::unsatisfiable_showcase(),
        Size::Stress => exact_cover::unsatisfiable_stress(),
    }
}

fn exact_cover_outcomes(size: Size) -> Vec<Outcome> {
    let initial = exact_cover_initial(size);
    let unsatisfiable = exact_cover_unsatisfiable(size);
    assert!(exact_cover::algorithm_x(&unsatisfiable).is_none());
    assert!(exact_cover::solve_bfs(&unsatisfiable).is_none());
    vec![
        outcome(
            "breadth-first",
            exact_cover::solve_bfs(&initial).unwrap().trace(),
            Vec::new(),
        ),
        outcome(
            "algorithm-x",
            &exact_cover::algorithm_x(&initial).unwrap(),
            Vec::new(),
        ),
        outcome(
            "unsatisfiable",
            &Trace::<exact_cover::Ontology>::new(),
            Vec::new(),
        ),
    ]
}

fn workshop_outcomes(size: Size) -> Vec<Outcome> {
    use ObjectiveDirection::Minimize;
    use workshop::ObjectiveKey::{Revenue, Time, Waste};
    let initial = match size {
        Size::Micro => workshop::initial(),
        Size::Showcase => workshop::initial_showcase(),
        Size::Stress => workshop::initial_stress(),
    };
    let pareto = workshop::pareto_front(&initial).unwrap();
    let mut traces = vec![
        (
            "breadth-first",
            workshop::solve_bfs(&initial).unwrap().trace().clone(),
        ),
        (
            "minimum-waste",
            workshop::minimize_waste(&initial).unwrap().trace().clone(),
        ),
        (
            "pareto-waste",
            best_on(&pareto, (Waste, Minimize), (Time, Minimize)),
        ),
        (
            "pareto-time",
            best_on(&pareto, (Time, Minimize), (Waste, Minimize)),
        ),
    ];
    // Only a workshop with customers earns revenue.
    if initial.account(&workshop::AccountId::Customer(0)).is_some() {
        let richest = pareto
            .front()
            .entries()
            .iter()
            .max_by_key(|entry| front_value(entry, &Revenue))
            .unwrap();
        traces.push(("pareto-revenue", richest.payload().clone()));
    }
    traces
        .into_iter()
        .map(|(name, trace)| {
            let after = initial.replayed(&trace).unwrap();
            let values = workshop::objectives(&after)
                .objectives()
                .iter()
                .map(|objective| {
                    let key = match objective.key() {
                        Waste => "waste",
                        Time => "time",
                        Revenue => "revenue",
                    };
                    (key, Quantity::from(*objective.value()))
                })
                .collect();
            outcome(name, &trace, values)
        })
        .collect()
}

/// A bridge's objectives: each traveller's priority and credit, and in
/// timed traffic its on-time trips and the time elapsed.
fn bridge_values(world: &bridge::World) -> Vec<(&'static str, Quantity)> {
    use bridge::AgentId::{A, B};
    let mut values = valued([
        ("priority/a", bridge::priority(world, A)),
        ("credit/a", bridge::credit(world, A)),
        ("priority/b", bridge::priority(world, B)),
        ("credit/b", bridge::credit(world, B)),
    ]);
    if bridge::timed::enabled(world) {
        let ontime =
            |agent| world.balance(&bridge::AccountId::Agent(agent), &bridge::Asset::OnTime);
        values.extend([
            ("ontime/a", ontime(A)),
            ("ontime/b", ontime(B)),
            ("elapsed", Quantity::from(bridge::timed::now(world))),
        ]);
    }
    values
}

fn bridge_outcomes(size: Size) -> Vec<Outcome> {
    use bridge::AgentId::{self, A, B};
    let initial = match size {
        Size::Micro => bridge::initial(),
        Size::Showcase => bridge::initial_showcase(),
        Size::Stress => bridge::initial_stress(),
    };
    let first_come = |agent| match size {
        Size::Micro => bridge::first_come_proposal(agent),
        Size::Showcase => bridge::first_come_showcase(agent),
        Size::Stress => bridge::first_come_stress(agent),
    };
    let first_a = first_come(A).unwrap();
    let first_b = first_come(B).unwrap();
    let auction = match size {
        Size::Micro => bridge::auction_proposal(2, 1),
        Size::Showcase => bridge::auction_showcase(),
        Size::Stress => bridge::auction_stress(),
    }
    .unwrap();
    let adaptive = bridge::timed::enabled(&initial)
        .then(|| bridge::timed::run(&initial, bridge::timed::Policy::Adaptive).unwrap());
    // Micro's exact front, preferring a traveller's priority, then its
    // credit, then the other's; in timed traffic, the policy whose run is
    // best by the traveller's on-time trips, priority and credit, the last
    // of equals.
    let pareto = (size == Size::Micro).then(|| bridge::pareto_front(&initial).unwrap());
    let favoured = |favored: AgentId| -> Trace<bridge::Ontology> {
        use bridge::ObjectiveKey::{Credit, Priority};
        let other = if favored == A { B } else { A };
        if let Some(pareto) = &pareto {
            return pareto
                .front()
                .entries()
                .iter()
                .max_by_key(|entry| {
                    (
                        front_value(entry, &Priority(favored)),
                        front_value(entry, &Credit(favored)),
                        front_value(entry, &Priority(other)),
                        front_value(entry, &Credit(other)),
                    )
                })
                .unwrap()
                .payload()
                .clone();
        }
        [&first_a, &first_b, &auction]
            .into_iter()
            .chain(adaptive.as_ref())
            .max_by_key(|trace| {
                let after = initial.replayed(trace).unwrap();
                (
                    after
                        .balance(&bridge::AccountId::Agent(favored), &bridge::Asset::OnTime)
                        .to_u64(),
                    bridge::priority(&after, favored),
                    bridge::credit(&after, favored),
                )
            })
            .unwrap()
            .clone()
    };
    let mut traces = vec![
        (
            "breadth-first",
            bridge::solve(&initial).unwrap().trace().clone(),
        ),
        ("pareto-a", favoured(A)),
        ("pareto-b", favoured(B)),
        ("first-come-a", first_a.clone()),
        ("first-come-b", first_b.clone()),
        ("auction", auction.clone()),
    ];
    traces.extend(adaptive.clone().map(|trace| ("adaptive", trace)));
    traces
        .into_iter()
        .map(|(name, trace)| {
            outcome(
                name,
                &trace,
                bridge_values(&initial.replayed(&trace).unwrap()),
            )
        })
        .collect()
}

fn rescue_outcomes(size: Size) -> Vec<Outcome> {
    use rescue::Policy;
    if size == Size::Micro {
        // The scenario Micro plays: the survivor south, the sensor's seed 1.
        let model = rescue::uniform_uncertain();
        let sample = rescue::instantiate(&model, rescue::Location::South, 1).unwrap();
        return [
            ("observe-then-follow", Policy::ObserveThenFollow),
            ("direct-north", Policy::NorthWithoutObserving),
        ]
        .into_iter()
        .map(|(name, policy)| {
            let rollout = rescue::run_sampled_policy(&mut model.clone(), &sample, policy).unwrap();
            let values = valued([
                ("success", u64::from(rollout.succeeded())),
                ("sensor", u64::from(rollout.used_sensor())),
                ("energy", rollout.spent_energy()),
            ]);
            outcome(name, rollout.trace(), values)
        })
        .collect();
    }
    use rescue::incident::{self, AccountId, Asset};
    let initial = incident::initial(size == Size::Stress, SEED);
    [
        ("observe-then-follow", incident::Policy::Triage),
        ("direct-north", incident::Policy::RapidNorth),
        ("stabilize-first", incident::Policy::StabilizeFirst),
    ]
    .into_iter()
    .map(|(name, policy)| {
        let (trace, after) = simulated(&initial, |world| incident::run(world, policy));
        let value = |account, asset| incident::value(&after, account, asset);
        let values = valued([
            ("saved", value(AccountId::World, Asset::SavedCount)),
            ("vitality", value(AccountId::World, Asset::SavedVitality)),
            ("deaths", value(AccountId::World, Asset::DeathCount)),
            ("fuel", value(AccountId::Responder, Asset::SpentFuel)),
            (
                "medicine",
                value(AccountId::Responder, Asset::UsedMedicalKit),
            ),
            ("time", value(AccountId::World, Asset::Time)),
        ]);
        outcome(name, &trace, values)
    })
    .collect()
}

/// Micro's front of the rescue's policies, sampled by Monte Carlo over
/// Nature's prior.
fn rescue_front() -> rescue::PolicyFront {
    rescue::policy_front(&rescue::uniform_uncertain(), EFFORT, SEED, &mut Unrecorded).unwrap()
}

fn scheduling_outcomes(size: Size) -> Vec<Outcome> {
    use ObjectiveDirection::Minimize;
    use scheduling::ObjectiveKey::{JobOneCompletion as One, JobTwoCompletion as Two};
    let initial = match size {
        Size::Micro => scheduling::initial(),
        Size::Showcase => scheduling::initial_showcase(),
        Size::Stress => scheduling::initial_stress(),
    };
    let pareto = scheduling::pareto_front(&initial).unwrap();
    let mut outcomes: Vec<_> = [
        (
            "best-first",
            scheduling::solve_best_first(&initial)
                .unwrap()
                .trace()
                .clone(),
        ),
        (
            "bounded-optimizer",
            scheduling::branch_optimize(&initial)
                .unwrap()
                .trace()
                .clone(),
        ),
        (
            "pareto-job-one",
            best_on(&pareto, (One, Minimize), (Two, Minimize)),
        ),
        (
            "pareto-job-two",
            best_on(&pareto, (Two, Minimize), (One, Minimize)),
        ),
    ]
    .into_iter()
    .map(|(name, trace)| {
        let after = initial.replayed(&trace).unwrap();
        let values = valued([
            (
                "job-one",
                scheduling::completion_time(&after, scheduling::Job::One),
            ),
            (
                "job-two",
                scheduling::completion_time(&after, scheduling::Job::Two),
            ),
        ]);
        outcome(name, &trace, values)
    })
    .collect();
    outcomes.push(outcome(
        "infeasible-horizon",
        &Trace::<scheduling::Ontology>::new(),
        Vec::new(),
    ));
    outcomes
}

/// Too short a horizon for each instance's work.
fn scheduling_impossible(size: Size) -> scheduling::World {
    match size {
        Size::Micro => scheduling::impossible(),
        Size::Showcase | Size::Stress => scheduling::impossible_showcase(),
    }
}

fn amm_outcomes(size: Size) -> Vec<Outcome> {
    use amm::{AccountId, Actor, Asset, Scenario};
    let initial = match size {
        Size::Micro => amm::initial(),
        Size::Showcase => amm::initial_showcase_with_seed(SEED),
        Size::Stress => amm::initial_stress_with_seed(SEED),
    };
    let sum = |world: &amm::World, actors: &[Actor], asset: fn(Actor) -> Asset| -> u64 {
        actors
            .iter()
            .map(|&actor| {
                world
                    .balance(&AccountId::Actor(actor), &asset(actor))
                    .to_u64()
            })
            .sum()
    };
    let needy = [Actor::Factory, Actor::Household];
    [
        ("market-day", Scenario::MarketDay),
        ("no-whale", Scenario::NoWhale),
        ("thin-liquidity", Scenario::ThinLiquidity),
    ]
    .into_iter()
    .map(|(name, scenario)| {
        let trace = amm::trace(&initial, scenario);
        let after = initial.replayed(&trace).unwrap();
        let values = valued([
            ("needs", sum(&after, &needy, Asset::SatisfiedNeed)),
            ("utility", sum(&after, &amm::ACTORS, |_| Asset::Utility)),
            ("depth", amm::pool_state(&after).energy),
            ("unmet", sum(&after, &needy, Asset::MissedNeed)),
            (
                "defaults",
                sum(
                    &after,
                    &[Actor::Factory, Actor::Generator],
                    Asset::DefaultedObligation,
                ),
            ),
        ]);
        outcome(name, &trace, values)
    })
    .collect()
}

fn lending_outcomes(size: Size) -> Vec<Outcome> {
    use lending::{AccountId, Asset, Config, balance};
    let config = match size {
        Size::Micro => Config::micro(),
        Size::Showcase => Config::showcase(),
        Size::Stress => Config::stress(),
    };
    let initial = lending::world(&config);
    let values = |trace: &lending::LoanTrace| {
        let after = initial.replayed(trace).unwrap();
        valued([
            (
                "interest",
                lending::interest_charged(&initial, trace).unwrap(),
            ),
            (
                "collateral",
                balance(&after, AccountId::Borrower, Asset::Collateral)
                    + balance(&after, AccountId::Borrower, Asset::PostedCollateral),
            ),
            ("cash", balance(&after, AccountId::Borrower, Asset::Cash)),
        ])
    };
    let cheapest = lending::least_interest(&config).unwrap();
    let fewest = lending::fewest_steps(&config).unwrap();
    let (_, liquidation) = lending::over_borrowed(&config).unwrap();
    // From the instance's fresh market, where no value is measured.
    let (_, attack) = lending::first_depositor_attack(&config).unwrap();
    vec![
        outcome("least-interest", cheapest.trace(), values(cheapest.trace())),
        outcome("fewest-steps", fewest.trace(), values(fewest.trace())),
        outcome("over-borrowed", &liquidation, values(&liquidation)),
        outcome("first-depositor-attack", &attack, Vec::new()),
    ]
}

fn marketplace_outcomes(size: Size) -> Vec<Outcome> {
    use marketplace::{AccountId, BuyerId, ObjectiveKey, SellerId, fulfillment};
    let initial = match size {
        Size::Micro => marketplace::initial(),
        Size::Showcase => marketplace::initial_showcase(),
        Size::Stress => marketplace::initial_stress(),
    };
    // Micro ranks its exact front by the buyers' or the sellers' total
    // utility, the last of equals; the funded markets plan fulfillment.
    let ranked = |buyers: bool| -> Trace<marketplace::Ontology> {
        if fulfillment::enabled(&initial) {
            let preference = if buyers {
                fulfillment::Preference::Buyers
            } else {
                fulfillment::Preference::Sellers
            };
            return fulfillment::plan(&initial, preference).unwrap();
        }
        marketplace::pareto_front(&initial)
            .unwrap()
            .front()
            .entries()
            .iter()
            .max_by_key(|entry| {
                entry
                    .objectives()
                    .objectives()
                    .iter()
                    .filter(|objective| {
                        matches!(
                            (buyers, objective.key()),
                            (true, ObjectiveKey::Buyer(_)) | (false, ObjectiveKey::Seller(_))
                        )
                    })
                    .map(|objective| *objective.value())
                    .sum::<u64>()
            })
            .unwrap()
            .payload()
            .clone()
    };
    let values = |world: &marketplace::World| {
        let utility = |account| marketplace::utility(world, account);
        let mut values = valued([
            ("buyer-a", utility(AccountId::Buyer(BuyerId::A))),
            ("buyer-b", utility(AccountId::Buyer(BuyerId::B))),
            ("seller-a", utility(AccountId::Seller(SellerId::A))),
            ("seller-b", utility(AccountId::Seller(SellerId::B))),
        ]);
        if fulfillment::enabled(world) {
            let ontime = marketplace::orders(world)
                .into_iter()
                .map(|order| {
                    world
                        .balance(&AccountId::Order(order), &marketplace::Asset::OnTime)
                        .to_u64()
                })
                .sum::<u64>();
            values.extend(valued([
                ("buyer-c", utility(AccountId::Buyer(BuyerId::C))),
                ("seller-c", utility(AccountId::Seller(SellerId::C))),
                ("ontime", ontime),
                ("elapsed", fulfillment::now(world)),
            ]));
        }
        values
    };
    [
        (
            "market-clearing",
            marketplace::clear_market(&initial).trace().clone(),
        ),
        ("pareto-buyers", ranked(true)),
        ("pareto-sellers", ranked(false)),
    ]
    .into_iter()
    .map(|(name, trace)| outcome(name, &trace, values(&initial.replayed(&trace).unwrap())))
    .collect()
}

/// The salt Micro's planner seeds its choices with, beside the seed.
const LOGISTICS_SALT: u64 = 0x4c4f_4749_5354_4943;

fn logistics_outcomes(size: Size) -> Vec<Outcome> {
    if size == Size::Micro {
        use logistics::{MissionRollout, Policy};
        let initial = logistics::initial_micro();
        let values = |rollout: &MissionRollout| {
            valued([
                ("completed", u64::from(rollout.completed())),
                ("delivered", rollout.delivered() as u64),
                ("time", rollout.elapsed_time()),
            ])
        };
        let mut outcomes: Vec<_> = [("direct", Policy::Direct), ("reliable", Policy::Reliable)]
            .into_iter()
            .map(|(name, policy)| {
                let rollout = logistics::run_policy(&mut initial.clone(), policy, SEED);
                outcome(name, rollout.trace(), values(&rollout))
            })
            .collect();
        // The planner's iterations: the effort, as many as Micro samples.
        let planned = logistics::run_planned_with_progress(
            &initial,
            16,
            SEED,
            SEED ^ LOGISTICS_SALT,
            8,
            |_, _| ControlFlow::Continue(()),
        )
        .unwrap()
        .unwrap();
        outcomes.push(outcome(
            "mcts",
            planned.rollout.trace(),
            values(&planned.rollout),
        ));
        return outcomes;
    }
    use logistics::fleet::{self, AccountId, Asset, Policy, VANS};
    let initial = fleet::initial(size == Size::Stress, SEED);
    [
        ("direct", Policy::Fastest),
        ("reliable", Policy::Reliable),
        ("mcts", Policy::Relay),
    ]
    .into_iter()
    .map(|(name, policy)| {
        let (trace, after) = simulated(&initial, |world| fleet::run(world, policy));
        let value = |account, asset| fleet::value(&after, account, asset);
        let values = valued([
            ("delivered", value(AccountId::World, Asset::DeliveredCount)),
            ("on-time", value(AccountId::World, Asset::OnTimeCount)),
            ("expired", value(AccountId::World, Asset::ExpiredCount)),
            ("cash", value(AccountId::World, Asset::Money)),
            (
                "fuel",
                VANS.into_iter()
                    .map(|van| value(AccountId::Vehicle(van), Asset::SpentFuel))
                    .sum(),
            ),
            ("time", fleet::now(&after)),
        ]);
        outcome(name, &trace, values)
    })
    .collect()
}

/// How many iterations Red searches each move with at each instance.
fn connect_four_strength(size: Size) -> usize {
    match size {
        Size::Micro => 64,
        Size::Showcase => 16,
        Size::Stress => 32,
    }
}

fn connect_four_outcomes(size: Size) -> Vec<Outcome> {
    let iterations = connect_four_strength(size);
    let chunk = iterations.clamp(1, 8);
    let carry_on = |_| ControlFlow::Continue(());
    let (initial, self_play) = if size == Size::Micro {
        let game = connect_four::play_game_with_progress(iterations, SEED, chunk, carry_on);
        (connect_four::initial(), game)
    } else {
        let game =
            connect_four::play_standard_game_with_progress(iterations, SEED, chunk, carry_on);
        (connect_four::initial_standard(), game)
    };
    let mut games = vec![("self-play", self_play.unwrap())];
    for (name, opponent) in [
        ("center-opponent", connect_four::Opponent::CenterFirst),
        ("search-opponent", connect_four::Opponent::AlphaBeta),
    ] {
        let game = connect_four::play_opponent_game_from_with_progress(
            initial.clone(),
            iterations,
            SEED,
            opponent,
            |_| ControlFlow::Continue(()),
        );
        games.push((name, game.unwrap()));
    }
    games
        .into_iter()
        .map(|(name, trace)| {
            let terminal = connect_four::terminal_values(&initial.replayed(&trace).unwrap())
                .expect("every game ends");
            // A win is one, a draw a half and a loss nothing, in tenths.
            let tenths = |value: f64| (value * 10.0).round() as u64;
            let values = valued([
                ("red", tenths(terminal[0])),
                ("yellow", tenths(terminal[1])),
            ]);
            outcome(name, &trace, values)
        })
        .collect()
}

/// Each of the league's fields: its name and the lineup its workers play.
fn work_league_fields() -> [(&'static str, [work_league::Policy; 4]); 3] {
    [
        ("mixed-field", work_league::mixed_lineup()),
        ("throughput-field", work_league::throughput_lineup()),
        ("sustainable-field", work_league::sustainable_lineup()),
    ]
}

fn work_league_profile(size: Size) -> work_league::Profile {
    match size {
        Size::Micro => work_league::Profile::Micro,
        Size::Showcase => work_league::Profile::Showcase,
        Size::Stress => work_league::Profile::Stress,
    }
}

fn work_league_outcomes(size: Size) -> Vec<Outcome> {
    use work_league::{AccountId, Asset};
    work_league_fields()
        .into_iter()
        .map(|(name, lineup)| {
            let league = work_league::league(work_league_profile(size), lineup);
            let ran = work_league::run(&league, SEED).unwrap();
            let total = |asset: Asset| -> u64 {
                league
                    .agents()
                    .iter()
                    .map(|agent| {
                        ran.final_world()
                            .balance(&AccountId::Agent(*agent), &asset)
                            .to_u64()
                    })
                    .sum()
            };
            let values = valued([
                ("value", total(Asset::Value)),
                ("completed", total(Asset::Completed)),
                ("waste", total(Asset::Waste)),
            ]);
            outcome(name, ran.trace(), values)
        })
        .collect()
}

/// How many scenarios Micro's mission samples, and plans with.
const MISSION_SAMPLES: usize = 16;

fn mission_outcomes(size: Size) -> Vec<Outcome> {
    if size == Size::Micro {
        use mission::{MissionRollout, Policy};
        let model = mission::initial();
        // The scenario the seed draws, of the sixteen.
        let sample = usize::try_from(SEED).unwrap() % 16;
        let values = |rollout: &MissionRollout| {
            valued([
                ("success", u64::from(rollout.succeeded())),
                ("time", rollout.elapsed_time()),
                ("medical", u64::from(rollout.used_medical_kit())),
            ])
        };
        let planned = mission::run_planned_with_progress(
            &model,
            sample,
            MctsConfig::new(MISSION_SAMPLES, 12).with_seed(SEED),
            8,
            |_, _| ControlFlow::Continue(()),
        )
        .unwrap()
        .unwrap();
        let north = mission::run_policy(&mut model.clone(), Policy::NorthTogether, sample);
        return vec![
            outcome(
                "coordinated",
                planned.rollout.trace(),
                values(&planned.rollout),
            ),
            outcome("direct-north", north.trace(), values(&north)),
        ];
    }
    use mission::expedition::{self, AGENTS, AccountId, Agent, Asset, Policy};
    let initial = expedition::initial(size == Size::Stress, SEED);
    [
        ("coordinated", Policy::RadioCoordination),
        ("direct-north", Policy::NorthFirst),
        ("rendezvous", Policy::Rendezvous),
    ]
    .into_iter()
    .map(|(name, policy)| {
        let (trace, after) = simulated(&initial, |world| expedition::run(world, policy));
        let value = |account, asset| expedition::value(&after, account, asset);
        let values = valued([
            ("completed", value(AccountId::World, Asset::Completed)),
            ("failed", value(AccountId::World, Asset::FailedCount)),
            ("vitality", value(AccountId::World, Asset::SavedVitality)),
            (
                "radio",
                value(AccountId::Agent(Agent::Scout), Asset::UsedRadio),
            ),
            (
                "energy",
                AGENTS
                    .into_iter()
                    .map(|agent| value(AccountId::Agent(agent), Asset::SpentEnergy))
                    .sum(),
            ),
            ("time", expedition::now(&after)),
        ]);
        outcome(name, &trace, values)
    })
    .collect()
}

/// Micro's front of the mission's fixed policies, sampled by Monte Carlo
/// over Nature's prior.
fn mission_front() -> mission::PolicyFront {
    mission::policy_front_with_progress(
        &mission::initial(),
        MISSION_SAMPLES,
        SEED,
        &mut Unrecorded,
        8,
        |_| ControlFlow::Continue(()),
    )
    .unwrap()
}

fn perishables_outcomes(size: Size) -> Vec<Outcome> {
    if size == Size::Micro {
        use ObjectiveDirection::{Maximize, Minimize};
        use perishables::ObjectiveKey::{CoolingEnergy, UsableInventory};
        // Micro's inventory, moved between rooms ten units at a time.
        let initial = perishables::initial_with_inventory(70, 30);
        let transfer = 10;
        let outage = perishables::run_outage_scenario(&initial, transfer).unwrap();
        let claims = outage.claim_index();
        let mut outcomes = vec![outcome(
            "outage",
            outage.trace(),
            vec![
                ("claims", Quantity::from(claims.total_claims())),
                (
                    "usable",
                    Quantity::from(claims.usable_total(outage.world())),
                ),
            ],
        )];
        let front = perishables::storage_plan_front_with_transfer(&initial, transfer).unwrap();
        for (name, trace) in [
            (
                "pareto-inventory",
                best_on(
                    &front,
                    (UsableInventory, Maximize),
                    (CoolingEnergy, Minimize),
                ),
            ),
            (
                "pareto-energy",
                best_on(
                    &front,
                    (CoolingEnergy, Minimize),
                    (UsableInventory, Maximize),
                ),
            ),
        ] {
            let after = initial.replayed(&trace).unwrap();
            let values = valued([
                ("usable", perishables::usable_inventory(&after)),
                ("energy", perishables::spent_cooling_energy(&after)),
            ]);
            outcomes.push(outcome(name, &trace, values));
        }
        return outcomes;
    }
    use perishables::cold_chain::{self as chain, AccountId, Asset, Policy};
    let initial = chain::initial(size == Size::Stress, SEED);
    [
        ("outage", Policy::ProtectCold),
        ("pareto-inventory", Policy::ProcessFirst),
        ("pareto-energy", Policy::ConserveFuel),
        ("storage-buffer", Policy::RebalanceCold),
    ]
    .into_iter()
    .map(|(name, policy)| {
        let trace = chain::run(&initial, policy);
        let after = initial.replayed(&trace).unwrap();
        let total = |asset| chain::total(&after, asset);
        let values = valued([
            (
                "saved",
                total(Asset::Fresh) + total(Asset::Preserved) + total(Asset::Delivered),
            ),
            ("delivered", total(Asset::Delivered)),
            (
                "revenue",
                chain::value(&after, AccountId::World, Asset::Money),
            ),
            ("energy", total(Asset::SpentEnergy)),
            (
                "fuel",
                total(Asset::GeneratorFuel) + total(Asset::TransportFuel),
            ),
            ("missed", total(Asset::MissedDemand)),
        ]);
        outcome(name, &trace, values)
    })
    .collect()
}

/// What each sample checks of its package at `instance`.
fn maze(engine: &Engine, instance: &str) {
    let outcomes = maze_outcomes(Size::of(instance));
    let strategies = [
        "breadth-first",
        "dijkstra",
        "a-star",
        "pareto-energy",
        "pareto-time",
    ];
    assert_eq!(
        reproduces(engine, "maze", instance, &outcomes, &strategies),
        strategies.len(),
        "{instance}"
    );
}

fn sokoban(engine: &Engine, instance: &str) {
    let size = Size::of(instance);
    let outcomes = sokoban_outcomes(size);
    assert_eq!(
        reproduces(
            engine,
            "sokoban",
            instance,
            &outcomes,
            &["a-star", "breadth-first"]
        ),
        2,
        "{instance}"
    );
    recorded(
        engine,
        &outcomes,
        "deadlock",
        sokoban_initial(size),
        &format!("sokoban/{instance}/deadlock"),
    );
}

fn exact_cover(engine: &Engine, instance: &str) {
    let size = Size::of(instance);
    let outcomes = exact_cover_outcomes(size);
    assert_eq!(
        reproduces(
            engine,
            "exact-cover",
            instance,
            &outcomes,
            &["breadth-first", "algorithm-x"],
        ),
        2,
        "{instance}"
    );
    let unsatisfiable = format!("exact-cover/{instance}/unsatisfiable");
    recorded(
        engine,
        &outcomes,
        "unsatisfiable",
        exact_cover_unsatisfiable(size),
        &unsatisfiable,
    );
    // No strategy covers the unsatisfiable twin: search runs out, and
    // Algorithm X makes no plan.
    let results = engine
        .run_experiment(&unsatisfiable.parse().unwrap(), &mut |_, _| {
            ControlFlow::Continue(())
        })
        .unwrap();
    let ended: Vec<_> = results.arms.iter().map(|arm| arm.trials[0].ended).collect();
    assert_eq!(ended, [Ended::Exhausted, Ended::Stopped], "{instance}");
}

fn workshop(engine: &Engine, instance: &str) {
    let compared = reproduces(
        engine,
        "workshop",
        instance,
        &workshop_outcomes(Size::of(instance)),
        &[
            "breadth-first",
            "minimum-waste",
            "pareto-waste",
            "pareto-time",
            "pareto-revenue",
        ],
    );
    // Only instances with customers earn revenue.
    assert_eq!(
        compared,
        if instance == "micro" { 4 } else { 5 },
        "{instance}"
    );
}

fn bridge(engine: &Engine, instance: &str) {
    let outcomes = bridge_outcomes(Size::of(instance));
    // Micro's mechanisms meet on an exact front; timed traffic adds the
    // deadline-responsive policy and compares only the policies it runs.
    let strategies: &[&str] = if instance == "micro" {
        &[
            "breadth-first",
            "first-come-a",
            "first-come-b",
            "auction",
            "pareto-a",
            "pareto-b",
        ]
    } else {
        &[
            "breadth-first",
            "first-come-a",
            "first-come-b",
            "auction",
            "adaptive",
        ]
    };
    assert_eq!(
        reproduces(engine, "bridge", instance, &outcomes, strategies),
        strategies.len(),
        "{instance}"
    );
    if instance == "micro" {
        return;
    }
    // In timed traffic the bridge prefers, for each traveller, the policy
    // whose run is best by that traveller's on-time trips, then priority,
    // then credit, the last of equals: `pick` keeps the first, so it reads
    // the experiment's runs in reverse.
    let results = engine
        .results(&format!("bridge/{instance}").parse().unwrap())
        .unwrap();
    let ends: Vec<_> = ["adaptive", "auction", "first-come-b", "first-come-a"]
        .into_iter()
        .map(|policy| {
            let arm = results
                .arms
                .iter()
                .find(|arm| arm.arm.strategy.as_str() == format!("bridge/{policy}"))
                .unwrap_or_else(|| panic!("no arm runs {policy}"));
            arm.trials[0].end
        })
        .collect();
    for (name, traveller) in [("pareto-a", "a"), ("pareto-b", "b")] {
        let by: Vec<ObjectivePath> = ["ontime", "priority", "credit"]
            .into_iter()
            .map(|objective| format!("bridge/{objective}/{traveller}").parse().unwrap())
            .collect();
        let picked = engine.pick(&ends, &by).unwrap().unwrap();
        let preferred = find(&outcomes, name).unwrap();
        assert_eq!(
            engine.history(picked).unwrap().1,
            preferred.trace,
            "bridge {instance} {name}: the trace"
        );
        for (key, value) in &preferred.values {
            let path = format!("bridge/{key}").parse().unwrap();
            assert_eq!(
                &engine.measure(picked, &path).unwrap(),
                value,
                "bridge {instance} {name}: {key}"
            );
        }
    }
}

fn rescue(engine: &Engine, instance: &str) {
    let compared = reproduces(
        engine,
        "rescue",
        instance,
        &rescue_outcomes(Size::of(instance)),
        &["observe-then-follow", "direct-north", "stabilize-first"],
    );
    // Only the incidents have kits to stabilize with.
    assert_eq!(
        compared,
        if instance == "micro" { 2 } else { 3 },
        "{instance}"
    );
    if instance == "micro" {
        rescue_monte_carlo(engine);
    }
}

fn lending(engine: &Engine, instance: &str) {
    // The searches, the over-borrowed scenario, and the attack on the
    // instance's fresh market, an arm starting from a ref of its own.
    let strategies = [
        "least-interest",
        "fewest-steps",
        "over-borrowed",
        "first-depositor-attack",
    ];
    assert_eq!(
        reproduces(
            engine,
            "lending",
            instance,
            &lending_outcomes(Size::of(instance)),
            &strategies
        ),
        strategies.len(),
        "{instance}"
    );
}

/// Every episode a Rust Monte Carlo of the rescue played, in its order.
#[derive(Default)]
struct Episodes(Vec<(rescue::Policy, usize, Vec<paths::Exchange>)>);

impl axionomy_search::rollout::Episodes<rescue::Ontology, rescue::Policy, rescue::Rollout>
    for Episodes
{
    type World = rescue::World;

    fn begin(&mut self, _: &rescue::Policy, _: usize, initial: rescue::World) -> rescue::World {
        initial
    }

    fn end(
        &mut self,
        policy: &rescue::Policy,
        sample: usize,
        _: rescue::World,
        rollout: &rescue::Rollout,
    ) {
        let trace = rollout.trace().exchanges().iter().map(written).collect();
        self.0.push((*policy, sample, trace));
    }
}

/// A Monte Carlo mean over `trials` samples, as the exact total it averages:
/// a total one apart moves the mean by a whole `1 / trials`, far beyond its
/// rounding.
fn total_of(mean: f64, trials: usize) -> u64 {
    let total = mean * trials as f64;
    let rounded = total.round();
    assert!(
        (total - rounded).abs() < 1e-6,
        "{mean} is no mean of {trials} counts"
    );
    rounded as u64
}

/// Micro's rescue estimates each policy by Monte Carlo over Nature's prior:
/// the package's Monte Carlo experiment plays the scenario each of the Rust
/// Monte Carlo's samples plays, trial by trial, and its totals are what the
/// Rust front averages, on every objective.
fn rescue_monte_carlo(engine: &Engine) {
    let path = path!("rescue/micro/monte-carlo");
    let experiment = engine.experiment(&path).unwrap();
    let results = engine
        .run_experiment(&path, &mut |_, _| ControlFlow::Continue(()))
        .unwrap();
    let trials = usize::try_from(experiment.trials).unwrap();
    assert_eq!((trials, experiment.effort.seed), (EFFORT, SEED));

    // Trial by trial, the episodes of the Rust Monte Carlo seeded alike.
    let mut episodes = Episodes::default();
    rescue::policy_front(
        &rescue::uniform_uncertain(),
        trials,
        experiment.effort.seed,
        &mut episodes,
    )
    .unwrap();
    let policies = [
        ("observe-then-follow", rescue::Policy::ObserveThenFollow),
        ("direct-north", rescue::Policy::NorthWithoutObserving),
    ];
    let mut played = episodes.0.iter();
    for (strategy, policy) in policies {
        for (index, trial) in arm(&results, strategy).trials.iter().enumerate() {
            let (sampled, sample, trace) = played.next().unwrap();
            assert_eq!((*sampled, *sample), (policy, index));
            assert_eq!(
                engine.history(trial.end).unwrap().1,
                *trace,
                "rescue micro {strategy}: sample {index}"
            );
        }
    }
    assert!(played.next().is_none());

    // Each policy's point of the front: the arm's totals.
    let front = rescue_front();
    assert_eq!(front.entries().len(), policies.len());
    for entry in front.entries() {
        let estimate = entry.payload();
        let (strategy, _) = policies
            .iter()
            .find(|(_, policy)| policy == estimate.policy())
            .unwrap();
        let totals = arm(&results, strategy).totals();
        for (objective, summary) in entry
            .objectives()
            .objectives()
            .iter()
            .zip(estimate.summary().dimensions())
        {
            assert_eq!(summary.samples(), trials);
            let key = match objective.key() {
                rescue::PolicyObjective::SuccessProbability => "success",
                rescue::PolicyObjective::SensorUseProbability => "sensor",
                rescue::PolicyObjective::MeanSpentEnergy => "energy",
            };
            let index = objective_named(&experiment.objectives, key)
                .unwrap_or_else(|| panic!("rescue measures no `{key}`"));
            assert_eq!(
                totals[index],
                Quantity::from(total_of(*objective.value(), trials)),
                "rescue micro {strategy}: {key}"
            );
        }
    }
}

fn scheduling(engine: &Engine, instance: &str) {
    let size = Size::of(instance);
    let outcomes = scheduling_outcomes(size);
    let compared = reproduces(
        engine,
        "scheduling",
        instance,
        &outcomes,
        &[
            "best-first",
            "bounded-optimizer",
            "pareto-job-one",
            "pareto-job-two",
        ],
    );
    assert_eq!(compared, 4, "{instance}");
    let infeasible = format!("scheduling/{instance}/infeasible-horizon");
    recorded(
        engine,
        &outcomes,
        "infeasible-horizon",
        scheduling_impossible(size),
        &infeasible,
    );
    // No strategy fits the work into too short a horizon: every search
    // runs out.
    let results = engine
        .run_experiment(&infeasible.parse().unwrap(), &mut |_, _| {
            ControlFlow::Continue(())
        })
        .unwrap();
    let ended: Vec<_> = results.arms.iter().map(|arm| arm.trials[0].ended).collect();
    assert_eq!(ended, [Ended::Exhausted; 4], "{instance}");
}

fn amm(engine: &Engine, instance: &str) {
    // Micro plans its one market day whole; the seasons play turn by turn.
    let documents = ["market-day", "no-whale", "thin-liquidity"];
    assert_eq!(
        reproduces(
            engine,
            "amm",
            instance,
            &amm_outcomes(Size::of(instance)),
            &documents
        ),
        documents.len(),
        "{instance}"
    );
}

fn marketplace(engine: &Engine, instance: &str) {
    // Micro clears and ranks its exact front; the funded markets plan
    // fulfillment by beam search, for volume, buyers or sellers.
    let compared = reproduces(
        engine,
        "marketplace",
        instance,
        &marketplace_outcomes(Size::of(instance)),
        &["market-clearing", "pareto-buyers", "pareto-sellers"],
    );
    assert_eq!(compared, 3, "{instance}");
}

fn logistics(engine: &Engine, instance: &str) {
    // Micro's fixed routes and planner; the fleet's dispatch policies.
    let documents = ["direct", "reliable", "mcts"];
    assert_eq!(
        reproduces(
            engine,
            "logistics",
            instance,
            &logistics_outcomes(Size::of(instance)),
            &documents
        ),
        documents.len(),
        "{instance}"
    );
    if instance == "micro" {
        logistics_beyond_micro(engine);
    }
}

/// Micro plans a single decision. On the economy with all four orders
/// waiting, which its Monte Carlo samples, the package's policies play the
/// Rust ones, and its planner the Rust receding-horizon run, which replans
/// thirteen times there, around seven breakdowns, with this seed.
fn logistics_beyond_micro(engine: &Engine) {
    use axionomy_problems::logistics::{self, Policy};
    const SEED: u64 = 2;
    let start = engine
        .store()
        .record::<logistics::Ontology>(logistics::initial(), Checkpoints::Ends)
        .finish()
        .unwrap();
    // The goal and the turns its experiment gives.
    let experiment = engine.experiment(&path!("logistics/micro")).unwrap();
    let ran = |strategy: &str| {
        let strategy = engine
            .strategy(&format!("logistics/{strategy}").parse().unwrap())
            .unwrap();
        let effort = Effort {
            seed: SEED,
            ..experiment.effort
        };
        let ran = engine
            .run_strategy(start, &strategy, &experiment.goal, effort, &mut |_| {
                ControlFlow::Continue(())
            })
            .unwrap();
        engine.history(ran.end).unwrap().1
    };
    let traced = |trace: &Trace<logistics::Ontology>| -> Vec<paths::Exchange> {
        trace.exchanges().iter().map(written).collect()
    };
    for (strategy, policy) in [("direct", Policy::Direct), ("reliable", Policy::Reliable)] {
        let rollout = logistics::run_policy(&mut logistics::initial(), policy, SEED);
        assert_eq!(
            ran(strategy),
            traced(rollout.trace()),
            "logistics {strategy}"
        );
    }
    let planned = logistics::run_planned_with_progress(
        &logistics::initial(),
        16,
        SEED,
        SEED ^ LOGISTICS_SALT,
        8,
        |_, _| ControlFlow::Continue(()),
    )
    .unwrap()
    .unwrap();
    let trace = traced(planned.rollout.trace());
    let repairs = trace
        .iter()
        .filter(|exchange| exchange.rate().as_str().starts_with("repair/"))
        .count();
    assert_eq!((planned.decisions.len(), repairs), (13, 7));
    assert_eq!(ran("mcts"), trace, "logistics mcts");
}

fn mission(engine: &Engine, instance: &str) {
    // Micro's planner and both agents going north; the expedition's radio
    // coordination, north-first search and rendezvous.
    let compared = reproduces(
        engine,
        "mission",
        instance,
        &mission_outcomes(Size::of(instance)),
        &["coordinated", "direct-north", "rendezvous"],
    );
    assert_eq!(
        compared,
        if instance == "micro" { 2 } else { 3 },
        "{instance}"
    );
    if instance == "micro" {
        mission_monte_carlo(engine);
        // Beyond the seed's scenario, where the sensor reads wrong and the
        // planner replans on its report: a direct move south that strands
        // the team, and a direct move north where Nature's injury is seen,
        // treated, and the victim rescued.
        assert_eq!(mission_planned(engine, [2, 6]), [(false, 1), (true, 4)]);
    }
}

/// Micro plays the scenario its seed draws, sample `seed` modulo the
/// sixteen, and plans its `k`th decision seeded `seed` plus `k`. On the
/// scenario each of `seeds` draws, the package's planner makes the Rust
/// receding-horizon run's decisions; returns whether each run succeeded,
/// and after how many decisions.
fn mission_planned(engine: &Engine, seeds: impl IntoIterator<Item = u64>) -> Vec<(bool, usize)> {
    let model = mission::initial();
    let planner = engine.strategy(&path!("mission/coordinated")).unwrap();
    let experiment = engine.experiment(&path!("mission/micro")).unwrap();
    let prior = engine.referred(&path!("mission/micro")).unwrap();
    let mut outcomes = Vec::new();
    for seed in seeds {
        let planned = mission::run_planned_with_progress(
            &model,
            usize::try_from(seed % 16).unwrap(),
            MctsConfig::new(MISSION_SAMPLES, 12).with_seed(seed),
            8,
            |_, _| ControlFlow::Continue(()),
        )
        .unwrap()
        .unwrap();
        let trace: Vec<paths::Exchange> = planned
            .rollout
            .trace()
            .exchanges()
            .iter()
            .map(written)
            .collect();
        // The run's first exchange is Nature's draw of its scenario.
        let scenario = engine.apply(prior, &trace[..1]).unwrap();
        let effort = Effort {
            seed,
            ..experiment.effort
        };
        let ran = engine
            .run_strategy(scenario, &planner, &experiment.goal, effort, &mut |_| {
                ControlFlow::Continue(())
            })
            .unwrap();
        assert_eq!(engine.history(ran.end).unwrap().1, trace, "mission {seed}");
        outcomes.push((planned.rollout.succeeded(), planned.decisions.len()));
    }
    outcomes
}

/// Micro's fixed policies' front, sampled by Monte Carlo over Nature's
/// prior: the package's Monte Carlo experiment samples each policy over the
/// same scenarios, and its totals are what the Rust front averages, on
/// every objective.
fn mission_monte_carlo(engine: &Engine) {
    let path = path!("mission/micro/monte-carlo");
    let experiment = engine.experiment(&path).unwrap();
    let results = engine
        .run_experiment(&path, &mut |_, _| ControlFlow::Continue(()))
        .unwrap();
    let trials = usize::try_from(experiment.trials).unwrap();
    assert_eq!(trials, MISSION_SAMPLES);
    let front = mission_front();
    // Neither fixed policy dominates the other.
    assert_eq!(front.entries().len(), 2);
    for entry in front.entries() {
        let estimate = entry.payload();
        let strategy = match estimate.policy() {
            mission::Policy::ShareAndCoordinate => "share-and-coordinate",
            mission::Policy::NorthTogether => "direct-north",
        };
        let totals = arm(&results, strategy).totals();
        for (objective, summary) in entry
            .objectives()
            .objectives()
            .iter()
            .zip(estimate.summary().dimensions())
        {
            assert_eq!(summary.samples(), trials);
            let key = match objective.key() {
                mission::PolicyObjective::SuccessProbability => "success",
                mission::PolicyObjective::MeanElapsedTime => "time",
                mission::PolicyObjective::MedicalKitUseProbability => "medical",
            };
            let index = objective_named(&experiment.objectives, key)
                .unwrap_or_else(|| panic!("mission measures no `{key}`"));
            assert_eq!(
                totals[index],
                Quantity::from(total_of(*objective.value(), trials)),
                "mission micro {strategy}: {key}"
            );
        }
    }
}

fn connect_four(engine: &Engine, instance: &str) {
    // Self-play, and Red's tree search against each opponent, every game
    // on the instance's board, Red as strong as the instance makes it.
    let size = Size::of(instance);
    let strength = connect_four_strength(size);
    let experiment = engine
        .experiment(&format!("connect-four/{instance}").parse().unwrap())
        .unwrap();
    let arms: Vec<String> = experiment
        .arms
        .iter()
        .map(|arm| arm.strategy.to_string())
        .collect();
    let games = ["self-play", "center-opponent", "search-opponent"];
    assert_eq!(
        arms,
        games.map(|game| format!("connect-four/mcts-{strength}/{game}")),
        "{instance}"
    );
    assert_eq!(
        reproduces(
            engine,
            "connect-four",
            instance,
            &connect_four_outcomes(size),
            &games
        ),
        games.len(),
        "{instance}"
    );
}

fn perishables(engine: &Engine, instance: &str) {
    let outcomes = perishables_outcomes(Size::of(instance));
    // Micro plays its outage until it is resolved and, in an experiment of
    // its own, picks from its exact front of sealed storage plans; the cold
    // chains plan each of their four daily policies over the outage
    // schedule, the storage buffer among them.
    let compared = if instance == "micro" {
        reproduces(engine, "perishables", instance, &outcomes, &["outage"])
            + reproduced_by(
                engine,
                "perishables/micro/storage-plans",
                "perishables",
                instance,
                &outcomes,
                &["pareto-inventory", "pareto-energy"],
            )
    } else {
        reproduces(
            engine,
            "perishables",
            instance,
            &outcomes,
            &[
                "outage",
                "pareto-inventory",
                "pareto-energy",
                "storage-buffer",
            ],
        )
    };
    assert_eq!(
        compared,
        if instance == "micro" { 3 } else { 4 },
        "{instance}"
    );
}

fn work_league(engine: &Engine, instance: &str) {
    // Each field from the economy its workers' policies make: Micro's
    // serial match planned whole, the seasons played turn by turn.
    let documents = ["mixed-field", "throughput-field", "sustainable-field"];
    assert_eq!(
        reproduces(
            engine,
            "work-league",
            instance,
            &work_league_outcomes(Size::of(instance)),
            &documents
        ),
        documents.len(),
        "{instance}"
    );
    work_league_on_more_seeds(engine, instance);
}

/// With the seeds one to four, each arm of the instance's experiment runs
/// the Rust league's match. Between them the seeds bring failed attempts,
/// and in the seasons maintenance, voluntary releases and handoffs too.
fn work_league_on_more_seeds(engine: &Engine, instance: &str) {
    use work_league::{Outcome, RateId};
    let profile = work_league_profile(Size::of(instance));
    let experiment = engine
        .experiment(&format!("work-league/{instance}").parse().unwrap())
        .unwrap();
    // How many of the Rust matches' exchanges were each of those.
    let mut met = [0; 4];
    for arm in &experiment.arms {
        let (_, lineup) = work_league_fields()
            .into_iter()
            .find(|(name, _)| *name == last(arm.strategy.as_str()))
            .unwrap_or_else(|| panic!("no field {}", arm.strategy));
        let league = work_league::league(profile, lineup);
        let start = engine
            .referred(arm.start.as_ref().unwrap_or(&experiment.start))
            .unwrap();
        let strategy = engine.strategy(&arm.strategy).unwrap();
        for seed in 1..=4 {
            let effort = Effort {
                seed,
                ..experiment.effort
            };
            let ran = engine
                .run_strategy(start, &strategy, &experiment.goal, effort, &mut |_| {
                    ControlFlow::Continue(())
                })
                .unwrap();
            let rust = work_league::run(&league, seed).unwrap();
            for exchange in rust.trace().exchanges() {
                match exchange.rate() {
                    RateId::Finish {
                        outcome: Outcome::Failure,
                        ..
                    } => met[0] += 1,
                    RateId::BeginService {
                        maintenance: true, ..
                    } => met[1] += 1,
                    RateId::ReleaseClaim { expired: false, .. } => met[2] += 1,
                    RateId::Handoff { .. } => met[3] += 1,
                    _ => {}
                }
            }
            let trace: Vec<paths::Exchange> =
                rust.trace().exchanges().iter().map(written).collect();
            assert_eq!(
                engine.history(ran.end).unwrap().1,
                trace,
                "work-league {instance} {}: seed {seed}",
                arm.strategy
            );
        }
    }
    // Micro's serial match only ever fails and retries.
    let seasons = instance != "micro";
    assert!(
        met[0] > 0 && met[1..].iter().all(|met| (*met > 0) == seasons),
        "work-league {instance}: failures, maintenance, voluntary releases, handoffs {met:?}"
    );
}

/// Every sample, its package builder and its check.
type Sample = (
    &'static str,
    axionomy_problems::WritePackage,
    fn(&Engine, &str),
);

const SAMPLES: [Sample; 15] = [
    ("maze", axionomy_problems::maze::package::write, maze),
    (
        "sokoban",
        axionomy_problems::sokoban::package::write,
        sokoban,
    ),
    (
        "exact-cover",
        axionomy_problems::exact_cover::package::write,
        exact_cover,
    ),
    (
        "workshop",
        axionomy_problems::workshop::package::write,
        workshop,
    ),
    ("bridge", axionomy_problems::bridge::package::write, bridge),
    ("rescue", axionomy_problems::rescue::package::write, rescue),
    (
        "scheduling",
        axionomy_problems::scheduling::package::write,
        scheduling,
    ),
    ("amm", axionomy_problems::amm::package::write, amm),
    (
        "lending",
        axionomy_problems::lending::package::write,
        lending,
    ),
    (
        "marketplace",
        axionomy_problems::marketplace::package::write,
        marketplace,
    ),
    (
        "logistics",
        axionomy_problems::logistics::package::write,
        logistics,
    ),
    (
        "connect-four",
        axionomy_problems::connect_four::package::write,
        connect_four,
    ),
    (
        "work-league",
        axionomy_problems::work_league::package::write,
        work_league,
    ),
    (
        "mission",
        axionomy_problems::mission::package::write,
        mission,
    ),
    (
        "perishables",
        axionomy_problems::perishables::package::write,
        perishables,
    ),
];

/// Each sample's package reproduces its Rust outcomes at `instances`.
fn samples_reproduce(instances: &[&str]) {
    for (name, write, check) in SAMPLES {
        let engine = loaded(name, write);
        for instance in instances {
            check(&engine, instance);
        }
    }
}

#[test]
fn every_sample_reproduces_its_micro_instance() {
    samples_reproduce(&["micro"]);
}

#[test]
#[ignore = "minutes in a debug build; CI runs it in a release build"]
fn every_sample_reproduces_its_larger_instances() {
    samples_reproduce(&["showcase", "stress"]);
}

#[test]
#[ignore = "seconds in a debug build; CI runs it in a release build"]
fn the_mission_planner_plans_every_scenario_as_rust_does() {
    let engine = loaded("mission", axionomy_problems::mission::package::write);
    let planned = mission_planned(&engine, 0..16);
    // The planner rescues the victim in eleven of the sixteen scenarios: it
    // is stranded where the sensor reads wrong, seed 0 of either site, and
    // where it went south at once with the victim north.
    let stranded: Vec<_> = (0..16).filter(|&seed| !planned[seed].0).collect();
    assert_eq!(stranded, [0, 1, 2, 8, 9], "{planned:?}");
}

#[test]
fn a_package_carries_its_view() {
    let engine = loaded("maze", axionomy_problems::maze::package::write);
    let start = engine.referred(&path!("maze/micro")).unwrap();
    assert_eq!(engine.draw(start).unwrap().title, "Key-door maze");
}
