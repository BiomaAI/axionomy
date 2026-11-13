//! A package carries what an experiment needs, and re-runs it identically
//! wherever it is loaded, zipped or not.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{
    Arm, CandidateSet, Contents, Engine, EngineError, Experiment, Manifest, Measure, Objective,
    PackageError, ScriptCall, Strategy, Version, package, zipped,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Store;
use std::{ops::ControlFlow, path::PathBuf};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn call(function: &str) -> ScriptCall {
    ScriptCall::new(path("bank"), function.parse().unwrap())
}

/// A file in a directory of its own, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "axionomy-packages-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A bank paying alice, with what an experiment on it needs, and a run of
/// that experiment.
fn bank() -> Engine {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 20)]).into())
        .account(path("alice"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("bank"),
            r#"
            fn randomly(state, random) {
                exchange("pay", #{ from: "bank", to: "alice" }, random.next(5) + 1)
            }
            fn paid(state) { state.balance("alice", "cash") }
            fn payment(before, exchange, after) { 1 }
            "#,
        )
        .unwrap();
    // A script nothing in the package uses stays out of it.
    engine
        .keep_script(&path("bank/unused"), "fn nothing() { () }")
        .unwrap();
    engine
        .keep_goal(
            &path("bank/rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 10)])),
        )
        .unwrap();
    let pay = |units| {
        paths::Exchange::new(path("pay"), Quantity::new(units))
            .bind(path("from"), path("bank"))
            .bind(path("to"), path("alice"))
    };
    engine
        .keep_candidates(
            &path("bank/payments"),
            &CandidateSet::Listed(vec![pay(1), pay(5)]),
        )
        .unwrap();
    engine
        .keep_strategy(
            &path("bank/randomly"),
            &Strategy::Policy {
                policy: call("randomly"),
                goal: path("bank/rich"),
                horizon: 20,
            },
        )
        .unwrap();
    engine
        .keep_strategy(
            &path("bank/searched"),
            &Strategy::BreadthFirst {
                candidates: path("bank/payments"),
                goal: path("bank/rich"),
            },
        )
        .unwrap();
    for (objective, measure, direction) in [
        (
            "bank/payments",
            Measure::Steps(call("payment")),
            ObjectiveDirection::Minimize,
        ),
        (
            "bank/paid",
            Measure::State(call("paid")),
            ObjectiveDirection::Maximize,
        ),
    ] {
        engine
            .keep_objective(&path(objective), &Objective { measure, direction })
            .unwrap();
    }
    let start = engine.author(economy).unwrap();
    engine.refer(&path("bank/start"), start).unwrap();
    engine
        .keep_experiment(
            &path("bank/paying"),
            &Experiment {
                start: path("bank/start"),
                arms: ["bank/randomly", "bank/searched"]
                    .iter()
                    .map(|strategy| Arm {
                        strategy: path(strategy),
                        start: None,
                    })
                    .collect(),
                objectives: vec![path("bank/payments"), path("bank/paid")],
                seed: 5,
                budget: 1_000,
                trials: 3,
            },
        )
        .unwrap();
    run(&engine);
    engine
}

fn run(engine: &Engine) -> axionomy_engine::Results {
    engine
        .run_experiment(&path("bank/paying"), &mut |_| ControlFlow::Continue(()))
        .unwrap()
}

fn version() -> Version {
    "1.2.0".parse().unwrap()
}

/// `engine`'s experiment exported as package `bank` into a compacted file.
fn exported(engine: &Engine, scratch: &Scratch) -> (Manifest, Vec<u8>) {
    let file = scratch.file("bank.redb");
    let manifest = {
        let to = Store::open(&file).unwrap();
        engine
            .export(
                &path("bank"),
                version(),
                Contents {
                    refs: Vec::new(),
                    definitions: vec![path("experiment/bank/paying")],
                },
                &to,
            )
            .unwrap()
    };
    Store::compact(&file).unwrap();
    (manifest, std::fs::read(&file).unwrap())
}

/// Every trial's seed, how it ended, its values and its exchanges.
fn trials(engine: &Engine, results: &axionomy_engine::Results) -> Vec<String> {
    results
        .arms
        .iter()
        .flat_map(|arm| &arm.trials)
        .map(|trial| {
            format!(
                "{} {:?} {:?} {:?}",
                trial.seed,
                trial.ended,
                trial.values,
                engine.history(trial.end).unwrap().1
            )
        })
        .collect()
}

#[test]
fn a_package_carries_what_its_experiment_needs_and_reruns_it() {
    let scratch = Scratch::new("rerun");
    let source = bank();
    let (manifest, bytes) = exported(&source, &scratch);
    assert_eq!(manifest.name.as_str(), "bank");
    assert_eq!(manifest.version.to_string(), "1.2.0");
    assert_eq!(
        manifest
            .refs
            .iter()
            .map(|reference| reference.as_str())
            .collect::<Vec<_>>(),
        ["bank/start"]
    );
    let carried: Vec<&str> = manifest
        .definitions
        .iter()
        .map(|definition| definition.as_str())
        .collect();
    assert_eq!(
        carried,
        [
            "candidates/bank/payments",
            "experiment/bank/paying",
            "goal/bank/rich",
            "objective/bank/paid",
            "objective/bank/payments",
            "results/bank/paying",
            "script/bank",
            "strategy/bank/randomly",
            "strategy/bank/searched",
        ]
    );

    for file in [bytes.clone(), zipped(&bytes).unwrap()] {
        let loaded = Engine::new(Store::memory().unwrap()).unwrap();
        let outcome = loaded.load(&package(&file).unwrap()).unwrap();
        assert_eq!(outcome.manifests, std::slice::from_ref(&manifest));
        assert!(outcome.kept.refs.is_empty() && outcome.kept.definitions.is_empty());
        assert_eq!(loaded.manifests().unwrap(), std::slice::from_ref(&manifest));
        // The results came with the package, and running the experiment
        // again gives them again.
        let carried = loaded.results(&path("bank/paying")).unwrap();
        assert_eq!(carried, source.results(&path("bank/paying")).unwrap());
        let again = run(&loaded);
        assert_eq!(trials(&loaded, &again), trials(&source, &carried));
        assert!(loaded.script(&path("bank/unused")).unwrap().is_none());
    }
}

#[test]
fn a_package_names_its_refs_under_its_name() {
    let source = bank();
    let start = source.referred(&path("bank/start")).unwrap();
    source.refer(&path("elsewhere"), start).unwrap();
    let refused = source.export(
        &path("bank"),
        version(),
        Contents {
            refs: vec![path("elsewhere")],
            definitions: Vec::new(),
        },
        &Store::memory().unwrap(),
    );
    assert!(matches!(
        refused,
        Err(EngineError::Package(PackageError::Outside { .. }))
    ));
}

#[test]
fn loading_checks_the_package() {
    let scratch = Scratch::new("checks");
    let (mut manifest, bytes) = exported(&bank(), &scratch);
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    assert!(matches!(
        engine.load(&Store::memory().unwrap()),
        Err(EngineError::Package(PackageError::NoManifest))
    ));

    // A package written for a newer script host API is refused, and
    // nothing of it is merged.
    let newer = package(&bytes).unwrap();
    manifest.host_api = axionomy_engine::HOST_API + 1;
    let encoded = postcard::to_allocvec(&manifest).unwrap();
    newer
        .transact(axionomy_store::Durability::Immediate, move |txn| {
            txn.define(&path("package/bank"), Some(&encoded))
        })
        .wait()
        .unwrap();
    assert!(matches!(
        engine.load(&newer),
        Err(EngineError::Package(PackageError::HostApi { .. }))
    ));
    assert!(engine.manifests().unwrap().is_empty());

    // So is one whose manifest says it is written in another store format.
    let other = package(&bytes).unwrap();
    manifest.host_api = axionomy_engine::HOST_API;
    manifest.format = axionomy_store::FORMAT_VERSION + 1;
    let encoded = postcard::to_allocvec(&manifest).unwrap();
    other
        .transact(axionomy_store::Durability::Immediate, move |txn| {
            txn.define(&path("package/bank"), Some(&encoded))
        })
        .wait()
        .unwrap();
    assert!(matches!(
        engine.load(&other),
        Err(EngineError::Package(PackageError::Format { .. }))
    ));
    assert!(engine.manifests().unwrap().is_empty());

    assert!(matches!(
        package(b"PK\x03\x04 not really a zip"),
        Err(EngineError::Package(PackageError::Archive(_)))
    ));
}
