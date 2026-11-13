//! The samples as packages (P9): each package, loaded alone into a fresh
//! store, runs experiments that reproduce its study's outcomes, objectives
//! and traces.
#![cfg(feature = "packages")]

use axionomy::{path, paths};
use axionomy_engine::{ArmResult, Effort, Ended, Engine, ObjectivePath, Results, Version};
use axionomy_problems::{registry, rescue};
use axionomy_store::{Checkpoints, Store};
use axionomy_study::{Progress, StudyControl, StudyObserver, StudyRequest};
use axionomy_view::{DocumentPath, TelemetryKindView};
use serde::{Serialize, de::DeserializeOwned};
use std::{cell::RefCell, collections::HashMap, ops::ControlFlow};

#[derive(Default)]
struct Trajectories(Vec<(DocumentPath, paths::Trace)>);

impl StudyObserver for Trajectories {
    fn progress(&mut self, _: Progress) {}

    fn trajectory(&mut self, document: &DocumentPath, _: &paths::Economy, trace: &paths::Trace) {
        self.0.push((document.clone(), trace.clone()));
    }
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

/// A study and the trajectory of each of its documents.
type Studied = (axionomy_study::Study, Vec<(DocumentPath, paths::Trace)>);

/// What a study is asked for: a problem, an instance, maybe a strategy.
type Asked = (String, String, Option<String>);

thread_local! {
    /// Every study made so far, which a sample's checks may ask for again.
    static STUDIED: RefCell<HashMap<Asked, Studied>> = RefCell::default();
}

/// The study of `problem` at `instance`, asked for `strategy` or its
/// default, and the trajectory of each of its documents.
fn study(problem: &str, instance: &str, strategy: Option<&str>) -> Studied {
    let key = (
        problem.to_owned(),
        instance.to_owned(),
        strategy.map(str::to_owned),
    );
    if let Some(studied) = STUDIED.with_borrow(|studied| studied.get(&key).cloned()) {
        return studied;
    }
    let mut request =
        StudyRequest::new(problem.parse().unwrap()).with_instance(instance.parse().unwrap());
    if let Some(strategy) = strategy {
        request = request.with_strategy(strategy.parse().unwrap());
    }
    let mut observed = Trajectories::default();
    let study = registry()
        .study_with(&request, &StudyControl::default(), &mut observed)
        .unwrap();
    let studied = (study, observed.0);
    STUDIED.with_borrow_mut(|studied_so_far| studied_so_far.insert(key, studied.clone()));
    studied
}

/// Where the objective `key` a document reports is among `objectives`: the
/// one whose path ends in the key, such as `maze/energy` for `energy` or
/// `bridge/priority/a` for `priority/a`.
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

/// The study of `problem` at `instance`, asked for each of `strategies`,
/// gives a document named for the strategy that the package's experiment
/// at `instance` reproduces: the arm running the strategy of that name
/// reaches the document's exchanges, and its value on each objective the
/// document reports, the experiment's objective of that name. Returns how
/// many documents it compared.
fn reproduces(engine: &Engine, problem: &str, instance: &str, strategies: &[&str]) -> usize {
    let mut compared = 0;
    let experiment = format!("{problem}/{instance}").parse().unwrap();
    let objectives = engine.experiment(&experiment).unwrap().objectives;
    let results = engine
        .run_experiment(&experiment, &mut |_| ControlFlow::Continue(()))
        .unwrap();
    let default = study(problem, instance, None);
    for strategy in strategies {
        // A study holds documents for its default and other strategies; one
        // asked for its strategy is studied only when that one is missing.
        let has = |(study, _): &(axionomy_study::Study, _)| {
            study
                .documents
                .iter()
                .any(|document| document.id.as_str() == *strategy)
        };
        let catalogued = registry()
            .problem(&problem.parse().unwrap())
            .unwrap()
            .strategies
            .iter()
            .any(|listed| listed.key.as_str() == *strategy);
        let asked;
        let (study, trajectories) = if has(&default) {
            &default
        } else if catalogued {
            asked = study(problem, instance, Some(strategy));
            &asked
        } else {
            // A document no catalogued strategy asks for, which the study
            // gives only for some instances.
            continue;
        };
        let arm = arm(&results, strategy);
        let (_, trace) = trajectories
            .iter()
            .find(|(document, _)| document.as_str() == *strategy)
            .unwrap_or_else(|| panic!("no document of {strategy}"));
        let trial = &arm.trials[0];
        let (_, exchanges) = engine.history(trial.end).unwrap();
        assert_eq!(
            exchanges,
            trace.exchanges(),
            "{problem} {instance} {strategy}: the trace"
        );
        let document = study
            .documents
            .iter()
            .find(|document| document.id.as_str() == *strategy)
            .unwrap();
        for reported in &document.objectives {
            let index = objective_named(&objectives, reported.key.as_str())
                .unwrap_or_else(|| panic!("{problem} measures no `{}`", reported.key));
            assert_eq!(
                trial.values[index], reported.value,
                "{problem} {instance} {strategy}: {}",
                reported.key
            );
        }
        compared += 1;
    }
    compared
}

/// The document `document` of the study of `problem` at `instance` is the
/// history of the package's ref `reference`.
fn recorded(engine: &Engine, problem: &str, instance: &str, document: &str, reference: &str) {
    let (_, trajectories) = study(problem, instance, None);
    let (_, trace) = trajectories
        .iter()
        .find(|(id, _)| id.as_str() == document)
        .unwrap_or_else(|| panic!("no document {document}"));
    let point = engine.referred(&reference.parse().unwrap()).unwrap();
    assert_eq!(
        engine.history(point).unwrap().1,
        trace.exchanges(),
        "{problem} {instance} {document}"
    );
}

/// What each sample checks of its package at `instance`.
fn maze(engine: &Engine, instance: &str) {
    reproduces(
        engine,
        "maze",
        instance,
        &[
            "breadth-first",
            "dijkstra",
            "a-star",
            "pareto-energy",
            "pareto-time",
        ],
    );
}

fn sokoban(engine: &Engine, instance: &str) {
    reproduces(engine, "sokoban", instance, &["a-star", "breadth-first"]);
    recorded(
        engine,
        "sokoban",
        instance,
        "deadlock",
        &format!("sokoban/{instance}/deadlock"),
    );
}

fn exact_cover(engine: &Engine, instance: &str) {
    reproduces(
        engine,
        "exact-cover",
        instance,
        &["breadth-first", "algorithm-x"],
    );
    recorded(
        engine,
        "exact-cover",
        instance,
        "unsatisfiable",
        &format!("exact-cover/{instance}/unsatisfiable"),
    );
    // No strategy covers the unsatisfiable twin: search runs out, and
    // Algorithm X makes no plan.
    let results = engine
        .run_experiment(
            &format!("exact-cover/{instance}/unsatisfiable")
                .parse()
                .unwrap(),
            &mut |_| ControlFlow::Continue(()),
        )
        .unwrap();
    let ended: Vec<_> = results.arms.iter().map(|arm| arm.trials[0].ended).collect();
    assert_eq!(ended, [Ended::Exhausted, Ended::Stopped], "{instance}");
}

fn workshop(engine: &Engine, instance: &str) {
    let compared = reproduces(
        engine,
        "workshop",
        instance,
        &[
            "breadth-first",
            "minimum-waste",
            "pareto-waste",
            "pareto-time",
            "pareto-revenue",
        ],
    );
    // Only instances with customers earn revenue, and have its document.
    assert_eq!(
        compared,
        if instance == "micro" { 4 } else { 5 },
        "{instance}"
    );
}

fn bridge(engine: &Engine, instance: &str) {
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
        reproduces(engine, "bridge", instance, strategies),
        strategies.len(),
        "{instance}"
    );
    if instance == "micro" {
        return;
    }
    // In timed traffic the study prefers, for each traveller, the policy
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
    let (study, trajectories) = study("bridge", instance, None);
    for (document, traveller) in [("pareto-a", "a"), ("pareto-b", "b")] {
        let by: Vec<ObjectivePath> = ["ontime", "priority", "credit"]
            .into_iter()
            .map(|objective| format!("bridge/{objective}/{traveller}").parse().unwrap())
            .collect();
        let picked = engine.pick(&ends, &by).unwrap().unwrap();
        let (_, trace) = trajectories
            .iter()
            .find(|(id, _)| id.as_str() == document)
            .unwrap_or_else(|| panic!("no document {document}"));
        assert_eq!(
            engine.history(picked).unwrap().1,
            trace.exchanges(),
            "bridge {instance} {document}: the trace"
        );
        let reported = &study
            .documents
            .iter()
            .find(|found| found.id.as_str() == document)
            .unwrap()
            .objectives;
        for objective in reported {
            let path = format!("bridge/{}", objective.key).parse().unwrap();
            assert_eq!(
                engine.measure(picked, &path).unwrap(),
                objective.value,
                "bridge {instance} {document}: {}",
                objective.key
            );
        }
    }
}

fn rescue(engine: &Engine, instance: &str) {
    let compared = reproduces(
        engine,
        "rescue",
        instance,
        &["observe-then-follow", "direct-north", "stabilize-first"],
    );
    // Only the incidents have kits to stabilize with, and its document.
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
        reproduces(engine, "lending", instance, &strategies),
        strategies.len(),
        "{instance}"
    );
}

/// `value`, a typed identifier, exchange or economy, as paths.
fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

/// Every episode a Rust Monte Carlo of the rescue played, in its order.
#[derive(Default)]
struct Episodes(Vec<(rescue::Policy, usize, Vec<axionomy::paths::Exchange>)>);

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

/// The Micro study estimates each policy by Monte Carlo over Nature's
/// prior: the package's Monte Carlo experiment plays the scenario each of
/// the study's samples plays, trial by trial, and its means are the study's
/// sampled front.
fn rescue_monte_carlo(engine: &Engine) {
    let path = path!("rescue/micro/monte-carlo");
    let experiment = engine.experiment(&path).unwrap();
    let results = engine
        .run_experiment(&path, &mut |_| ControlFlow::Continue(()))
        .unwrap();
    let trials = usize::try_from(experiment.trials).unwrap();

    // Trial by trial, the episodes of the Rust Monte Carlo seeded alike.
    let mut episodes = Episodes::default();
    rescue::policy_front(
        &rescue::uniform_uncertain(),
        trials,
        experiment.seed,
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

    // Each document's front point for its policy: the arm's means, which the
    // front prints to three decimals, finer than a trial in 128 moves them.
    let (study, _) = study("rescue", "micro", None);
    for document in &study.documents {
        let arm = arm(&results, document.id.as_str());
        let sampled = document
            .telemetry
            .iter()
            .flat_map(|telemetry| &telemetry.points)
            .find(|point| point.kind == TelemetryKindView::Sample)
            .unwrap();
        assert_eq!(sampled.value, trials.to_string());
        let front = &document.pareto_fronts[0];
        let point = front.points.iter().find(|point| point.selected).unwrap();
        let totals = arm.totals();
        for (axis, value) in front.axes.iter().zip(&point.values) {
            let index = experiment
                .objectives
                .iter()
                .position(|objective| last(objective.as_str()) == axis.key.as_str())
                .unwrap_or_else(|| panic!("rescue measures no `{}`", axis.key));
            let mean = u64::try_from(totals[index]).unwrap() as f64 / trials as f64;
            let reported: f64 = value.parse().unwrap();
            assert!(
                (mean - reported).abs() <= 0.0005,
                "rescue micro {}: {} is {mean}, the study's {reported}",
                document.id,
                axis.key
            );
        }
    }
}

fn scheduling(engine: &Engine, instance: &str) {
    let compared = reproduces(
        engine,
        "scheduling",
        instance,
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
        "scheduling",
        instance,
        "infeasible-horizon",
        &infeasible,
    );
    // No strategy fits the work into too short a horizon: every search
    // runs out.
    let results = engine
        .run_experiment(&infeasible.parse().unwrap(), &mut |_| {
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
        reproduces(engine, "amm", instance, &documents),
        documents.len(),
        "{instance}"
    );
}

fn marketplace(engine: &Engine, instance: &str) {
    // Micro clears and ranks its exact front; the funded markets plan
    // fulfillment by the study's beam, for volume, buyers or sellers.
    let compared = reproduces(
        engine,
        "marketplace",
        instance,
        &["market-clearing", "pareto-buyers", "pareto-sellers"],
    );
    assert_eq!(compared, 3, "{instance}");
}

fn logistics(engine: &Engine, instance: &str) {
    // Micro's fixed routes and planner; the fleet's dispatch policies.
    let documents = ["direct", "reliable", "mcts"];
    assert_eq!(
        reproduces(engine, "logistics", instance, &documents),
        documents.len(),
        "{instance}"
    );
    if instance == "micro" {
        logistics_beyond_the_study(engine);
    }
}

/// Micro's study plans a single decision. On the economy with all four
/// orders waiting, which its Monte Carlo samples, the package's policies
/// play the Rust ones, and its planner the Rust receding-horizon run, which
/// replans thirteen times there, around three breakdowns, with this seed.
fn logistics_beyond_the_study(engine: &Engine) {
    use axionomy_problems::logistics::{self, Policy};
    const SEED: u64 = 2;
    let start = engine
        .store()
        .record::<logistics::Ontology>(logistics::initial(), Checkpoints::Ends)
        .finish()
        .unwrap();
    let ran = |strategy: &str| {
        let strategy = engine
            .strategy(&format!("logistics/{strategy}").parse().unwrap())
            .unwrap();
        let effort = Effort {
            seed: SEED,
            budget: 1,
        };
        let ran = engine
            .run_strategy(start, &strategy, effort, &mut |_| ControlFlow::Continue(()))
            .unwrap();
        engine.history(ran.end).unwrap().1
    };
    let traced = |trace: &axionomy::Trace<logistics::Ontology>| -> Vec<paths::Exchange> {
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
    // The study's planning seed for a request seeded `SEED`.
    let planned = logistics::run_planned_with_progress(
        &logistics::initial(),
        16,
        SEED,
        SEED ^ 0x4c4f_4749_5354_4943,
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
    assert_eq!((planned.decisions.len(), repairs), (13, 3));
    assert_eq!(ran("mcts"), trace, "logistics mcts");
}

fn perishables(engine: &Engine, instance: &str) {
    // Micro plays its outage and picks from its exact storage front; the
    // cold chains plan each of their four daily policies over the outage
    // schedule, the storage buffer among them.
    let compared = reproduces(
        engine,
        "perishables",
        instance,
        &[
            "outage",
            "pareto-inventory",
            "pareto-energy",
            "storage-buffer",
        ],
    );
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
        reproduces(engine, "work-league", instance, &documents),
        documents.len(),
        "{instance}"
    );
    work_league_beyond_the_study(engine, instance);
}

/// Beyond the study's seed: with the seeds below four, which bring more
/// failed attempts, maintenance, voluntary releases and handoffs, each
/// arm of the instance's experiment runs the Rust league's match.
fn work_league_beyond_the_study(engine: &Engine, instance: &str) {
    use axionomy_problems::work_league::{self, Profile};
    let profile = match instance {
        "micro" => Profile::Micro,
        "showcase" => Profile::Showcase,
        _ => Profile::Stress,
    };
    let experiment = engine
        .experiment(&format!("work-league/{instance}").parse().unwrap())
        .unwrap();
    for arm in &experiment.arms {
        let lineup = match last(arm.strategy.as_str()) {
            "mixed-field" => work_league::mixed_lineup(),
            "throughput-field" => work_league::throughput_lineup(),
            _ => work_league::sustainable_lineup(),
        };
        let league = work_league::league(profile, lineup);
        let start = engine
            .referred(arm.start.as_ref().unwrap_or(&experiment.start))
            .unwrap();
        let strategy = engine.strategy(&arm.strategy).unwrap();
        for seed in 0..4 {
            let effort = Effort { seed, budget: 1 };
            let ran = engine
                .run_strategy(start, &strategy, effort, &mut |_| ControlFlow::Continue(()))
                .unwrap();
            let rust = work_league::run(&league, seed).unwrap();
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
}

/// Every sample, its package builder and its check.
type Sample = (
    &'static str,
    axionomy_problems::WritePackage,
    fn(&Engine, &str),
);

const SAMPLES: [Sample; 13] = [
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
        "work-league",
        axionomy_problems::work_league::package::write,
        work_league,
    ),
    (
        "perishables",
        axionomy_problems::perishables::package::write,
        perishables,
    ),
];

/// Each sample's package reproduces its study at `instances`.
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
fn a_package_carries_its_view() {
    let engine = loaded("maze", axionomy_problems::maze::package::write);
    let start = engine.referred(&path!("maze/micro")).unwrap();
    assert_eq!(engine.draw(start).unwrap().title, "Key-door maze");
}
